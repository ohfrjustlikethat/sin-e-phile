//! The bandit over the arms (SPEC.md Phase 18): how much of Home each way of choosing gets,
//! learned from what the profile does with what each arm put in front of it.
//!
//! # Thompson sampling
//!
//! Each arm's chance of a good outcome is unknown. The bandit keeps a belief about it, a
//! Beta distribution: `Beta(1 + successes, 1 + failures)`, flat before anything is known,
//! narrowing as outcomes arrive. Each time Home is built it **draws one plausible chance
//! per arm from its belief** and shares the slots out by the draws. An arm that has done
//! well usually draws high; an arm little tried has a wide belief and sometimes draws high
//! too, which is how it gets tried. That is the whole of the exploration: no separate
//! "explore 10% of the time" rule, the uncertainty does it (Thompson, 1933; Chapelle and
//! Li, "An Empirical Evaluation of Thompson Sampling", 2011).
//!
//! # Contextual
//!
//! Outcomes are kept per [`Context`] (the part of the day), and a context's belief borrows
//! [`BORROW`] of what the arm did in every other context, so an evening learns from the
//! afternoon until it has its own evidence. "Not right now" counts against the arm in that
//! context only.
//!
//! # Guards (E2, and "never let one strategy dominate")
//!
//! - **The exploration floor**: discovery (every arm but [`Arm::Familiar`]) keeps at least
//!   [`DISCOVERY_FLOOR`] of the slots whatever the profile does, so a profile that only
//!   ever plays the safe choice still meets something new.
//! - **The cap**: no discovery arm takes more than [`ARM_CAP`] of discovery's slots.
//! - **Forgetting**: an outcome counts half after [`HALF_LIFE_DAYS`], so a profile whose
//!   taste moves is followed (E1).
//!
//! # The budget
//!
//! A profile's "Comfort to Adventure" setting (0 to 1) sets where discovery's share starts,
//! from [`DISCOVERY_FLOOR`] at Comfort to [`DISCOVERY_CEILING`] at Adventure; what the
//! profile does moves it from there: finishing what the discovery arms chose pushes it up,
//! abandoning it eases it down ([`discovery_share`]).

use crate::strategy::Arm;

/// What a profile did with a title an arm chose. The strongest outcome a title reached is
/// the one that counts: a film played and then finished is one success, not two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Started, not (yet) finished.
    Played,
    Completed,
    /// Stopped in the first fifth.
    AbandonedEarly,
    Watchlisted,
    /// "Not for me".
    NotForMe,
    /// "Not right now": against the arm in this context only.
    NotRightNow,
    /// "Already seen": says nothing about the arm.
    AlreadySeen,
}

impl Outcome {
    /// The reward, 0 to 1, or `None` when the outcome says nothing about the arm.
    pub fn reward(self) -> Option<f64> {
        match self {
            Outcome::Completed => Some(1.0),
            Outcome::Watchlisted => Some(0.7),
            Outcome::Played => Some(0.6),
            Outcome::NotRightNow => Some(0.25),
            Outcome::AbandonedEarly | Outcome::NotForMe => Some(0.0),
            Outcome::AlreadySeen => None,
        }
    }

    /// Which counts when a title reached several: the most decisive.
    fn strength(self) -> u8 {
        match self {
            Outcome::AlreadySeen => 0,
            Outcome::NotRightNow => 1,
            Outcome::Played => 2,
            Outcome::Watchlisted => 3,
            Outcome::AbandonedEarly => 4,
            Outcome::NotForMe => 5,
            Outcome::Completed => 6,
        }
    }

    /// The outcome that counts of several for one title.
    pub fn strongest(outcomes: &[Outcome]) -> Option<Outcome> {
        outcomes.iter().copied().max_by_key(|o| o.strength())
    }
}

/// The part of the day a Home was built in: when someone watches says something about what
/// they want then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Context {
    /// 05:00 to 17:00.
    Day,
    /// 17:00 to 23:00.
    Evening,
    /// 23:00 to 05:00.
    Late,
}

impl Context {
    pub const ALL: [Context; 3] = [Context::Day, Context::Evening, Context::Late];

