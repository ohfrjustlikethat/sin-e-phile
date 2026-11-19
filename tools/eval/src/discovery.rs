//! `eval discovery`: Phase 18's harnesses.
//!
//! `--simulate` (E1, E2): the bandit (`sinephile_recommend::bandit`) against simulated
//! viewers whose rewards per arm are known, day by day, exactly as Home asks it: one
//! allocation of 60 slots a day (the taste rail shows at most 20 of the familiar's), and
//! the viewer acts on three cards, each drawn from what was shown, finishing it with their
//! chance for its arm and otherwise saying "not for me". Each day's beliefs are rebuilt from
//! every outcome so far, faded by age, as the app does.
//!
//! The verdicts were fixed before the first run (docs/eval-results.md, 18.2):
//!
//! - **E1**: 1,000 viewers, each with a favourite discovery arm (chance 0.7, the other
//!   arms 0.15, the familiar 0.4) that changes to a different one on day 61. Met when the
//!   new favourite's mean share of discovery's slots over days 101 to 120 is at least 15
//!   points above its share over days 41 to 60, the old favourite's at least 15 points
//!   below, and the new favourite's share rose for at least 80% of viewers.
//! - **E2**: 1,000 maximally conservative viewers (Comfort, every familiar card finished
//!   with chance 0.9, every discovery card "not for me") for 120 days. Met when every day of
//!   every viewer gives discovery at least 20% of the slots and at least one rail's worth,
//!   and every discovery arm is shown on at least 10% of each viewer's days.

use std::collections::HashMap;

use sinephile_recommend::bandit::{self, Beliefs, Context, Guards, Outcome, Rng, Trial, GUARDS};
use sinephile_recommend::strategy::Arm;

use crate::error::EvalError;

const DAYS: usize = 120;
const SWITCH: usize = 60;
const TOTAL: usize = 60;
/// A rail's worth: Home leaves out a discovery rail shorter than this.
const LEAST: usize = 6;
/// The most of the familiar's slots the taste rail shows.
const TASTE_RAIL: usize = 20;
const ACTS_PER_DAY: usize = 3;
const VIEWERS: u64 = 1_000;

fn discovery_arms() -> Vec<Arm> {
    Arm::ALL
        .into_iter()
        .filter(|a| *a != Arm::Familiar)
        .collect()
}

/// One simulated viewer: their chance of finishing a card from each arm, before and after
/// day `SWITCH`, and their budget.
struct Viewer {
    before: HashMap<Arm, f64>,
    after: HashMap<Arm, f64>,
    budget: f64,
}

/// Each day's allocation for one viewer.
fn live(viewer: &Viewer, seed: u64, guards: &Guards, learn: bool) -> Vec<Vec<(Arm, usize)>> {
    let mut rng = Rng::new(seed);
    let mut log: Vec<(usize, Arm, Outcome)> = Vec::new();
    let mut days = Vec::with_capacity(DAYS);
    for day in 0..DAYS {
        let trials: Vec<Trial> = log
            .iter()
            .map(|(d, arm, outcome)| Trial {
                arm: *arm,
                context: Context::Evening,
                outcome: *outcome,
                age_days: (day - d) as f64,
            })
            .collect();
        let beliefs = if learn {
            Beliefs::from_trials(&trials)
        } else {
            Beliefs::default()
        };
        let plan = bandit::allocate_within(
            &beliefs,
            Context::Evening,
            viewer.budget,
            TOTAL,
            LEAST,
            guards,
            &mut rng,
        );
        let chances = if day < SWITCH {
            &viewer.before
        } else {
            &viewer.after
        };
        let shown: Vec<Arm> = plan
            .iter()
            .flat_map(|(arm, n)| {
                let n = if *arm == Arm::Familiar {
                    (*n).min(TASTE_RAIL)
                } else {
                    *n
                };
                std::iter::repeat_n(*arm, n)
            })
            .collect();
        for _ in 0..ACTS_PER_DAY {
            let arm = shown[((rng.unit() * shown.len() as f64) as usize).min(shown.len() - 1)];
            let outcome = if rng.unit() < chances[&arm] {
                Outcome::Completed
            } else {
                Outcome::NotForMe
            };
            log.push((day, arm, outcome));
        }
        days.push(plan);
    }
    days
}

/// An arm's share of discovery's slots over days `from..to`.
fn share(days: &[Vec<(Arm, usize)>], arm: Arm, from: usize, to: usize) -> f64 {
    let (mut mine, mut all) = (0usize, 0usize);
    for plan in &days[from..to] {
        for (a, n) in plan {
            if *a != Arm::Familiar {
                all += n;
                if *a == arm {
                    mine += n;
                }
            }
        }
    }
    mine as f64 / all.max(1) as f64
}

