//! Taste modes (SPEC.md Phase 15): not one average, because a person who loves both
//! Tarkovsky and slasher films is not well described by their midpoint.
//!
//! 1. **Signals, weighted and decayed** ([`weigh`]): a completed viewing 1.0, a rewatch 1.5,
//!    a rating by its stars, a watchlist entry 0.4; an abandon early is a negative, later
//!    barely one. Each fades with age (half-life 180 days; 720 for five stars and rewatches,
//!    the strongly held), so the model follows a person who changes.
//! 2. **Clustered** ([`modes`]): k-means over the liked titles' vectors (cosine), k from 3
//!    to 8 chosen by silhouette; each mode has a direction, a weight and a recency.
//! 3. **Recommended across modes** ([`interleave_by_weight`]): each mode's nearest, taken in
//!    proportion to its weight, so a person's smaller tastes are not drowned by the largest.

use std::collections::HashMap;

/// How long a signal takes to count half (days).
pub const HALF_LIFE: f64 = 180.0;
/// For five stars and rewatches: held strongly, so they fade four times slower.
pub const HALF_LIFE_STRONG: f64 = 720.0;

/// One thing a profile did with a title.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// A viewing that reached the end.
    Completed,
    /// Stopped this far in, of a film this long (seconds).
    Abandoned {
        at: f64,
        runtime: Option<f64>,
    },
    /// Rated 1 to 5.
    Rated(i64),
    Watchlisted,
}

/// What an abandon says: at 5 minutes, "not for me" (−0.6); near the end, little (−0.1).
/// Linear between, by the share of the runtime watched; without a runtime, by minutes
/// against a 100-minute film.
pub fn abandon_weight(at: f64, runtime: Option<f64>) -> f64 {
    let share = (at / runtime.filter(|r| *r > 0.0).unwrap_or(6000.0)).clamp(0.0, 1.0);
    -0.6 + 0.5 * share
}

/// A signal's weight before decay, and whether it is strongly held.
pub fn base(signal: &Signal) -> (f64, bool) {
    match signal {
        Signal::Completed => (1.0, false),
        Signal::Abandoned { at, runtime } => (abandon_weight(*at, *runtime), false),
        Signal::Rated(5) => (1.6, true),
        Signal::Rated(4) => (1.0, false),
        Signal::Rated(3) => (0.2, false),
        Signal::Rated(2) => (-0.6, false),
        Signal::Rated(_) => (-1.0, false),
        Signal::Watchlisted => (0.4, false),
    }
}

/// Each title's weight from all its signals, faded by age: (title, weight), strongest
/// first. A second completed viewing makes a rewatch (×1.5, strongly held).
/// `signals`: (title, signal, age in days).
pub fn weigh(signals: &[(i64, Signal, f64)]) -> Vec<(i64, f64)> {
    let mut completions: HashMap<i64, usize> = HashMap::new();
    for (id, s, _) in signals {
        if *s == Signal::Completed {
            *completions.entry(*id).or_default() += 1;
        }
    }
    let mut total: HashMap<i64, f64> = HashMap::new();
    for (id, s, age) in signals {
        let (mut w, mut strong) = base(s);
        if *s == Signal::Completed && completions.get(id).copied().unwrap_or(0) > 1 {
            w = 1.5;
            strong = true;
        }
        let half = if strong { HALF_LIFE_STRONG } else { HALF_LIFE };
        *total.entry(*id).or_default() += w * 0.5f64.powf(age.max(0.0) / half);
    }
    let mut out: Vec<(i64, f64)> = total.into_iter().collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// One taste mode.
#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    /// Unit length.
    pub direction: Vec<f32>,
    /// Share of the total positive weight, 0 to 1.
    pub weight: f64,
    /// The titles it was made of, strongest first.
    pub members: Vec<i64>,
}