    /// The context of a local hour, 0 to 23.
    pub fn of_hour(hour: u32) -> Context {
        match hour {
            5..=16 => Context::Day,
            17..=22 => Context::Evening,
            _ => Context::Late,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Context::Day => "day",
            Context::Evening => "evening",
            Context::Late => "late",
        }
    }

    pub fn from_key(key: &str) -> Option<Context> {
        Context::ALL.into_iter().find(|c| c.key() == key)
    }
}

/// One outcome, as the bandit learns from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trial {
    pub arm: Arm,
    pub context: Context,
    pub outcome: Outcome,
    /// How long ago, in days.
    pub age_days: f64,
}

/// How long an outcome takes to count half.
pub const HALF_LIFE_DAYS: f64 = 60.0;
/// How much of an arm's evidence from other contexts a context's belief borrows.
pub const BORROW: f64 = 0.3;
/// Discovery's least share of the slots: the exploration floor (E2).
pub const DISCOVERY_FLOOR: f64 = 0.2;
/// Discovery's greatest share: the familiar always keeps a fifth.
pub const DISCOVERY_CEILING: f64 = 0.8;
/// The most of discovery's slots one discovery arm may take: half, so the others always have
/// the other half. (0.4 first: with a rail's minimum for each arm shown, it kept a favourite
/// that had changed at 30% of discovery for two months; docs/eval-results.md, 18.2.)
pub const ARM_CAP: f64 = 0.5;
/// The guards, together: what the app uses is [`GUARDS`]; the evaluation varies them to show
/// its verdicts can fail (`eval discovery --simulate --no-floor`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guards {
    pub floor: f64,
    pub ceiling: f64,
    pub arm_cap: f64,
}

pub const GUARDS: Guards = Guards {
    floor: DISCOVERY_FLOOR,
    ceiling: DISCOVERY_CEILING,
    arm_cap: ARM_CAP,
};

/// A population prior over the discovery arms (blocker B17): what a profile with no outcomes
/// yet is expected to find in each, as `weight` pseudo-trials at each arm's `mean`. Used only in
/// the draws that share discovery's slots between its arms ([`allocate_within`]): discovery's
/// share of Home is the budget's and the profile's, not the population's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prior {
    pub weight: f64,
    /// Stretch, bridge, blind spot, canon, deep cut: [`Arm::ALL`]'s order without the familiar.
    pub means: [f64; 5],
}

impl Prior {
    /// None at all: every arm flat.
    pub const NONE: Prior = Prior {
        weight: 0.0,
        means: [0.5; 5],
    };

    /// The same means at another weight (the evaluation's sweep).
    pub fn weighed(self, weight: f64) -> Prior {
        Prior { weight, ..self }
    }

    fn mean(&self, arm: Arm) -> f64 {
        let at = Arm::ALL
            .into_iter()
            .filter(|a| *a != Arm::Familiar)
            .position(|a| a == arm);
        at.map_or(0.5, |i| self.means[i])
    }
}

/// The prior the app uses (docs/eval-results.md, 18.2, B17), by the rule fixed there before it
/// ran. The means: each arm's held-out likes per card and novelty on 2,000 tuning users, each
/// over the best discovery arm's, summed and halved (stretch 0.0302 and 4.69 bits, bridge
/// 0.0303 and 5.49, blind spot 0.0108 and 4.15, canon 0.0497 and 3.64, deep cut 0.0018 and 11.35).
/// The weight: chosen on the same users from {0, 2, 4, 8, 16} (Recall@N against popularity
/// +38.8%, +44.1%, +43.6%, +43.0%, +44.0%; novelty over the taste rail 1.95, 1.79, 1.63, 1.50,
/// 1.37 bits): the best relevance with at least 1.2 bits, 2.
pub const PRIOR: Prior = Prior {
    weight: 2.0,
    means: [0.5104, 0.5467, 0.2915, 0.6604, 0.5181],
};

/// How far the profile's outcomes move discovery's share from where the budget puts it: a
/// difference of 0.5 between the discovery arms' mean chance and the familiar's moves it 0.3.
pub const ADAPT: f64 = 0.6;

/// Successes and failures, faded by age.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Counts {
    success: f64,
    failure: f64,
}

