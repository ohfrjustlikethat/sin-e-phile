//! Item-item collaborative filtering (SPEC.md Phase 17, Layer 2).
//!
//! Two titles are alike when the same people loved both. With 160,000 MovieLens users that
//! is a signal no description can give: it knows that people who love one slow Japanese
//! family drama love another, whatever words the synopses use.
//!
//! # How it is computed
//!
//! - **A like** is a rating of [`LIKED`] or more (MovieLens' half-star scale). Only likes
//!   count: "people who rated both" mostly says "people who watch a lot".
//! - **Each person is weighted** `1 / ln(e + their likes)`, so one person with 5,000 likes
//!   does not outweigh fifty people with fifty each. An item's **mass** is the sum of its
//!   likers' weights, a weighted count of the people who liked it.
//! - **Liked together**, `c(i, j)`, is the summed weight of the people who liked both.
//! - **The score** is one family with two numbers ([`Score`]), chosen on tuning users rather
//!   than by taste:
//!   `c / (mass_i^(1-alpha) * mass_j^alpha)`, times `c / (c + shrink)`.
//!   With `alpha` 0 it is "of the people who liked i, how many liked j": blockbusters win,
//!   because everyone liked them. With `alpha` 0.5 it is cosine. Towards 1 it is lift, which
//!   divides the popularity back out, so a beloved obscure film can outrank a mediocre
//!   blockbuster. `shrink` distrusts a pair seen together only a handful of times.
//! - **Item by item**: for item `i`, walk the people who liked it and add each one's weight
//!   to every other item they liked. One dense array per thread, reset after each item
//!   through the list of what it touched, so nothing the size of items x items is ever held.
//! - Only the [`Matrix`]'s top neighbours per item are kept.

use std::collections::HashMap;

/// A rating of this or more, on MovieLens' 0.5-5 scale, is a like.
pub const LIKED: f32 = 4.0;

/// Items liked by fewer people than this are left out: their neighbours are noise.
pub const MIN_LIKES: u32 = 5;

/// One rating, as MovieLens writes it: a user, an item, and the stars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rating {
    pub user: u32,
    pub item: u32,
    pub stars: f32,
}

/// Rows of numbers, stored end to end: row `r` is `of[start[r]..start[r + 1]]`.
struct Rows {
    start: Vec<usize>,
    of: Vec<u32>,
}

impl Rows {
    fn row(&self, r: usize) -> &[u32] {
        &self.of[self.start[r]..self.start[r + 1]]
    }

    fn len(&self) -> usize {
        self.start.len() - 1
    }
}

/// Every like, indexed by person and by item. Items are numbered densely (`0..n`) inside;
/// [`Likes::id`] gives an item's own id back.
pub struct Likes {
    ids: Vec<u32>,
    by_user: Rows,
    weight: Vec<f32>,
    by_item: Rows,
    mass: Vec<f32>,
}

impl Likes {
    /// The likes in `ratings`, leaving out items liked by fewer than `min_likes` people.
    pub fn from_ratings(ratings: impl IntoIterator<Item = Rating>, min_likes: u32) -> Self {
        let mut pairs: Vec<(u32, u32)> = ratings
            .into_iter()
            .filter(|r| r.stars >= LIKED)
            .map(|r| (r.user, r.item))
            .collect();
        pairs.sort_unstable();
        pairs.dedup();

        let mut likers: HashMap<u32, u32> = HashMap::new();
        for &(_, item) in &pairs {
            *likers.entry(item).or_default() += 1;
        }
        let mut ids: Vec<u32> = likers
            .into_iter()
            .filter(|&(_, n)| n >= min_likes)
            .map(|(item, _)| item)
            .collect();
        ids.sort_unstable();
        let dense: HashMap<u32, u32> = ids
            .iter()
            .enumerate()
            .map(|(d, &item)| (item, d as u32))
            .collect();

        // By person: the pairs are sorted by person, so each person's items are a run.
        let mut start = vec![0];
        let mut of = Vec::with_capacity(pairs.len());
        let mut current = None;
        for &(user, item) in &pairs {
            let Some(&d) = dense.get(&item) else { continue };
            if current != Some(user) {
                if current.is_some() {
                    start.push(of.len());
                }
                current = Some(user);
            }
            of.push(d);
        }
        if current.is_some() {
            start.push(of.len());
        }
        drop(pairs);
        let by_user = Rows { start, of };
        let weight: Vec<f32> = (0..by_user.len())
            .map(|u| 1.0 / (std::f32::consts::E + by_user.row(u).len() as f32).ln())
            .collect();

        // By item: count, then place each person in their items' rows.
        let mut count = vec![0usize; ids.len()];
        for &d in &by_user.of {
            count[d as usize] += 1;
        }
        let mut start = Vec::with_capacity(ids.len() + 1);
        start.push(0);
        for c in &count {
            start.push(start.last().unwrap() + c);
        }
        let mut next = start.clone();
        let mut of = vec![0u32; by_user.of.len()];
        let mut mass = vec![0f32; ids.len()];
        for (u, &w) in weight.iter().enumerate() {
            for &d in by_user.row(u) {
                of[next[d as usize]] = u as u32;
                next[d as usize] += 1;
                mass[d as usize] += w;
            }
        }
        Self {
            ids,
            by_user,
            weight,
            by_item: Rows { start, of },
            mass,
        }
    }

