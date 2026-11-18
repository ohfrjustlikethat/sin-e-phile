//! Layer 3, the hybrid ranker (SPEC.md Phase 17).
//!
//! The candidates come in already described: how close each is to the profile's nearest
//! taste mode (Layer 1, the meaning index), which of the profile's liked titles point to it
//! through the item-item matrix (Layer 2), how widely seen it is, and the facts the rail's
//! rules need. This module only decides, so it is pure and fast: ranking 1,000 candidates is
//! E5's budget, under 150 ms on Tier 0.
//!
//! # How a rail is chosen
//!
//! 1. **One score per candidate**: closeness to a mode, plus how strongly titles the profile
//!    loved point to it, plus a small popularity prior, plus the profile's axes (a preferred
//!    era or length raises it, an avoided one lowers it), plus a little for what can be
//!    played right now. Each part is scaled to 0..1 over the candidates first, so the
//!    weights ([`Weights`]) say how much each part counts, not what units it came in.
//! 2. **Maximal marginal relevance**: the rail is picked one title at a time, each the best
//!    remaining score minus how much it repeats what is already picked (its closeness in
//!    meaning to the most similar pick), so a rail is not eight versions of one film.
//! 3. **Caps** ([`Caps`]): no more than two by one director, two per franchise, five from one
//!    year, so none takes more than a quarter of a rail of twenty. A title over a cap is passed
//!    over, not penalised: the rule is the rule. (Tighter caps, one per franchise and three per
//!    year, cost a sixth of the recall on MovieLens' tuning users: docs/eval-results.md.)
//! 4. **A reason** ([`Reason`]) from the part that contributed most to its score, naming only
//!    things true of the profile: the titles it loved that point here, or the mode it is
//!    near. E4: every recommendation has a truthful explanation.

use std::collections::HashMap;

/// One title that could be recommended, with everything the ranker weighs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Candidate {
    pub id: i64,
    /// The nearest taste mode and the title's closeness to it (cosine, -1..1); `None`
    /// when the title has no vector in the meaning index.
    pub near: Option<(usize, f32)>,
    /// The profile's liked titles whose stored neighbours include this one, each with the
    /// matrix's score.
    pub loved_with: Vec<(i64, f32)>,
    /// The profile's loved titles made by someone who made this one too, each with that
    /// person (a director or creator): how a title nobody has rated reaches a profile
    /// (ADR-0054 step 5).
    pub made_by: Vec<(i64, i64)>,
    /// IMDb votes: how widely seen.
    pub votes: i64,
    pub year: Option<i64>,
    pub directors: Vec<i64>,
    /// See [`franchise_key`].
    pub franchise: Option<String>,
    /// In the library, or on a configured source, now.
    pub playable: bool,
    /// The profile's axes on this title, -1 (all avoided) to 1 (all preferred).
    pub axes: f32,
    /// Its meaning vector, unit length, for the diversity step; empty when it has none.
    pub vector: Vec<f32>,
}

/// How much each part of the score counts, chosen on MovieLens' tuning users
/// (`eval recommend --whole --tune` and `--check`, docs/eval-results.md): the best Recall@20
/// of 108 settings, then, over every tuning user, the best whose coverage clears E2's 15% by a
/// point. The matrix's half weight is not a demotion: its part is the steepest, and at 1 it
/// crowded out titles near a taste mode. Popularity weighs nothing: removing its push cost no
/// recall and widened coverage from 15.6% to 16.7%; the most-voted still enter as candidates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub near: f32,
    pub loved: f32,
    /// Made by someone who made what the profile loved.
    pub people: f32,
    pub popular: f32,
    pub axes: f32,
    pub playable: f32,
}

pub const WEIGHTS: Weights = Weights {
    near: 1.0,
    loved: 0.5,
    people: 0.25,
    popular: 0.0,
    axes: 0.2,
    playable: 0.1,
};

/// At most this many per director, per franchise and per year in one rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub per_director: usize,
    pub per_franchise: usize,
    pub per_year: usize,
}

pub const CAPS: Caps = Caps {
    per_director: 2,
    per_franchise: 2,
    per_year: 5,
};

/// How much repeating an earlier pick costs, 0 (none) to 1 (only novelty counts). Chosen with
/// [`WEIGHTS`].
pub const DIVERSITY: f32 = 0.15;