fn chances(familiar: f64, rest: f64, favourite: Option<(Arm, f64)>) -> HashMap<Arm, f64> {
    Arm::ALL
        .into_iter()
        .map(|a| {
            let p = match favourite {
                _ if a == Arm::Familiar => familiar,
                Some((f, p)) if f == a => p,
                _ => rest,
            };
            (a, p)
        })
        .collect()
}

pub fn simulate(no_floor: bool, frozen: bool) -> Result<bool, EvalError> {
    let guards = if no_floor {
        Guards {
            floor: 0.0,
            ..GUARDS
        }
    } else {
        GUARDS
    };
    let arms = discovery_arms();
    println!(
        "discovery --simulate: {VIEWERS} viewers, {DAYS} days, {TOTAL} slots a day, {ACTS_PER_DAY} cards acted on{}{}\n",
        if no_floor { ", NO FLOOR" } else { "" },
        if frozen { ", NOT LEARNING" } else { "" }
    );

    // E1: a favourite that changes.
    let (mut old_before, mut old_after, mut new_before, mut new_after) = (0.0, 0.0, 0.0, 0.0);
    let mut rose = 0;
    let mut by_arm: HashMap<Arm, (f64, f64, usize)> = HashMap::new();
    for seed in 0..VIEWERS {
        let mut pick = Rng::new(seed ^ 0x5eed);
        let first = arms[(pick.unit() * arms.len() as f64) as usize % arms.len()];
        let others: Vec<Arm> = arms.iter().copied().filter(|a| *a != first).collect();
        let second = others[(pick.unit() * others.len() as f64) as usize % others.len()];
        let viewer = Viewer {
            before: chances(0.4, 0.15, Some((first, 0.7))),
            after: chances(0.4, 0.15, Some((second, 0.7))),
            budget: 0.5,
        };
        let days = live(&viewer, seed, &guards, !frozen);
        let nb = share(&days, second, 40, 60);
        let na = share(&days, second, 100, 120);
        old_before += share(&days, first, 40, 60);
        old_after += share(&days, first, 100, 120);
        new_before += nb;
        new_after += na;
        if na > nb {
            rose += 1;
        }
        let e = by_arm.entry(second).or_default();
        e.0 += nb;
        e.1 += na;
        e.2 += 1;
    }
    let n = VIEWERS as f64;
    let (old_before, old_after, new_before, new_after) =
        (old_before / n, old_after / n, new_before / n, new_after / n);
    println!(
        "E1: the favourite discovery arm changes on day {}",
        SWITCH + 1
    );
    println!(
        "  old favourite's share of discovery: days 41-60 {:.1}%, days 101-120 {:.1}% ({:+.1} points)",
        old_before * 100.0,
        old_after * 100.0,
        (old_after - old_before) * 100.0
    );
    println!(
        "  new favourite's share of discovery: days 41-60 {:.1}%, days 101-120 {:.1}% ({:+.1} points)",
        new_before * 100.0,
        new_after * 100.0,
        (new_after - new_before) * 100.0
    );
    println!(
        "  the new favourite's share rose for {rose} of {VIEWERS} viewers ({:.1}%)",
        rose as f64 / n * 100.0
    );
    for arm in &arms {
        if let Some((b, a, k)) = by_arm.get(arm) {
            println!(
                "    {:>10} as the new favourite ({k} viewers): {:.1}% -> {:.1}%",
                arm.key(),
                b / *k as f64 * 100.0,
                a / *k as f64 * 100.0
            );
        }
    }
    let e1 =
        new_after - new_before >= 0.15 && old_before - old_after >= 0.15 && rose as f64 / n >= 0.80;

    // A viewer who warms to discovery: the share discovery gets follows.
    let mut warm = (0.0, 0.0);
    for seed in 0..VIEWERS {
        let viewer = Viewer {
            before: chances(0.7, 0.1, None),
            after: chances(0.2, 0.6, None),
            budget: 0.5,
        };
        let days = live(&viewer, seed + 50_000, &guards, !frozen);
        let disc = |from: usize, to: usize| {
            let (mut d, mut all) = (0usize, 0usize);
            for plan in &days[from..to] {
                for (a, n) in plan {
                    all += n;
                    if *a != Arm::Familiar {
                        d += n;
                    }
                }
            }
            d as f64 / all.max(1) as f64
        };
        warm.0 += disc(40, 60);
        warm.1 += disc(100, 120);
    }
    println!(
        "  a viewer who warms to discovery on day {}: discovery's share {:.1}% -> {:.1}%",
        SWITCH + 1,
        warm.0 / n * 100.0,
        warm.1 / n * 100.0
    );

    // E2: the maximally conservative viewer.
    let mut least_share: f64 = 1.0;
    let mut days_without_rail = 0usize;
    let mut least_arm_days: f64 = 1.0;
    let mut mean_share = 0.0;
    for seed in 0..VIEWERS {
        let viewer = Viewer {
            before: chances(0.9, 0.0, None),
            after: chances(0.9, 0.0, None),
            budget: 0.0,
        };
        let days = live(&viewer, seed + 100_000, &guards, !frozen);
        let mut seen: HashMap<Arm, usize> = HashMap::new();
        for plan in &days {
            let disc: usize = plan
                .iter()
                .filter(|(a, _)| *a != Arm::Familiar)
                .map(|(_, n)| n)
                .sum();
            let s = disc as f64 / TOTAL as f64;
            least_share = least_share.min(s);
            mean_share += s;
            if !plan.iter().any(|(a, n)| *a != Arm::Familiar && *n >= LEAST) {
                days_without_rail += 1;
            }
            for (a, n) in plan {
                if *a != Arm::Familiar && *n > 0 {
                    *seen.entry(*a).or_default() += 1;
                }
            }
        }
        for arm in &arms {
            let d = seen.get(arm).copied().unwrap_or(0) as f64 / DAYS as f64;
            least_arm_days = least_arm_days.min(d);
        }
    }
    mean_share /= n * DAYS as f64;
    println!(
        "\nE2: a viewer who only ever takes the familiar (Comfort, every discovery card refused)"
    );
    println!(
        "  discovery's share of the slots: least on any day {:.1}%, mean {:.1}%",
        least_share * 100.0,
        mean_share * 100.0
    );
    println!("  days with no discovery rail: {days_without_rail}");
    println!(
        "  the least any discovery arm was shown, as a share of one viewer's days: {:.1}%",
        least_arm_days * 100.0
    );
    let e2 = least_share >= 0.20 - 1e-9 && days_without_rail == 0 && least_arm_days >= 0.10;

    let said = |ok: bool| if ok { "met" } else { "NOT met" };
    println!(
        "\n  E1, the distribution follows a change of reward: {}",
        said(e1)
    );
    println!("  E2, the exploration floor holds: {}", said(e2));
    Ok(e1 && e2)
}