    /// How many items are in the matrix.
    pub fn items(&self) -> usize {
        self.ids.len()
    }

    /// How many people liked anything kept.
    pub fn people(&self) -> usize {
        self.by_user.len()
    }

    /// How many likes are kept.
    pub fn likes(&self) -> usize {
        self.by_user.of.len()
    }

    /// An item's own id.
    pub fn id(&self, dense: u32) -> u32 {
        self.ids[dense as usize]
    }

    /// Every item's own id with how many people liked it, most liked first: the popularity
    /// baseline a recommender has to beat (E1).
    pub fn popularity(&self) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = (0..self.ids.len())
            .map(|d| (self.ids[d], self.by_item.row(d).len() as u32))
            .collect();
        out.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }
}

/// The score's two numbers (see the module's note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    /// 0: of i's likers, how many liked j (popularity wins). 0.5: cosine. 1: lift.
    pub alpha: f32,
    /// How many people's weight of evidence a pair needs before it is believed.
    pub shrink: f32,
}

/// The score the app builds with, chosen on MovieLens' tuning users (`eval recommend
/// --tune`, docs/eval-results.md "The item-item matrix"): the best Recall@20 whose coverage
/// clears 15% (E2).
pub const CHOSEN: Score = Score {
    alpha: 0.65,
    shrink: 0.0,
};

impl Score {
    pub const COSINE: Score = Score {
        alpha: 0.5,
        shrink: 0.0,
    };

    /// The score of `j` as a neighbour of `i`.
    pub fn of(self, together: f32, mass_i: f32, mass_j: f32) -> f32 {
        let similarity = together / (mass_i.powf(1.0 - self.alpha) * mass_j.powf(self.alpha));
        similarity * (together / (together + self.shrink))
    }
}

/// One item's neighbours with their scores, best first.
type Row = Vec<(u32, f32)>;

/// Each item's best neighbours, in items' own ids.
pub struct Matrix {
    ids: Vec<u32>,
    index: HashMap<u32, u32>,
    rows: Vec<Vec<(u32, f32)>>,
}

/// The `top` neighbours of every item in `likes`, by `score`, on `threads` threads.
pub fn build(likes: &Likes, score: Score, top: usize, threads: usize) -> Matrix {
    let n = likes.items();
    let threads = threads.clamp(1, n.max(1));
    let mut rows: Vec<Vec<(u32, f32)>> = vec![Vec::new(); n];
    // Thread t takes items t, t + threads, t + 2 * threads, ...: popular items are spread
    // over the threads rather than landing in one thread's stretch of ids.
    let done: Vec<Vec<(usize, Row)>> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut together = vec![0f32; n];
                    let mut touched: Vec<u32> = Vec::new();
                    // Reused for every item: a popular item touches thousands, and a row
                    // that kept that much room would hold gigabytes over 18,000 items.
                    let mut scored: Vec<(u32, f32)> = Vec::new();
                    let mut out = Vec::new();
                    for i in (t..n).step_by(threads) {
                        for &u in likes.by_item.row(i) {
                            let w = likes.weight[u as usize];
                            for &j in likes.by_user.row(u as usize) {
                                if j as usize == i {
                                    continue;
                                }
                                // Every weight is above zero, so zero means "not yet touched".
                                if together[j as usize] == 0.0 {
                                    touched.push(j);
                                }
                                together[j as usize] += w;
                            }
                        }
                        scored.clear();
                        scored.extend(touched.iter().map(|&j| {
                            let c = together[j as usize];
                            (j, score.of(c, likes.mass[i], likes.mass[j as usize]))
                        }));
                        for &j in &touched {
                            together[j as usize] = 0.0;
                        }
                        touched.clear();
                        keep_best(&mut scored, top);
                        // Exactly the kept neighbours, in the items' own ids.
                        let row: Vec<(u32, f32)> =
                            scored.iter().map(|&(j, s)| (likes.id(j), s)).collect();
                        out.push((i, row));
                    }
                    out
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("a matrix thread panicked"))
            .collect()
    });
    for (i, row) in done.into_iter().flatten() {
        rows[i] = row;
    }
    Matrix {
        index: likes
            .ids
            .iter()
            .enumerate()
            .map(|(d, &id)| (id, d as u32))
            .collect(),
        ids: likes.ids.clone(),
        rows,
    }
}