/// Why a title is in the rail.
#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// Loved by people who loved these, strongest first (at most three).
    LovedWith(Vec<i64>),
    /// Made by `person`, who made these loved titles too (at most two).
    MadeBy { person: i64, loved: Vec<i64> },
    /// Close to this taste mode.
    Mode(usize),
    /// Widely loved, and the closest the profile's taste comes to anything here.
    Popular,
}

/// One title in a rail.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub id: i64,
    pub score: f32,
    pub reason: Reason,
}

/// Each part of every candidate's score, scaled to 0..1 over the candidates, and each part's
/// median: what a typical candidate gets from it.
struct Parts {
    near: Vec<f32>,
    loved: Vec<f32>,
    /// How many loved titles share a maker with it, over the most any candidate has.
    people: Vec<f32>,
    popular: Vec<f32>,
    typical: [f32; 4],
}

/// The distinct loved titles a candidate shares a maker with.
fn made_count(c: &Candidate) -> usize {
    let mut loved: Vec<i64> = c.made_by.iter().map(|(l, _)| *l).collect();
    loved.sort_unstable();
    loved.dedup();
    loved.len()
}

/// The middle value (the lower of the two middles for an even count); 0 for none.
fn median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    let mid = (v.len() - 1) / 2;
    *v.select_nth_unstable_by(mid, f32::total_cmp).1
}

fn parts(candidates: &[Candidate]) -> Parts {
    let loved_sum: Vec<f32> = candidates
        .iter()
        .map(|c| c.loved_with.iter().map(|(_, s)| s).sum())
        .collect();
    let max_loved = loved_sum.iter().copied().fold(0.0, f32::max);
    let max_votes = candidates.iter().map(|c| c.votes).max().unwrap_or(0).max(1);
    let top = ((max_votes + 1) as f32).ln();
    // Cosine -1..1 to 0..1; no vector counts as no closeness at all.
    let near: Vec<f32> = candidates
        .iter()
        .map(|c| c.near.map_or(0.0, |(_, s)| (s + 1.0) / 2.0))
        .collect();
    let loved: Vec<f32> = loved_sum
        .iter()
        .map(|s| if max_loved > 0.0 { s / max_loved } else { 0.0 })
        .collect();
    let popular: Vec<f32> = candidates
        .iter()
        .map(|c| ((c.votes.max(0) + 1) as f32).ln() / top)
        .collect();
    let max_made = candidates.iter().map(made_count).max().unwrap_or(0).max(1) as f32;
    let people: Vec<f32> = candidates
        .iter()
        .map(|c| made_count(c) as f32 / max_made)
        .collect();
    let typical = [
        median(&near),
        median(&loved),
        median(&people),
        median(&popular),
    ];
    Parts {
        near,
        loved,
        people,
        popular,
        typical,
    }
}