/// Self-information of a title, `-log2(p)`, `p` the share of build users who liked it;
/// `None` for a title none liked (left out of every mean, docs/eval-results.md).
fn novelty(id: i64, liked_by: &HashMap<i64, u32>, build_users: usize) -> Option<f64> {
    let n = *liked_by.get(&id)?;
    (n > 0).then(|| -(n as f64 / build_users as f64).log2())
}

/// Running mean of novelty, and how many titles had none.
#[derive(Default, Clone, Copy)]
struct Novel {
    sum: f64,
    counted: usize,
    unliked: usize,
}

impl Novel {
    fn add(&mut self, v: Option<f64>) {
        match v {
            Some(v) => {
                self.sum += v;
                self.counted += 1;
            }
            None => self.unliked += 1,
        }
    }
    fn mean(&self) -> f64 {
        self.sum / self.counted.max(1) as f64
    }
    fn unliked_share(&self) -> f64 {
        self.unliked as f64 / (self.counted + self.unliked).max(1) as f64
    }
}

/// E4 (and E3 on real users): Home as a new profile sees it, on MovieLens test users the
/// matrix never learned from. The taste rail is Phase 17's (the app's ranker on the app's
/// candidates); the discovery rails are the bandit's first allocation (no outcomes yet, the
/// budget at 0.5, 60 slots, the taste rail at most 20), each arm choosing in the plan's
/// order. Novelty of the discovery picks against the taste rail's; Recall@N of everything
/// shown against popularity's top N; and every discovery label checked with
/// `strategy::holds`.
pub async fn novelty_run(
    data_dir: &std::path::Path,
    limit: Option<usize>,
) -> Result<bool, EvalError> {
    use sinephile_recommend::rank;
    use sinephile_recommend::split::Part;
    use sinephile_recommend::strategy;
    use sinephile_taste::{discover, hybrid};
    use std::collections::HashSet;
    use std::time::Instant;

    println!("discovery: Home as a new profile sees it, MovieLens test users\n");
    let db = sinephile_persistence::Db::open_in(data_dir).await?;
    let (mut engine, _) = crate::search::engine(data_dir)?;
    let semantic = engine
        .semantic_mut()
        .ok_or_else(|| EvalError::Missing("the meaning index and model are needed".into()))?;
    let j = crate::recommend::judging(data_dir, &db, Part::Test, limit).await?;
    println!(
        "  {} build users liked {} catalogue titles; judged on {} users",
        j.build_users,
        j.liked_by.len(),
        j.users.len()
    );

    let mut cache = discover::Cache::default();
    let (mut taste_n, mut disc_n) = (Novel::default(), Novel::default());
    let mut per_arm: HashMap<Arm, Novel> = HashMap::new();
    let mut picks_per_arm: HashMap<Arm, usize> = HashMap::new();
    let mut rails_per_arm: HashMap<Arm, usize> = HashMap::new();
    let (mut recall_all, mut recall_pop, mut recall_taste) = (0.0, 0.0, 0.0);
    // Diagnostics: hits per card shown, by where it came from; and Phase 17's ranker asked
    // for as many as Home shows, the ceiling a relevance-only Home would reach.
    let (mut hits_taste, mut cards_taste, mut hits_disc, mut cards_disc) =
        (0usize, 0usize, 0usize, 0usize);
    let (mut hits_pop, mut cards_pop) = (0usize, 0usize);
    let mut recall_ranker_n = 0.0;
    let mut shown_total = 0usize;
    let mut labels = 0usize;
    let mut false_labels = 0usize;
    let mut gather_ms: Vec<f64> = Vec::new();
    let started = Instant::now();
    let recall = |top: &[i64], held: &HashSet<i64>, k: usize| {
        let hits = top.iter().filter(|i| held.contains(i)).count();
        hits as f64 / k.min(held.len()).max(1) as f64
    };
    for (user, shown, held) in &j.users {
        let met: HashSet<i64> = shown.iter().copied().collect();
        let held: HashSet<i64> = held.iter().copied().collect();
        let liked: Vec<(i64, f64)> = shown.iter().map(|id| (*id, 1.0)).collect();
        let with_vectors: Vec<(i64, Vec<f32>, f64)> = shown
            .iter()
            .filter_map(|id| {
                let v = semantic.index.vector(*id)?;
                Some((*id, v.iter().map(|x| f32::from(*x)).collect(), 1.0))
            })
            .collect();
        let modes = sinephile_taste::modes::modes(&with_vectors);
        let mut loved: HashMap<i64, Vec<(i64, f32)>> = HashMap::new();
        for from in hybrid::strongest(&liked) {
            let Ok(from32) = u32::try_from(from) else {
                continue;
            };
            for &(to, score) in j.matrix.neighbours(from32) {
                let to = i64::from(to);
                if !met.contains(&to) {
                    loved.entry(to).or_default().push((from, score));
                }
            }
        }
        let axes =
            sinephile_taste::axes::axes(&db, &liked, &sinephile_taste::Statements::default())
                .await?;
        let (made, _) = hybrid::made_alike(&db, &liked, &met).await?;
        let candidates = hybrid::candidates(
            &db,
            semantic,
            &modes,
            &met,
            &loved,
            &made,
            &axes,
            &HashSet::new(),
        )
        .await?;
        let shares: Vec<f32> = modes.iter().map(|m| m.weight as f32).collect();
        let taste: Vec<i64> = rank::rank(
            &candidates,
            &rank::WEIGHTS,
            &rank::CAPS,
            rank::DIVERSITY,
            &shares,
            TASTE_RAIL,
        )
        .into_iter()
        .map(|p| p.id)
        .collect();

        let t = Instant::now();
        let gathered = discover::gather(&db, semantic, &modes, &met, &loved, &mut cache).await?;
        gather_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        let plan = bandit::allocate(
            &Beliefs::default(),
            Context::Evening,
            0.5,
            TOTAL,
            LEAST,
            &mut Rng::new(u64::from(*user)),
        );
        let familiar = plan[0].1.min(TASTE_RAIL).min(taste.len());
        let mut taken: HashSet<i64> = met.clone();
        taken.extend(taste[..familiar].iter().copied());
        let rails = discover::planned(&gathered, &plan, &taken);
        let by_id: HashMap<i64, &strategy::Title> =
            gathered.titles.iter().map(|t| (t.id, t)).collect();

        for id in &taste {
            taste_n.add(novelty(*id, &j.liked_by, j.build_users));
        }
        let mut everything: Vec<i64> = taste[..familiar].to_vec();
        for rail in &rails {
            *rails_per_arm.entry(rail.arm).or_default() += 1;
            for d in &rail.picks {
                let v = novelty(d.id, &j.liked_by, j.build_users);
                disc_n.add(v);
                per_arm.entry(rail.arm).or_default().add(v);
                *picks_per_arm.entry(rail.arm).or_default() += 1;
                labels += 1;
                if !by_id
                    .get(&d.id)
                    .is_some_and(|t| strategy::holds(&d.label, t, &gathered.profile))
                {
                    false_labels += 1;
                }
                everything.push(d.id);
            }
        }
        let n = everything.len();
        shown_total += n;
        recall_all += recall(&everything, &held, n);
        recall_taste += recall(&taste, &held, TASTE_RAIL);
        let pop: Vec<i64> = j
            .popular
            .iter()
            .filter(|i| !met.contains(i))
            .take(n)
            .copied()
            .collect();
        recall_pop += recall(&pop, &held, n);
        hits_taste += everything[..familiar]
            .iter()
            .filter(|i| held.contains(i))
            .count();
        cards_taste += familiar;
        hits_disc += everything[familiar..]
            .iter()
            .filter(|i| held.contains(i))
            .count();
        cards_disc += n - familiar;
        hits_pop += pop.iter().filter(|i| held.contains(i)).count();
        cards_pop += pop.len();
        let ranked_n: Vec<i64> = rank::rank(
            &candidates,
            &rank::WEIGHTS,
            &rank::CAPS,
            rank::DIVERSITY,
            &shares,
            n,
        )
        .into_iter()
        .map(|p| p.id)
        .collect();
        recall_ranker_n += recall(&ranked_n, &held, n);
    }
    let users = j.users.len().max(1) as f64;
    gather_ms.sort_by(f64::total_cmp);
    let pct = |v: &[f64], p: f64| {
        v.get(((v.len() as f64 - 1.0) * p) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    println!(
        "  {} users in {:.0} s; gathering one user's discovery pool p50 {:.0} ms, p95 {:.0} ms",
        j.users.len(),
        started.elapsed().as_secs_f64(),
        pct(&gather_ms, 0.5),
        pct(&gather_ms, 0.95)
    );
    println!(
        "  shown per user: {:.1} (the taste rail's share and the discovery rails)\n",
        shown_total as f64 / users
    );
    println!(
        "  novelty, taste rail (20 a user): {:.2} bits ({:.1}% of picks unliked by any build user, left out)",
        taste_n.mean(),
        taste_n.unliked_share() * 100.0
    );
    println!(
        "  novelty, discovery rails:        {:.2} bits ({:.1}% left out)",
        disc_n.mean(),
        disc_n.unliked_share() * 100.0
    );
    for arm in Arm::ALL {
        if let Some(v) = per_arm.get(&arm) {
            println!(
                "    {:>10}: {:.2} bits, {} picks in {} rails ({:.1}% left out)",
                arm.key(),
                v.mean(),
                picks_per_arm.get(&arm).copied().unwrap_or(0),
                rails_per_arm.get(&arm).copied().unwrap_or(0),
                v.unliked_share() * 100.0
            );
        }
    }
    let (all, pop, taste_r) = (recall_all / users, recall_pop / users, recall_taste / users);
    println!(
        "\n  Recall@N of everything shown {all:.4}, popularity's top N {pop:.4} ({:+.1}%)",
        (all / pop - 1.0) * 100.0
    );
    println!("  (the taste rail alone, Recall@20: {taste_r:.4})");
    println!(
        "  (Phase 17's ranker asked for N: Recall@N {:.4}, {:+.1}% against popularity)",
        recall_ranker_n / users,
        (recall_ranker_n / users / pop - 1.0) * 100.0
    );
    let rate = |h: usize, c: usize| h as f64 / c.max(1) as f64;
    println!(
        "  (held-out likes per card: taste rail {:.4}, discovery {:.4}, popularity {:.4})",
        rate(hits_taste, cards_taste),
        rate(hits_disc, cards_disc),
        rate(hits_pop, cards_pop)
    );
    println!("  discovery labels checked: {labels}, false: {false_labels}");

    let lift_bits = disc_n.mean() - taste_n.mean();
    let e4 = lift_bits >= 1.0 && all / pop - 1.0 > 0.40;
    let e3 = false_labels == 0 && labels > 0;
    let said = |ok: bool| if ok { "met" } else { "NOT met" };
    println!(
        "\n  E4, discovery {lift_bits:+.2} bits over the taste rail (at least 1) with relevance held: {}",
        said(e4)
    );
    println!(
        "  E3, every discovery label true of its title: {}",
        said(e3)
    );
    Ok(e4 && e3)
}