/// What the bandit believes of each arm in each context.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Beliefs {
    counts: std::collections::HashMap<(Arm, Context), Counts>,
}

impl Beliefs {
    pub fn from_trials(trials: &[Trial]) -> Beliefs {
        let mut b = Beliefs::default();
        for t in trials {
            let Some(r) = t.outcome.reward() else {
                continue;
            };
            let w = 0.5f64.powf(t.age_days.max(0.0) / HALF_LIFE_DAYS);
            let c = b.counts.entry((t.arm, t.context)).or_default();
            c.success += w * r;
            c.failure += w * (1.0 - r);
        }
        b
    }

    /// The Beta belief about `arm` in `context`: (alpha, beta). "Not right now" is kept out
    /// of the evidence other contexts borrow, since it was about that moment.
    pub fn belief(&self, arm: Arm, context: Context) -> (f64, f64) {
        let own = self
            .counts
            .get(&(arm, context))
            .copied()
            .unwrap_or_default();
        let mut alpha = 1.0 + own.success;
        let mut beta = 1.0 + own.failure;
        for other in Context::ALL.into_iter().filter(|c| *c != context) {
            if let Some(c) = self.counts.get(&(arm, other)) {
                alpha += BORROW * c.success;
                beta += BORROW * c.failure;
            }
        }
        (alpha, beta)
    }

    /// The belief's mean: the chance the bandit expects.
    pub fn mean(&self, arm: Arm, context: Context) -> f64 {
        let (a, b) = self.belief(arm, context);
        a / (a + b)
    }
}

/// A small deterministic generator (SplitMix64): the crate takes no dependencies, and a
/// seeded draw makes a Home, and a test, repeatable.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1).
    pub fn unit(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// Standard normal (Box-Muller).
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.unit(), self.unit());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// Gamma(shape, 1) (Marsaglia and Tsang, 2000; below shape 1 by boosting).
    fn gamma(&mut self, shape: f64) -> f64 {
        if shape < 1.0 {
            return self.gamma(shape + 1.0) * self.unit().powf(1.0 / shape);
        }
        let d = shape - 1.0 / 3.0;
        let c = 1.0 / (9.0 * d).sqrt();
        loop {
            let x = self.normal();
            let v = (1.0 + c * x).powi(3);
            if v <= 0.0 {
                continue;
            }
            let u = self.unit();
            if u.ln() < 0.5 * x * x + d - d * v + d * v.ln() {
                return d * v;
            }
        }
    }

    /// Beta(alpha, beta), as two gammas.
    pub fn beta(&mut self, alpha: f64, beta: f64) -> f64 {
        let x = self.gamma(alpha);
        let y = self.gamma(beta);
        x / (x + y)
    }
}

/// Discovery's share of the slots: where the budget (0 Comfort to 1 Adventure) puts it,
/// moved by how the discovery arms have done against the familiar, within the floor and the
/// ceiling.
pub fn discovery_share(beliefs: &Beliefs, context: Context, budget: f64) -> f64 {
    discovery_share_within(beliefs, context, budget, &GUARDS)
}

/// [`discovery_share`] within other guards.
pub fn discovery_share_within(
    beliefs: &Beliefs,
    context: Context,
    budget: f64,
    guards: &Guards,
) -> f64 {
    let start = guards.floor + (guards.ceiling - guards.floor) * budget.clamp(0.0, 1.0);
    let discovery: Vec<f64> = Arm::ALL
        .into_iter()
        .filter(|a| *a != Arm::Familiar)
        .map(|a| beliefs.mean(a, context))
        .collect();
    let mean = discovery.iter().sum::<f64>() / discovery.len() as f64;
    let lead = mean - beliefs.mean(Arm::Familiar, context);
    (start + ADAPT * lead).clamp(guards.floor, guards.ceiling)
}

/// The most slots one discovery arm may have when discovery has `discovery_slots`: [`ARM_CAP`]
/// of them, but never fewer than `least`, or a short discovery could hold no rail at all.
pub fn arm_cap(discovery_slots: usize, least: usize) -> usize {
    cap_within(discovery_slots, least, ARM_CAP)
}