fn unit(v: &[f32]) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n == 0.0 {
        v.to_vec()
    } else {
        v.iter().map(|x| x / n).collect()
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// k-means (cosine, weighted) with farthest-point seeding from the heaviest point, so the
/// same input gives the same modes every time.
fn kmeans(points: &[(Vec<f32>, f64)], k: usize) -> Vec<usize> {
    let mut centres: Vec<Vec<f32>> = vec![points[0].0.clone()];
    while centres.len() < k {
        let far = (0..points.len())
            .max_by(|&a, &b| {
                let d = |i: usize| {
                    centres
                        .iter()
                        .map(|c| dot(&points[i].0, c))
                        .fold(f32::MIN, f32::max)
                };
                d(b).total_cmp(&d(a))
            })
            .unwrap_or(0);
        centres.push(points[far].0.clone());
    }
    let mut assign = vec![0usize; points.len()];
    for _ in 0..25 {
        let mut changed = false;
        for (i, (p, _)) in points.iter().enumerate() {
            let best = (0..k)
                .max_by(|&a, &b| dot(p, &centres[a]).total_cmp(&dot(p, &centres[b])))
                .unwrap_or(0);
            if assign[i] != best {
                assign[i] = best;
                changed = true;
            }
        }
        for (c, centre) in centres.iter_mut().enumerate() {
            let mut sum = vec![0f32; centre.len()];
            for (i, (p, w)) in points.iter().enumerate() {
                if assign[i] == c {
                    for (s, x) in sum.iter_mut().zip(p) {
                        *s += *w as f32 * x;
                    }
                }
            }
            if sum.iter().any(|x| *x != 0.0) {
                *centre = unit(&sum);
            }
        }
        if !changed {
            break;
        }
    }
    assign
}

/// The mean silhouette (cosine distance) of a clustering: how much closer each point is to
/// its own mode than to the next nearest. Higher is a cleaner split.
pub fn silhouette(points: &[Vec<f32>], assign: &[usize], k: usize) -> f64 {
    let n = points.len();
    let mut total = 0.0;
    for i in 0..n {
        let mut sum = vec![0f64; k];
        let mut count = vec![0usize; k];
        for j in 0..n {
            if i != j {
                sum[assign[j]] += f64::from(1.0 - dot(&points[i], &points[j]));
                count[assign[j]] += 1;
            }
        }
        let own = assign[i];
        if count[own] == 0 {
            continue;
        }
        let a = sum[own] / count[own] as f64;
        let b = (0..k)
            .filter(|c| *c != own && count[*c] > 0)
            .map(|c| sum[c] / count[c] as f64)
            .fold(f64::INFINITY, f64::min);
        if b.is_finite() {
            total += (b - a) / a.max(b).max(1e-9);
        }
    }
    total / n.max(1) as f64
}

/// The modes of a taste from its positively weighted titles' vectors, strongest first.
/// Fewer than six titles: one mode, there is not enough to split.
pub fn modes(liked: &[(i64, Vec<f32>, f64)]) -> Vec<Mode> {
    let pos: Vec<&(i64, Vec<f32>, f64)> = liked
        .iter()
        .filter(|(_, v, w)| *w > 0.0 && !v.is_empty())
        .collect();
    if pos.is_empty() {
        return Vec::new();
    }
    let points: Vec<(Vec<f32>, f64)> = pos.iter().map(|(_, v, w)| (unit(v), *w)).collect();
    let total: f64 = points.iter().map(|(_, w)| w).sum();
    let build = |assign: &[usize], k: usize| -> Vec<Mode> {
        let mut out: Vec<Mode> = (0..k)
            .filter_map(|c| {
                let members: Vec<usize> = (0..points.len()).filter(|i| assign[*i] == c).collect();
                if members.is_empty() {
                    return None;
                }
                let mut sum = vec![0f32; points[0].0.len()];
                let mut w = 0.0;
                for i in &members {
                    for (s, x) in sum.iter_mut().zip(&points[*i].0) {
                        *s += points[*i].1 as f32 * x;
                    }
                    w += points[*i].1;
                }
                let mut ids: Vec<(i64, f64)> =
                    members.iter().map(|i| (pos[*i].0, points[*i].1)).collect();
                ids.sort_by(|a, b| b.1.total_cmp(&a.1));
                Some(Mode {
                    direction: unit(&sum),
                    weight: w / total,
                    members: ids.into_iter().map(|(id, _)| id).collect(),
                })
            })
            .collect();
        out.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        out
    };
    if points.len() < 6 {
        return build(&vec![0; points.len()], 1);
    }
    // Silhouette over at most 160 points spread through the list (every nth): O(n²), and
    // 160 is enough to compare splits.
    let step = (points.len() / 160).max(1);
    let picked: Vec<usize> = (0..points.len()).step_by(step).take(160).collect();
    let sample: Vec<Vec<f32>> = picked.iter().map(|i| points[*i].0.clone()).collect();
    let mut best: Option<(f64, Vec<usize>, usize)> = None;
    for k in 3..=8.min(points.len() / 2) {
        let assign = kmeans(&points, k);
        let s = silhouette(
            &sample,
            &picked.iter().map(|i| assign[*i]).collect::<Vec<_>>(),
            k,
        );
        if best.as_ref().is_none_or(|(b, _, _)| s > *b) {
            best = Some((s, assign, k));
        }
    }
    match best {
        Some((_, assign, k)) => build(&assign, k),
        None => build(&vec![0; points.len()], 1),
    }
}

/// Take from each mode's list in proportion to its weight (largest remainder), skipping
/// what an earlier mode already took, until `k`.
pub fn interleave_by_weight(lists: &[(f64, Vec<i64>)], k: usize) -> Vec<i64> {
    let total: f64 = lists.iter().map(|(w, _)| w.max(0.0)).sum::<f64>().max(1e-9);
    let mut credit = vec![0.0f64; lists.len()];
    let mut next = vec![0usize; lists.len()];
    let mut out: Vec<i64> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while out.len() < k {
        for (c, (w, _)) in credit.iter_mut().zip(lists) {
            *c += w.max(0.0) / total;
        }
        let Some(pick) = (0..lists.len())
            .filter(|i| next[*i] < lists[*i].1.len())
            // Ties go to the earlier (stronger) mode.
            .max_by(|a, b| credit[*a].total_cmp(&credit[*b]).then(b.cmp(a)))
        else {
            break;
        };
        credit[pick] -= 1.0;
        while next[pick] < lists[pick].1.len() {
            let id = lists[pick].1[next[pick]];
            next[pick] += 1;
            if seen.insert(id) {
                out.push(id);
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abandoning_at_five_minutes_means_more_than_at_eighty() {
        let early = abandon_weight(300.0, Some(6000.0));
        let late = abandon_weight(4800.0, Some(6000.0));
        assert!(
            early < late && early < -0.5 && late > -0.25,
            "{early} {late}"
        );
    }

    #[test]
    fn signals_fade_with_age_and_strong_ones_fade_slower() {
        let w = weigh(&[
            (1, Signal::Rated(4), 180.0),
            (2, Signal::Rated(5), 180.0),
            (3, Signal::Completed, 0.0),
        ]);
        let get = |id| w.iter().find(|(i, _)| *i == id).unwrap().1;
        assert!(
            (get(1) - 0.5).abs() < 1e-9,
            "four stars, one half-life old: half"
        );
        assert!(
            (get(2) - 1.6 * 0.5f64.powf(0.25)).abs() < 1e-9,
            "five stars fade four times slower"
        );
        assert_eq!(get(3), 1.0);
        // Two completed viewings: a rewatch, 1.5 each.
        let r = weigh(&[(9, Signal::Completed, 0.0), (9, Signal::Completed, 0.0)]);
        assert_eq!(r, vec![(9, 3.0)]);
        // An early abandon outweighs nothing else: it is a negative.
        assert!(
            weigh(&[(
                5,
                Signal::Abandoned {
                    at: 300.0,
                    runtime: Some(6000.0)
                },
                0.0
            )])[0]
                .1
                < 0.0
        );
    }

    #[test]
    fn two_unrelated_tastes_become_two_separate_modes() {
        // Two tight groups in different directions, plus a third smaller one.
        let mut liked = Vec::new();
        for i in 0..20 {
            let j = i as f32 * 0.01;
            liked.push((i, vec![1.0, j, 0.0, 0.0], 1.0));
            liked.push((100 + i, vec![0.0, 0.0, 1.0, j], 1.0));
        }
        for i in 0..6 {
            liked.push((200 + i, vec![0.0, 1.0, 0.0, i as f32 * 0.01], 0.5));
        }
        let m = modes(&liked);
        assert!(m.len() >= 2, "{}", m.len());
        let of = |id: i64| m.iter().position(|x| x.members.contains(&id)).unwrap();
        assert_ne!(of(0), of(100), "the two tastes are not merged");
        assert!(
            (0..20).all(|i| of(i) == of(0)) && (100..120).all(|i| of(i) == of(100)),
            "each kept whole"
        );
        assert!((m.iter().map(|x| x.weight).sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn modes_take_turns_in_proportion_to_weight() {
        let out = interleave_by_weight(
            &[(0.75, vec![1, 2, 3, 4, 5, 6]), (0.25, vec![10, 11, 12])],
            8,
        );
        assert_eq!(out.len(), 8);
        assert_eq!(out.iter().filter(|x| **x >= 10).count(), 2, "{out:?}");
        assert_eq!(
            interleave_by_weight(&[(1.0, vec![1, 2]), (1.0, vec![2, 3])], 10),
            vec![1, 2, 3]
        );
    }
}