/// Best first; equal scores by id, so the same input always gives the same rows.
fn better(a: &(u32, f32), b: &(u32, f32)) -> std::cmp::Ordering {
    b.1.total_cmp(&a.1).then(a.0.cmp(&b.0))
}

/// Keep the `top` best of `scored`, best first.
fn keep_best(scored: &mut Vec<(u32, f32)>, top: usize) {
    if scored.len() > top {
        scored.select_nth_unstable_by(top, better);
        scored.truncate(top);
    }
    scored.sort_unstable_by(better);
}

impl Matrix {
    /// How many items have a row.
    pub fn items(&self) -> usize {
        self.ids.len()
    }

    /// `id`'s neighbours, best first; empty for an item not in the matrix.
    pub fn neighbours(&self, id: u32) -> &[(u32, f32)] {
        self.index
            .get(&id)
            .map(|&d| self.rows[d as usize].as_slice())
            .unwrap_or(&[])
    }

    /// Every item with its neighbours: what is stored.
    pub fn rows(&self) -> impl Iterator<Item = (u32, &[(u32, f32)])> {
        self.ids
            .iter()
            .zip(&self.rows)
            .map(|(&id, row)| (id, row.as_slice()))
    }

    /// The `n` items most like everything in `liked`, by the sum of their scores against
    /// each liked item, leaving out what is already liked. Classic item-based CF.
    pub fn recommend(&self, liked: &[u32], n: usize) -> Vec<u32> {
        let mut sum: HashMap<u32, f32> = HashMap::new();
        for &l in liked {
            for &(j, s) in self.neighbours(l) {
                *sum.entry(j).or_default() += s;
            }
        }
        for l in liked {
            sum.remove(l);
        }
        let mut scored: Vec<(u32, f32)> = sum.into_iter().collect();
        keep_best(&mut scored, n);
        scored.into_iter().map(|(j, _)| j).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn likes(rows: &[(u32, &[u32])]) -> Vec<Rating> {
        rows.iter()
            .flat_map(|&(user, items)| {
                items.iter().map(move |&item| Rating {
                    user,
                    item,
                    stars: 4.5,
                })
            })
            .collect()
    }

    fn weight(n: usize) -> f32 {
        1.0 / (std::f32::consts::E + n as f32).ln()
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn cosine_by_hand() {
        // Person 1 likes A (10) and B (20); person 2 A, B and C (30); person 3 B and C.
        let l = Likes::from_ratings(
            likes(&[(1, &[10, 20]), (2, &[10, 20, 30]), (3, &[20, 30])]),
            1,
        );
        let m = build(&l, Score::COSINE, 10, 1);
        let (w1, w2, w3) = (weight(2), weight(3), weight(2));
        let (mass_a, mass_b, mass_c) = (w1 + w2, w1 + w2 + w3, w2 + w3);
        let ab = (w1 + w2) / (mass_a * mass_b).sqrt();
        let ac = w2 / (mass_a * mass_c).sqrt();
        let row = m.neighbours(10);
        assert_eq!(row.len(), 2);
        assert_eq!(row[0].0, 20);
        assert!(close(row[0].1, ab), "{} vs {ab}", row[0].1);
        assert_eq!(row[1].0, 30);
        assert!(close(row[1].1, ac), "{} vs {ac}", row[1].1);
    }

    #[test]
    fn only_likes_count_and_rarely_liked_items_are_left_out() {
        let mut ratings = likes(&[(1, &[10, 20]), (2, &[10, 20]), (3, &[10])]);
        // A three-star rating of 20 by person 3 is not a like.
        ratings.push(Rating {
            user: 3,
            item: 20,
            stars: 3.0,
        });
        // 99 is liked once, under the threshold of two.
        ratings.push(Rating {
            user: 1,
            item: 99,
            stars: 5.0,
        });
        let l = Likes::from_ratings(ratings, 2);
        assert_eq!(l.items(), 2);
        assert_eq!(l.likes(), 5);
        assert_eq!(l.popularity(), vec![(10, 3), (20, 2)]);
        // Person 1's like of 99 is gone, and with it 99 from every row.
        let m = build(&l, Score::COSINE, 10, 1);
        assert!(m.neighbours(10).iter().all(|&(j, _)| j != 99));
        assert!(m.neighbours(99).is_empty());
    }

    #[test]
    fn alpha_decides_whether_the_blockbuster_or_the_kindred_film_comes_first() {
        // Everyone (people 1-40) likes the blockbuster, 1. The four who like the quiet film,
        // 2, also like 3; so do 18 of the people who like 1 and nothing else in common.
        let mut rows: Vec<(u32, Vec<u32>)> = (1..=40).map(|u| (u, vec![1])).collect();
        for u in 1..=4 {
            rows[u as usize - 1].1.extend([2, 3]);
        }
        for u in 5..=22 {
            rows[u as usize - 1].1.push(3);
        }
        let ratings: Vec<Rating> = rows
            .iter()
            .flat_map(|(u, items)| {
                items.iter().map(move |&item| Rating {
                    user: *u,
                    item,
                    stars: 5.0,
                })
            })
            .collect();
        let l = Likes::from_ratings(ratings, 1);
        let first = |alpha: f32| build(&l, Score { alpha, shrink: 0.0 }, 10, 1).neighbours(3)[0].0;
        // Of 3's likers, everyone liked 1: with alpha 0, the blockbuster.
        assert_eq!(first(0.0), 1);
        // Lift divides that popularity out: all of 2's few likers liked 3.
        assert_eq!(first(1.0), 2);
    }

    #[test]
    fn shrinkage_distrusts_a_pair_seen_together_by_few() {
        // Both of 2's likers like 1: a perfect pair, seen twice. 1 and 3 are liked together by
        // six people, and 3 by fourteen more who like nothing else here. By hand (weights
        // 0.6445 for two likes, 0.7614 for one): cosine(1, 2) = 0.500, cosine(1, 3) = 0.447,
        // so unshrunk the pair seen twice wins; shrunk by 5, 0.102 against 0.195.
        let mut rows: Vec<(u32, Vec<u32>)> = vec![(1, vec![1, 2]), (2, vec![1, 2])];
        for u in 3..=8 {
            rows.push((u, vec![1, 3]));
        }
        for u in 9..=22 {
            rows.push((u, vec![3]));
        }
        let ratings: Vec<Rating> = rows
            .iter()
            .flat_map(|(u, items)| {
                items.iter().map(move |&item| Rating {
                    user: *u,
                    item,
                    stars: 4.0,
                })
            })
            .collect();
        let l = Likes::from_ratings(ratings, 1);
        let best = |shrink: f32| build(&l, Score { alpha: 0.5, shrink }, 10, 1).neighbours(2)[0].0;
        assert_eq!(best(0.0), 1, "2's only neighbour");
        let of_one =
            |shrink: f32| build(&l, Score { alpha: 0.5, shrink }, 10, 1).neighbours(1)[0].0;
        assert_eq!(of_one(0.0), 2, "unshrunk, the pair seen twice wins");
        assert_eq!(of_one(5.0), 3, "shrunk, the pair seen six times wins");
    }

    #[test]
    fn threads_change_nothing() {
        let rows: Vec<(u32, Vec<u32>)> = (1..=60u32)
            .map(|u| (u, (0..12u32).map(|k| (u * 7 + k * 13) % 41).collect()))
            .collect();
        let ratings: Vec<Rating> = rows
            .iter()
            .flat_map(|(u, items)| {
                items.iter().map(move |&item| Rating {
                    user: *u,
                    item,
                    stars: 4.0,
                })
            })
            .collect();
        let l = Likes::from_ratings(ratings, 1);
        let one = build(
            &l,
            Score {
                alpha: 0.7,
                shrink: 2.0,
            },
            5,
            1,
        );
        let four = build(
            &l,
            Score {
                alpha: 0.7,
                shrink: 2.0,
            },
            5,
            4,
        );
        for (id, row) in one.rows() {
            assert_eq!(row, four.neighbours(id), "item {id}");
            assert!(row.len() <= 5);
            assert!(row.windows(2).all(|w| w[0].1 >= w[1].1), "best first");
        }
    }

    #[test]
    fn a_recommendation_sums_the_liked_items_neighbours_and_skips_them() {
        let l = Likes::from_ratings(
            likes(&[
                (1, &[10, 20, 30]),
                (2, &[10, 20, 40]),
                (3, &[20, 30, 40]),
                (4, &[30, 50]),
            ]),
            1,
        );
        let m = build(&l, Score::COSINE, 10, 1);
        let got = m.recommend(&[10, 20], 2);
        assert!(!got.contains(&10) && !got.contains(&20), "{got:?}");
        assert_eq!(got.len(), 2);
        // 30 and 40 are each liked with both 10 and 20; 50 with neither.
        assert!(got.contains(&30) && got.contains(&40), "{got:?}");
        assert!(
            m.recommend(&[999], 5).is_empty(),
            "an unknown item gives nothing"
        );
    }
}