fn cap_within(discovery_slots: usize, least: usize, cap: f64) -> usize {
    ((discovery_slots as f64 * cap).floor() as usize).max(least)
}

/// Joint draws per allocation: how often each arm's draw is the highest of them is the
/// chance, as the bandit believes, that it is the best (probability matching).
pub const DRAWS: usize = 100;

/// How many of `total` slots each arm gets this time: the familiar first, then the discovery
/// arms, most promising first.
///
/// 1. Discovery's slots are [`discovery_share`] of `total`.
/// 2. **Each discovery arm's chance of being the best**: of [`DRAWS`] joint draws from the
///    beliefs, how often its draw was the highest (one more each, so none is ever zero).
///    This is Thompson sampling spread over a page: an arm gets slots as often as it could
///    be the best one.
/// 3. **As few rails as fit**: the arms most likely the best, as many as it takes to place
///    discovery's slots under [`arm_cap`], and any other whose chance is worth a rail
///    (`least` slots).
/// 4. Each gets `least`; the rest go one at a time to the arm furthest below its chance's
///    share, never past the cap; what the caps cannot place goes to the familiar.
///
/// Measured (docs/eval-results.md, 18.2): sharing by single draws over as many rails as fit
/// gave every arm about a rail's minimum, and a favourite that changed moved its share by
/// only 12 points in 60 days.
pub fn allocate(
    beliefs: &Beliefs,
    context: Context,
    budget: f64,
    total: usize,
    least: usize,
    rng: &mut Rng,
) -> Vec<(Arm, usize)> {
    allocate_within(beliefs, context, budget, total, least, &GUARDS, &PRIOR, rng)
}