/// The score of candidate `i`, and the reason: the part that lifts it furthest above a
/// typical candidate.
fn scored(c: &Candidate, p: &Parts, i: usize, w: &Weights) -> (f32, Reason) {
    let near = w.near * p.near[i];
    let loved = w.loved * p.loved[i];
    let made = w.people * p.people[i];
    let popular = w.popular * p.popular[i];
    let score =
        near + loved + made + popular + w.axes * c.axes + if c.playable { w.playable } else { 0.0 };
    // WHY ABOVE A TYPICAL CANDIDATE, NOT THE BIGGEST PART. Every candidate the meaning pool
    // brings is close to some mode, so closeness is large for all of them, and as the
    // biggest part it named a mode for every pick: the real window (17.3) showed twenty of
    // twenty "Close to", Bresson's Pickpocket among them as "close to The Godfather", when
    // what put it there was the people who loved Stalker and Late Spring. A score is one
    // title against the others, so the part that ranked it is the one where it beats the
    // typical candidate by most: each part minus its median, weighted as in the score.
    let lift = |part: f32, typical: f32, weight: f32| weight * (part - typical);
    let [near_typical, loved_typical, people_typical, popular_typical] = p.typical;
    let near_lift = c.near.map(|_| lift(p.near[i], near_typical, w.near));
    let loved_lift = (loved > 0.0).then(|| lift(p.loved[i], loved_typical, w.loved));
    let made_lift = (!c.made_by.is_empty()).then(|| lift(p.people[i], people_typical, w.people));
    // "Widely loved" only of what is more widely seen than most candidates.
    let popular_lift =
        (p.popular[i] > popular_typical).then(|| lift(p.popular[i], popular_typical, w.popular));
    let beats = |a: Option<f32>, b: Option<f32>| match (a, b) {
        (Some(a), Some(b)) => a >= b,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let reason = if beats(loved_lift, near_lift)
        && beats(loved_lift, popular_lift)
        && beats(loved_lift, made_lift)
    {
        let mut by: Vec<(i64, f32)> = c.loved_with.clone();
        by.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        Reason::LovedWith(by.into_iter().take(3).map(|(id, _)| id).collect())
    } else if beats(made_lift, near_lift) && beats(made_lift, popular_lift) {
        // The person behind most of the loved titles it shares, and two of those titles.
        let mut per: HashMap<i64, Vec<i64>> = HashMap::new();
        for &(l, person) in &c.made_by {
            let titles = per.entry(person).or_default();
            if !titles.contains(&l) {
                titles.push(l);
            }
        }
        let (person, mut loved) = per
            .into_iter()
            .max_by(|a, b| a.1.len().cmp(&b.1.len()).then(b.0.cmp(&a.0)))
            .expect("made_by is not empty");
        loved.truncate(2);
        Reason::MadeBy { person, loved }
    } else if let Some((mode, _)) = c.near.filter(|_| beats(near_lift, popular_lift)) {
        Reason::Mode(mode)
    } else {
        Reason::Popular
    };
    (score, reason)
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The most of a rail of `k` each taste mode may fill: its share of the profile, the slots
/// shared out by largest remainder so they add up to `k`, and every mode at least one.
///
/// WHY: the loved part sums a candidate's neighbours over everything the profile loved, so
/// a larger taste outscores a smaller one title for title. A profile of seven arthouse
/// films and two anime got twenty arthouse films (17.6, on a copy of the development data).
/// Holding each mode to its share is calibration (Steck, "Calibrated Recommendations",
/// 2018): the rail's mix follows the profile's.
pub fn quotas(shares: &[f32], k: usize) -> Vec<usize> {
    let total: f32 = shares.iter().filter(|s| **s > 0.0).sum();
    if total <= 0.0 {
        return vec![k; shares.len()];
    }
    let exact: Vec<f32> = shares
        .iter()
        .map(|s| k as f32 * s.max(0.0) / total)
        .collect();
    let mut quota: Vec<usize> = exact.iter().map(|e| e.floor() as usize).collect();
    // The slots the floors leave go to the largest remainders.
    let mut order: Vec<usize> = (0..exact.len()).collect();
    order.sort_by(|&a, &b| {
        (exact[b] - exact[b].floor())
            .total_cmp(&(exact[a] - exact[a].floor()))
            .then(a.cmp(&b))
    });
    let mut spare = k.saturating_sub(quota.iter().sum());
    for &m in &order {
        if spare == 0 {
            break;
        }
        quota[m] += 1;
        spare -= 1;
    }
    // A mode too small for a slot of its own still gets one, from the largest.
    for m in 0..quota.len() {
        if shares[m] > 0.0 && quota[m] == 0 {
            if let Some(big) = (0..quota.len()).max_by_key(|&j| (quota[j], std::cmp::Reverse(j))) {
                if quota[big] > 1 {
                    quota[big] -= 1;
                    quota[m] = 1;
                }
            }
        }
    }
    quota
}

/// A rail of `k` from `candidates`: scored, then picked one at a time by maximal marginal
/// relevance within the caps, each taste mode within its quota ([`quotas`] of `shares`,
/// the modes' shares of the profile in the order `Candidate::near` names them; empty for
/// none). When only over-quota candidates are left, the quota gives way: a short rail
/// would serve nobody.
pub fn rank(
    candidates: &[Candidate],
    weights: &Weights,
    caps: &Caps,
    diversity: f32,
    shares: &[f32],
    k: usize,
) -> Vec<Pick> {
    if candidates.is_empty() || k == 0 {
        return Vec::new();
    }
    let p = parts(candidates);
    let scores: Vec<(f32, Reason)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| scored(c, &p, i, weights))
        .collect();
    // Scores to 0..1, so the diversity penalty (a cosine, 0..1 here) is on the same scale.
    let (lo, hi) = scores
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), (s, _)| {
            (lo.min(*s), hi.max(*s))
        });
    let span = (hi - lo).max(1e-6);

    let mut left: Vec<usize> = (0..candidates.len()).collect();
    // Best first, so equal marginal relevance falls to the better score, then the lower id.
    left.sort_by(|&a, &b| {
        scores[b]
            .0
            .total_cmp(&scores[a].0)
            .then(candidates[a].id.cmp(&candidates[b].id))
    });
    let mut picked: Vec<usize> = Vec::new();
    // Each candidate's closeness to the most similar pick so far, kept as picks are made:
    // comparing every candidate with every pick again in each round cost 26 ms for a
    // thousand candidates; updating with the newest pick alone gives the same answer.
    let mut repeat = vec![0.0f32; candidates.len()];
    let mut directors: HashMap<i64, usize> = HashMap::new();
    let mut franchises: HashMap<&str, usize> = HashMap::new();
    let mut years: HashMap<i64, usize> = HashMap::new();
    let quota = quotas(shares, k);
    let mut per_mode = vec![0usize; quota.len()];
    let over_quota = |c: &Candidate, per_mode: &[usize]| {
        c.near
            .is_some_and(|(m, _)| quota.get(m).is_some_and(|q| per_mode[m] >= *q))
    };
    while picked.len() < k {
        let mut best: Option<(f32, usize)> = None;
        let mut best_over: Option<(f32, usize)> = None;
        for (slot, &i) in left.iter().enumerate() {
            let c = &candidates[i];
            let over = c
                .directors
                .iter()
                .any(|d| directors.get(d).copied().unwrap_or(0) >= caps.per_director)
                || c.franchise
                    .as_deref()
                    .is_some_and(|f| franchises.get(f).copied().unwrap_or(0) >= caps.per_franchise)
                || c.year
                    .is_some_and(|y| years.get(&y).copied().unwrap_or(0) >= caps.per_year);
            if over {
                continue;
            }
            let value = (1.0 - diversity) * (scores[i].0 - lo) / span - diversity * repeat[i];
            let into = if over_quota(c, &per_mode) {
                &mut best_over
            } else {
                &mut best
            };
            if into.is_none_or(|(v, _)| value > v) {
                *into = Some((value, slot));
            }
        }
        let Some((_, slot)) = best.or(best_over) else {
            break;
        };
        let i = left.remove(slot);
        let c = &candidates[i];
        if diversity > 0.0 {
            for &j in &left {
                let s = cosine(&candidates[j].vector, &c.vector).max(0.0);
                if s > repeat[j] {
                    repeat[j] = s;
                }
            }
        }
        for d in &c.directors {
            *directors.entry(*d).or_default() += 1;
        }
        if let Some(f) = c.franchise.as_deref() {
            *franchises.entry(f).or_default() += 1;
        }
        if let Some(y) = c.year {
            *years.entry(y).or_default() += 1;
        }
        if let Some(n) = c.near.and_then(|(m, _)| per_mode.get_mut(m)) {
            *n += 1;
        }
        picked.push(i);
    }
    picked
        .into_iter()
        .map(|i| Pick {
            id: candidates[i].id,
            score: scores[i].0,
            reason: scores[i].1.clone(),
        })
        .collect()
}