/// [`allocate`] within other guards and another prior.
#[allow(clippy::too_many_arguments)]
pub fn allocate_within(
    beliefs: &Beliefs,
    context: Context,
    budget: f64,
    total: usize,
    least: usize,
    guards: &Guards,
    prior: &Prior,
    rng: &mut Rng,
) -> Vec<(Arm, usize)> {
    let share = discovery_share_within(beliefs, context, budget, guards);
    let discovery_slots = ((total as f64) * share).round() as usize;
    let arms: Vec<Arm> = Arm::ALL
        .into_iter()
        .filter(|a| *a != Arm::Familiar)
        .collect();
    let beliefs_now: Vec<(f64, f64)> = arms
        .iter()
        .map(|a| {
            let (alpha, beta) = beliefs.belief(*a, context);
            let m = prior.mean(*a);
            (alpha + prior.weight * m, beta + prior.weight * (1.0 - m))
        })
        .collect();
    let mut wins = vec![1usize; arms.len()];
    for _ in 0..DRAWS {
        let best = beliefs_now
            .iter()
            .map(|(alpha, beta)| rng.beta(*alpha, *beta))
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(0, |(i, _)| i);
        wins[best] += 1;
    }
    let mut chance: Vec<(Arm, f64)> = arms
        .iter()
        .zip(&wins)
        .map(|(a, w)| (*a, *w as f64 / (DRAWS + arms.len()) as f64))
        .collect();
    chance.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

    let cap = cap_within(discovery_slots, least, guards.arm_cap);
    let fit = discovery_slots / least.max(1);
    let needed = discovery_slots.div_ceil(cap.max(1));
    let worth = chance
        .iter()
        .filter(|(_, c)| c * discovery_slots as f64 >= least as f64)
        .count();
    let shown = needed.max(worth).min(fit).min(chance.len());
    let chosen = &chance[..shown];
    let mut slots: Vec<usize> = vec![least; shown];
    let weight: f64 = chosen.iter().map(|(_, c)| c).sum::<f64>().max(1e-12);
    let mut left = discovery_slots - least * shown;
    while left > 0 {
        let furthest = (0..shown).filter(|&i| slots[i] < cap).max_by(|&i, &j| {
            let behind = |k: usize| discovery_slots as f64 * chosen[k].1 / weight - slots[k] as f64;
            behind(i).total_cmp(&behind(j)).then(j.cmp(&i))
        });
        let Some(i) = furthest else { break };
        slots[i] += 1;
        left -= 1;
    }
    let mut out = vec![(Arm::Familiar, total - discovery_slots + left)];
    out.extend(chosen.iter().zip(slots).map(|((arm, _), n)| (*arm, n)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trials(arm: Arm, outcome: Outcome, n: usize, context: Context) -> Vec<Trial> {
        (0..n)
            .map(|_| Trial {
                arm,
                context,
                outcome,
                age_days: 0.0,
            })
            .collect()
    }

    #[test]
    fn the_strongest_outcome_of_a_title_is_the_one_that_counts() {
        use Outcome::*;
        assert_eq!(Outcome::strongest(&[Played, Completed]), Some(Completed));
        assert_eq!(Outcome::strongest(&[Watchlisted, NotForMe]), Some(NotForMe));
        assert_eq!(Outcome::strongest(&[AlreadySeen]), Some(AlreadySeen));
        assert_eq!(AlreadySeen.reward(), None);
        assert_eq!(Outcome::strongest(&[]), None);
    }

    #[test]
    fn a_belief_starts_flat_and_learns() {
        let b = Beliefs::default();
        assert_eq!(b.belief(Arm::Canon, Context::Evening), (1.0, 1.0));
        let b = Beliefs::from_trials(&trials(Arm::Canon, Outcome::Completed, 8, Context::Evening));
        assert_eq!(b.belief(Arm::Canon, Context::Evening), (9.0, 1.0));
        // Another context borrows a part.
        let (a, beta) = b.belief(Arm::Canon, Context::Day);
        assert!((a - 3.4).abs() < 1e-9 && (beta - 1.0).abs() < 1e-9);
        // Old outcomes fade: one 60 days old counts half.
        let old = Beliefs::from_trials(&[Trial {
            arm: Arm::Canon,
            context: Context::Day,
            outcome: Outcome::Completed,
            age_days: HALF_LIFE_DAYS,
        }]);
        assert!((old.belief(Arm::Canon, Context::Day).0 - 1.5).abs() < 1e-9);
    }

    #[test]
    fn draws_follow_the_beta_they_come_from() {
        let mut rng = Rng::new(7);
        let n = 20_000;
        let mean = (0..n).map(|_| rng.beta(3.0, 7.0)).sum::<f64>() / n as f64;
        assert!((mean - 0.3).abs() < 0.01, "{mean}");
        let small = (0..n).map(|_| rng.beta(0.5, 0.5)).sum::<f64>() / n as f64;
        assert!((small - 0.5).abs() < 0.02, "{small}");
    }

    #[test]
    fn the_floor_holds_for_a_profile_that_only_ever_takes_the_familiar() {
        // E2's shape: every discovery pick dismissed, every familiar one finished.
        let mut t = trials(Arm::Familiar, Outcome::Completed, 200, Context::Evening);
        for arm in Arm::ALL.into_iter().filter(|a| *a != Arm::Familiar) {
            t.extend(trials(arm, Outcome::NotForMe, 60, Context::Evening));
        }
        let b = Beliefs::from_trials(&t);
        assert!((discovery_share(&b, Context::Evening, 0.0) - DISCOVERY_FLOOR).abs() < 1e-9);
        let mut rng = Rng::new(1);
        for _ in 0..200 {
            let a = allocate(&b, Context::Evening, 0.0, 60, 5, &mut rng);
            assert_eq!(a.iter().map(|(_, n)| n).sum::<usize>(), 60);
            let discovery: usize = a
                .iter()
                .filter(|(arm, _)| *arm != Arm::Familiar)
                .map(|(_, n)| n)
                .sum();
            assert!(discovery >= 10, "{a:?}");
        }
    }

    #[test]
    fn no_discovery_arm_takes_more_than_its_cap() {
        // One arm always rewarded, the rest never: it leads, and stops at the cap.
        let mut t = trials(Arm::DeepCut, Outcome::Completed, 300, Context::Day);
        for arm in [Arm::Stretch, Arm::Bridge, Arm::BlindSpot, Arm::Canon] {
            t.extend(trials(arm, Outcome::AbandonedEarly, 30, Context::Day));
        }
        let b = Beliefs::from_trials(&t);
        let planned = (60.0 * discovery_share(&b, Context::Day, 1.0)).round() as usize;
        let mut rng = Rng::new(3);
        for _ in 0..100 {
            let a = allocate(&b, Context::Day, 1.0, 60, 5, &mut rng);
            assert_eq!(a.iter().map(|(_, n)| n).sum::<usize>(), 60);
            let discovery: usize = a.iter().skip(1).map(|(_, n)| n).sum();
            assert_eq!(discovery, planned, "every planned slot placed: {a:?}");
            let deep = a.iter().find(|(arm, _)| *arm == Arm::DeepCut).unwrap().1;
            assert_eq!(a[1].0, Arm::DeepCut, "it leads: {a:?}");
            assert!(deep as f64 <= discovery as f64 * ARM_CAP, "{a:?}");
            assert!(a.iter().skip(1).all(|(_, n)| *n >= 5), "{a:?}");
        }
    }

    #[test]
    fn finishing_the_difficult_ones_pushes_further_and_bailing_eases_off() {
        let base = Beliefs::default();
        let start = discovery_share(&base, Context::Evening, 0.5);
        assert!((start - 0.5).abs() < 1e-9);
        let mut bold = trials(Arm::BlindSpot, Outcome::Completed, 20, Context::Evening);
        bold.extend(trials(
            Arm::Stretch,
            Outcome::Completed,
            20,
            Context::Evening,
        ));
        let pushed = discovery_share(&Beliefs::from_trials(&bold), Context::Evening, 0.5);
        let mut timid = trials(
            Arm::BlindSpot,
            Outcome::AbandonedEarly,
            20,
            Context::Evening,
        );
        timid.extend(trials(
            Arm::Stretch,
            Outcome::AbandonedEarly,
            20,
            Context::Evening,
        ));
        let eased = discovery_share(&Beliefs::from_trials(&timid), Context::Evening, 0.5);
        assert!(pushed > start + 0.05, "{pushed}");
        assert!(eased < start - 0.05, "{eased}");
    }

    #[test]
    fn a_prior_leans_a_new_profile_and_outcomes_outweigh_it() {
        // Canon favoured, deep cuts not; a new profile follows the prior, and a profile that
        // has finished thirty deep cuts and refused thirty canon picks follows itself.
        let prior = Prior {
            weight: 8.0,
            means: [0.5, 0.5, 0.5, 0.9, 0.1],
        };
        let share = |b: &Beliefs, arm: Arm| {
            let mut rng = Rng::new(5);
            let mut n = 0;
            for _ in 0..200 {
                let plan = allocate_within(b, Context::Day, 0.5, 60, 6, &GUARDS, &prior, &mut rng);
                n += plan.iter().find(|(a, _)| *a == arm).map_or(0, |(_, k)| *k);
            }
            n
        };
        let fresh = Beliefs::default();
        assert!(share(&fresh, Arm::Canon) > 3 * share(&fresh, Arm::DeepCut).max(1));
        let mut t = trials(Arm::DeepCut, Outcome::Completed, 30, Context::Day);
        t.extend(trials(Arm::Canon, Outcome::NotForMe, 30, Context::Day));
        let lived = Beliefs::from_trials(&t);
        assert!(share(&lived, Arm::DeepCut) > share(&lived, Arm::Canon));
        // And it never touches discovery's share of Home.
        let flat = allocate_within(
            &fresh,
            Context::Day,
            0.5,
            60,
            6,
            &GUARDS,
            &Prior::NONE,
            &mut Rng::new(1),
        );
        let leaned = allocate_within(
            &fresh,
            Context::Day,
            0.5,
            60,
            6,
            &GUARDS,
            &prior,
            &mut Rng::new(1),
        );
        assert_eq!(flat[0].1, leaned[0].1);
    }

    #[test]
    fn contexts_keep_their_keys_and_hours() {
        for c in Context::ALL {
            assert_eq!(Context::from_key(c.key()), Some(c));
        }
        assert_eq!(Context::of_hour(9), Context::Day);
        assert_eq!(Context::of_hour(20), Context::Evening);
        assert_eq!(Context::of_hour(2), Context::Late);
        assert_eq!(Context::of_hour(23), Context::Late);
    }
}