/// A title's franchise, as far as the title alone says: the part before a colon or a dash,
/// a leading "The" dropped, a sequel's number or "Part …" dropped, then its first two words.
/// "Toy Story 3" and "Toy Story" share "toy story"; "Star Wars: Episode IV" is "star wars".
/// A proxy: no free keyless source names franchises, and two unrelated films can share a
/// key, which costs one of them a place in a rail, never a wrong reason. `None` for a title
/// with nothing left.
pub fn franchise_key(title: &str) -> Option<String> {
    let head = title.split([':', '–', '—']).next().unwrap_or(title);
    let head = head.split(" - ").next().unwrap_or(head);
    let mut words: Vec<String> = head
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    if words.first().is_some_and(|w| w == "the") {
        words.remove(0);
    }
    let sequel = |w: &str| {
        w.chars().all(|c| c.is_ascii_digit())
            || matches!(w, "ii" | "iii" | "iv" | "v" | "vi" | "part" | "chapter")
    };
    while words.last().is_some_and(|w| sequel(w)) {
        words.pop();
    }
    if let Some(at) = words.iter().position(|w| w == "part" || w == "chapter") {
        words.truncate(at);
    }
    words.truncate(2);
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn film(id: i64) -> Candidate {
        Candidate {
            id,
            votes: 1_000,
            ..Default::default()
        }
    }

    #[test]
    fn a_title_loved_alongside_what_you_loved_ranks_first_and_says_why() {
        let strong = Candidate {
            loved_with: vec![(901, 0.4), (902, 0.9), (903, 0.2), (904, 0.1)],
            near: Some((0, 0.2)),
            ..film(1)
        };
        let near_only = Candidate {
            near: Some((1, 0.6)),
            ..film(2)
        };
        let rail = rank(&[near_only, strong], &WEIGHTS, &CAPS, 0.0, &[], 2);
        assert_eq!(rail[0].id, 1);
        // The three strongest, strongest first.
        assert_eq!(rail[0].reason, Reason::LovedWith(vec![902, 901, 903]));
        assert_eq!(rail[1].reason, Reason::Mode(1));
    }

    #[test]
    fn a_reason_is_what_lifts_a_title_above_the_typical_candidate() {
        // 17.3's real window: ten titles from the meaning pool, all close to a mode, and a
        // title loved alongside what the profile loved that is only loosely close to one
        // (Pickpocket, said to be "close to The Godfather"). Closeness is what every
        // candidate has; what this one has that the others do not is the people.
        let mut c: Vec<Candidate> = (10..20)
            .map(|i| Candidate {
                near: Some((0, 0.6)),
                ..film(i)
            })
            .collect();
        c.push(Candidate {
            near: Some((1, 0.35)),
            loved_with: vec![(901, 0.6)],
            ..film(1)
        });
        c.push(Candidate {
            loved_with: vec![(902, 1.0)],
            ..film(2)
        });
        let rail = rank(&c, &WEIGHTS, &CAPS, 0.0, &[], c.len());
        let reason = |id: i64| rail.iter().find(|p| p.id == id).unwrap().reason.clone();
        assert_eq!(reason(1), Reason::LovedWith(vec![901]));
        assert_eq!(reason(2), Reason::LovedWith(vec![902]));
        for id in 10..20 {
            assert_eq!(reason(id), Reason::Mode(0), "{id}");
        }
    }

    #[test]
    fn each_taste_mode_fills_no_more_than_its_share_of_the_rail() {
        // Seven loved films to two loved anime: thirty strong film candidates loved beside
        // several loved titles, ten weaker anime ones loved beside one. Without quotas the
        // films take all eight; with them the anime get the share the profile gives them.
        let mut c: Vec<Candidate> = (1..=30)
            .map(|i| Candidate {
                near: Some((0, 0.8)),
                loved_with: vec![(901, 0.3), (902, 0.3), (903, 0.3)],
                ..film(i)
            })
            .collect();
        c.extend((100..110).map(|i| Candidate {
            near: Some((1, 0.8)),
            loved_with: vec![(904, 0.29)],
            ..film(i)
        }));
        let anime = |rail: &[Pick]| rail.iter().filter(|p| p.id >= 100).count();
        assert_eq!(anime(&rank(&c, &WEIGHTS, &CAPS, 0.0, &[], 8)), 0);
        let shares = [7.0 / 9.0, 2.0 / 9.0];
        assert_eq!(quotas(&shares, 8), vec![6, 2]);
        let rail = rank(&c, &WEIGHTS, &CAPS, 0.0, &shares, 8);
        assert_eq!(rail.len(), 8);
        assert_eq!(anime(&rail), 2, "{rail:?}");
        // Only one mode has candidates left: its quota gives way rather than leave the
        // rail short.
        let films_only: Vec<Candidate> = c.iter().filter(|x| x.id < 100).cloned().collect();
        assert_eq!(rank(&films_only, &WEIGHTS, &CAPS, 0.0, &shares, 8).len(), 8);
        // A mode the rail cannot hold a share of still gets one slot.
        assert_eq!(quotas(&[0.98, 0.02], 20), vec![19, 1]);
        assert_eq!(quotas(&[7.0, 2.0], 20).iter().sum::<usize>(), 20);
        assert_eq!(quotas(&[], 20), Vec::<usize>::new());
    }

    #[test]
    fn no_more_than_two_by_one_director_two_per_franchise_five_per_year() {
        let mut c: Vec<Candidate> = (1..=6)
            .map(|i| Candidate {
                directors: vec![77],
                near: Some((0, 0.9 - i as f32 * 0.01)),
                ..film(i)
            })
            .collect();
        c.extend((10..=13).map(|i| Candidate {
            franchise: franchise_key(&format!("Toy Story {}", i - 9)),
            near: Some((0, 0.5)),
            ..film(i)
        }));
        c.extend((20..=25).map(|i| Candidate {
            year: Some(1999),
            near: Some((0, 0.4)),
            ..film(i)
        }));
        let rail = rank(&c, &WEIGHTS, &CAPS, 0.0, &[], 20);
        let ids: Vec<i64> = rail.iter().map(|p| p.id).collect();
        assert_eq!(
            ids.iter().filter(|i| (1..=6).contains(*i)).count(),
            2,
            "{ids:?}"
        );
        assert_eq!(
            ids.iter().filter(|i| (10..=13).contains(*i)).count(),
            2,
            "{ids:?}"
        );
        assert_eq!(
            ids.iter().filter(|i| (20..=25).contains(*i)).count(),
            5,
            "{ids:?}"
        );
        // The two kept by the director are his best two.
        assert_eq!(&ids[..2], &[1, 2]);
    }

    #[test]
    fn diversity_passes_over_a_near_copy_of_what_is_already_picked() {
        let v = |x: f32, y: f32| {
            let n = (x * x + y * y).sqrt();
            vec![x / n, y / n]
        };
        let first = Candidate {
            near: Some((0, 0.9)),
            vector: v(1.0, 0.0),
            ..film(1)
        };
        let copy = Candidate {
            near: Some((0, 0.88)),
            vector: v(1.0, 0.01),
            ..film(2)
        };
        let other = Candidate {
            near: Some((0, 0.8)),
            vector: v(0.0, 1.0),
            ..film(3)
        };
        // A weak title sets the scale, as the rest of a real candidate list does: scores
        // are scaled over the candidates, and three near-equal ones alone would stretch a
        // 0.04 difference across the whole range.
        let weak = Candidate {
            near: Some((0, -0.5)),
            vector: v(-1.0, 0.0),
            ..film(4)
        };
        let c = [first, copy, other, weak];
        let plain: Vec<i64> = rank(&c, &WEIGHTS, &CAPS, 0.0, &[], 2)
            .iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(plain, vec![1, 2]);
        let varied: Vec<i64> = rank(&c, &WEIGHTS, &CAPS, DIVERSITY, &[], 2)
            .iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(varied, vec![1, 3]);
    }

    #[test]
    fn what_plays_now_wins_a_tie_and_an_avoided_axis_loses_one() {
        let a = Candidate {
            near: Some((0, 0.5)),
            ..film(1)
        };
        let playable = Candidate {
            playable: true,
            ..a.clone()
        };
        let avoided = Candidate {
            axes: -1.0,
            ..film(3)
        };
        let ids: Vec<i64> = rank(
            &[
                a,
                Candidate { id: 2, ..playable },
                Candidate {
                    near: Some((0, 0.5)),
                    ..avoided
                },
            ],
            &WEIGHTS,
            &CAPS,
            0.0,
            &[],
            3,
        )
        .iter()
        .map(|p| p.id)
        .collect();
        assert_eq!(ids, vec![2, 1, 3]);
    }

    #[test]
    fn every_reason_names_something_true_of_the_candidate() {
        // E4, as a property over many mixed candidates: a LovedWith reason lists only liked
        // titles that point here; a Mode reason only the mode the title is near.
        let candidates: Vec<Candidate> = (0..300i64)
            .map(|i| Candidate {
                id: i,
                near: (i % 3 != 0).then(|| ((i % 4) as usize, ((i * 37) % 100) as f32 / 100.0)),
                loved_with: (0..(i % 5))
                    .map(|k| (1_000 + k, ((i + k) % 7) as f32 / 7.0))
                    .collect(),
                made_by: (0..(i % 4))
                    .map(|k| (2_000 + k, 3_000 + (i + k) % 3))
                    .collect(),
                votes: (i * 9_973) % 2_000_000,
                year: Some(1950 + i % 70),
                directors: vec![i % 40],
                ..Default::default()
            })
            .collect();
        let rail = rank(&candidates, &WEIGHTS, &CAPS, DIVERSITY, &[], 60);
        assert_eq!(rail.len(), 60);
        for pick in &rail {
            let c = candidates.iter().find(|c| c.id == pick.id).unwrap();
            match &pick.reason {
                Reason::LovedWith(ids) => {
                    assert!(!ids.is_empty() && ids.len() <= 3);
                    for id in ids {
                        assert!(c.loved_with.iter().any(|(l, _)| l == id), "{pick:?}");
                    }
                }
                Reason::Mode(m) => assert_eq!(c.near.map(|(mode, _)| mode), Some(*m), "{pick:?}"),
                Reason::MadeBy { person, loved } => {
                    assert!(!loved.is_empty() && loved.len() <= 2);
                    for l in loved {
                        assert!(c.made_by.contains(&(*l, *person)), "{pick:?}");
                    }
                }
                Reason::Popular => {}
            }
        }
    }

    #[test]
    fn a_title_nobody_has_rated_reaches_the_profile_through_its_maker() {
        // ADR-0054 step 5: a new film with no neighbours and a weak vector, by the director
        // of two loved films, among candidates close to a mode. It enters the rail, and
        // says who made it and what of theirs the profile loved.
        let mut c: Vec<Candidate> = (1..=20)
            .map(|i| Candidate {
                near: Some((0, 0.5)),
                ..film(i)
            })
            .collect();
        c.push(Candidate {
            near: Some((0, 0.3)),
            made_by: vec![(901, 77), (902, 77), (903, 78)],
            year: Some(2026),
            ..film(100)
        });
        let rail = rank(&c, &WEIGHTS, &CAPS, 0.0, &[], 5);
        let new = rail.iter().find(|p| p.id == 100).expect("in the rail");
        assert_eq!(
            new.reason,
            Reason::MadeBy {
                person: 77,
                loved: vec![901, 902]
            }
        );
        // With no weight on it, the maker does not lift it past the closer ones.
        let none = Weights {
            people: 0.0,
            ..WEIGHTS
        };
        assert!(!rank(&c, &none, &CAPS, 0.0, &[], 5)
            .iter()
            .any(|p| p.id == 100));
    }

    #[test]
    fn a_franchise_is_read_from_the_title() {
        assert_eq!(franchise_key("Toy Story 3").as_deref(), Some("toy story"));
        assert_eq!(franchise_key("Toy Story").as_deref(), Some("toy story"));
        assert_eq!(
            franchise_key("Star Wars: Episode IV - A New Hope").as_deref(),
            Some("star wars")
        );
        assert_eq!(
            franchise_key("The Godfather Part II").as_deref(),
            Some("godfather")
        );
        assert_eq!(
            franchise_key("The Dark Knight Rises").as_deref(),
            Some("dark knight")
        );
        assert_eq!(franchise_key("Alien"), Some("alien".into()));
        assert_eq!(franchise_key("???"), None);
    }

    #[test]
    fn a_thousand_candidates_rank_well_inside_the_budget() {
        // E5's budget is 150 ms on Tier 0; this is a debug build on any machine, so it only
        // guards against the ranker becoming quadratic in something larger than the rail.
        let candidates: Vec<Candidate> = (0..1_000i64)
            .map(|i| Candidate {
                id: i,
                near: Some((0, (i % 97) as f32 / 97.0)),
                vector: (0..384)
                    .map(|d| (((i * 31 + d) % 17) as f32 - 8.0) / 40.0)
                    .collect(),
                ..film(i)
            })
            .collect();
        let started = std::time::Instant::now();
        let rail = rank(&candidates, &WEIGHTS, &CAPS, DIVERSITY, &[], 20);
        assert_eq!(rail.len(), 20);
        assert!(
            started.elapsed().as_millis() < 1_000,
            "{:?}",
            started.elapsed()
        );
    }
}
