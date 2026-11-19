//! The discovery arms (SPEC.md Phase 18): five ways to look past the obvious, beside Phase
//! 17's rail, which is the sixth (safe-familiar, [`Arm::Familiar`]).
//!
//! Every arm takes titles already described ([`Title`]) and a profile ([`Profile`]), and
//! returns picks, each with a [`Label`]: what is true of it that made the arm choose it.
//! A label is data, not words. The words are made from it later, by whoever knows the
//! names; and [`holds`] re-derives the claim from the facts, so a test can check every
//! label of every arm on generated profiles (E3: "rail labels accurately describe why each
//! item is there").
//!
//! # The arms
//!
//! - **Stretch** ([`stretch`]): close to a taste mode in meaning, and outside it on exactly
//!   one axis (a decade, a genre, a country or a movement none of the mode's titles have).
//!   "Like Stalker, but from the 1990s."
//! - **Bridge** ([`bridge`]): close to two modes at once. "Between Stalker and Spirited Away."
//! - **Blind spot** ([`blind_spot`]): acclaimed work from a region the profile has met
//!   nothing from. "You've never watched anything from the 1950s."
//! - **Canon** ([`canon`]): acclaimed and widely seen, close to a mode, not yet met.
//! - **Deep cut** ([`deep_cut`]): highly rated and little seen, close to a mode or loved by
//!   people who loved one of the profile's titles.
//!
//! # What "close" means
//!
//! A cosine means little on its own: the meaning model puts unrelated films at 0.4 and
//! related ones at 0.7, and a different model would not. So closeness is by rank: a title is
//! close to a mode when it is at least as close as the mode's [`Taste::close`], the cosine of
//! its 1,000th nearest title in the meaning index (the gatherer reads it from the index). "Close
//! to Stalker" means "among the thousand titles nearest that taste", of more than a hundred
//! thousand.

use std::collections::{HashMap, HashSet};

/// A way of choosing what to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Arm {
    /// Phase 17's rail: what the taste predicts, with its own reasons (`rank::Reason`).
    Familiar,
    Stretch,
    Bridge,
    BlindSpot,
    Canon,
    DeepCut,
}

impl Arm {
    pub const ALL: [Arm; 6] = [
        Arm::Familiar,
        Arm::Stretch,
        Arm::Bridge,
        Arm::BlindSpot,
        Arm::Canon,
        Arm::DeepCut,
    ];

    /// The name it is stored under.
    pub fn key(self) -> &'static str {
        match self {
            Arm::Familiar => "familiar",
            Arm::Stretch => "stretch",
            Arm::Bridge => "bridge",
            Arm::BlindSpot => "blind_spot",
            Arm::Canon => "canon",
            Arm::DeepCut => "deep_cut",
        }
    }

    pub fn from_key(key: &str) -> Option<Arm> {
        Arm::ALL.into_iter().find(|a| a.key() == key)
    }
}

/// A region of the catalogue: what a stretch steps into and what a blind spot is.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Region {
    /// The decade's first year: 1950 for the 1950s.
    Decade(i64),
    /// IMDb's genre, as the catalogue stores it ("Western").
    Genre(String),
    /// As Wikidata names it ("Iran", "Soviet Union").
    Country(String),
    /// A film movement, as Wikidata names it ("Iranian New Wave").
    Movement(String),
}

/// The four kinds of [`Region`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    Decade,
    Genre,
    Country,
    Movement,
}

impl Region {
    pub fn axis(&self) -> Axis {
        match self {
            Region::Decade(_) => Axis::Decade,
            Region::Genre(_) => Axis::Genre,
            Region::Country(_) => Axis::Country,
            Region::Movement(_) => Axis::Movement,
        }
    }
}

/// Genres that describe a kind of broadcast, not a region of cinema: never a blind spot, and
/// never a stretch.
const NOT_REGIONS: [&str; 6] = [
    "Adult",
    "Game-Show",
    "News",
    "Reality-TV",
    "Short",
    "Talk-Show",
];

/// The decade a year falls in.
pub fn decade(year: i64) -> i64 {
    year.div_euclid(10) * 10
}

/// One title as the arms see it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Title {
    pub id: i64,
    /// Its cosine to each taste mode, in the profile's order; empty when it has no vector.
    pub near: Vec<f32>,
    /// IMDb's rating times ten, 0 to 100.
    pub rating: Option<i64>,
    /// IMDb votes.
    pub votes: i64,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    /// As Wikidata names them; empty until asked (ADR-0055).
    pub countries: Vec<String>,
    pub movements: Vec<String>,
    /// The profile's loved titles whose stored neighbours include this one, with the
    /// matrix's score: "people who loved X loved this".
    pub loved_with: Vec<(i64, f32)>,
}

impl Title {
    /// Every region it belongs to.
    pub fn regions(&self) -> Vec<Region> {
        let mut out: Vec<Region> = self
            .year
            .map(|y| Region::Decade(decade(y)))
            .into_iter()
            .collect();
        out.extend(
            self.genres
                .iter()
                .filter(|g| !NOT_REGIONS.contains(&g.as_str()))
                .map(|g| Region::Genre(g.clone())),
        );
        out.extend(self.countries.iter().map(|c| Region::Country(c.clone())));
        out.extend(self.movements.iter().map(|m| Region::Movement(m.clone())));
        out
    }

    /// Its cosine to mode `m`, when it has a vector.
    fn cosine(&self, m: usize) -> Option<f32> {
        self.near.get(m).copied()
    }

    /// Close to mode `m` of `profile` (see the module's notes).
    fn close_to(&self, profile: &Profile, m: usize) -> bool {
        match (self.cosine(m), profile.modes.get(m)) {
            (Some(c), Some(t)) => c >= t.close,
            _ => false,
        }
    }

    /// How far inside mode `m`'s closeness it sits, 0 at the edge; `None` when outside.
    fn inside(&self, profile: &Profile, m: usize) -> Option<f32> {
        let t = profile.modes.get(m)?;
        let c = self.cosine(m)?;
        (c >= t.close).then_some(c - t.close)
    }

    fn rated_at_least(&self, rating: i64) -> bool {
        self.rating.is_some_and(|r| r >= rating)
    }
}

/// One taste mode as the arms see it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Taste {
    /// The loved titles it is made of, strongest first: the first is what a label names.
    pub members: Vec<i64>,
    /// The cosine of the mode's 1,000th nearest title in the meaning index: closer than this
    /// is "close".
    pub close: f32,
    /// Every region of its members.
    pub regions: HashSet<Region>,
}

/// What the arms know of a profile.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Profile {
    pub modes: Vec<Taste>,
    /// Every region of every title it has met (seen, rated, started or saved).
    pub met: HashSet<Region>,
    /// How many titles it has met: a profile of three has a blind spot everywhere, which is
    /// not worth saying.
    pub met_titles: usize,
    /// Axes on which something it met is not known yet (a country Wikidata has not been asked
    /// about): nothing is claimed on them, since "never" might be false.
    pub unknown: HashSet<Axis>,
}

/// Why an arm chose a title. Each is a claim [`holds`] can check against the facts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Label {
    /// Close to `mode`, and in `region`, which none of the mode's titles are.
    Stretch { mode: usize, region: Region },
    /// Close to modes `a` and `b` both (`a` < `b`).
    Between { a: usize, b: usize },
    /// Acclaimed ([`BLIND_RATING`], [`BLIND_VOTES`]) and in `region`, which the profile has
    /// met nothing from.
    BlindSpot(Region),
    /// Acclaimed and widely seen ([`CANON_RATING`], [`CANON_VOTES`]), close to `mode`.
    Canon { mode: usize },
    /// Highly rated and little seen ([`DEEP_RATING`], [`DEEP_VOTES`]), close to `mode`.
    DeepCutNear { mode: usize },
    /// Highly rated and little seen, and loved by people who loved `loved`.
    DeepCutLoved { loved: i64 },
}

impl Label {
    pub fn arm(&self) -> Arm {
        match self {
            Label::Stretch { .. } => Arm::Stretch,
            Label::Between { .. } => Arm::Bridge,
            Label::BlindSpot(_) => Arm::BlindSpot,
            Label::Canon { .. } => Arm::Canon,
            Label::DeepCutNear { .. } | Label::DeepCutLoved { .. } => Arm::DeepCut,
        }
    }
}

/// One title an arm chose, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Discovery {
    pub id: i64,
    pub label: Label,
}

/// Canon: IMDb 7.8 or more, from 50,000 votes. 694 films in the development catalogue.
pub const CANON_RATING: i64 = 78;
pub const CANON_VOTES: i64 = 50_000;
/// Deep cut: 7.5 or more, from 1,000 votes (fewer and a rating is a handful of friends) to
/// 15,000 (a hundredth of what the canon's most seen have). 5,441 films.
pub const DEEP_RATING: i64 = 75;
pub const DEEP_VOTES: (i64, i64) = (1_000, 15_000);
/// Blind spot: 7.5 or more, from 5,000 votes, so the region shows its best. At least 56
/// films in every decade from the 1920s.
pub const BLIND_RATING: i64 = 75;
pub const BLIND_VOTES: i64 = 5_000;
/// A profile needs this many titles met before a region it has not met is a blind spot.
pub const BLIND_MIN_MET: usize = 10;
/// A blind spot with fewer acclaimed titles than this is chosen only when none has more.
pub const BLIND_ENOUGH: usize = 5;

/// Is `label` true of `title` for `profile`? Re-derived from the facts, independently of how
/// the arms choose: E3's check.
pub fn holds(label: &Label, title: &Title, profile: &Profile) -> bool {
    match label {
        Label::Stretch { mode, region } => {
            !profile.unknown.contains(&region.axis())
                && title.close_to(profile, *mode)
                && title.regions().contains(region)
                && profile
                    .modes
                    .get(*mode)
                    .is_some_and(|t| !t.regions.contains(region))
        }
        Label::Between { a, b } => {
            a != b && title.close_to(profile, *a) && title.close_to(profile, *b)
        }
        Label::BlindSpot(region) => {
            profile.met_titles >= BLIND_MIN_MET
                && !profile.unknown.contains(&region.axis())
                && !profile.met.contains(region)
                && title.regions().contains(region)
                && title.rated_at_least(BLIND_RATING)
                && title.votes >= BLIND_VOTES
        }
        Label::Canon { mode } => {
            title.close_to(profile, *mode)
                && title.rated_at_least(CANON_RATING)
                && title.votes >= CANON_VOTES
        }
        Label::DeepCutNear { mode } => deep(title) && title.close_to(profile, *mode),
        Label::DeepCutLoved { loved } => {
            deep(title) && title.loved_with.iter().any(|(l, _)| l == loved)
        }
    }
}

fn deep(title: &Title) -> bool {
    title.rated_at_least(DEEP_RATING) && (DEEP_VOTES.0..=DEEP_VOTES.1).contains(&title.votes)
}

/// Picks taken in turn from each list, best first, each title once and at most `k`: so
/// every mode (or pair, or region) is heard, not only the first.
fn round_robin(lists: Vec<Vec<(i64, Label)>>, k: usize) -> Vec<Discovery> {
    let mut out = Vec::new();
    let mut taken: HashSet<i64> = HashSet::new();
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    for at in 0..longest {
        for list in &lists {
            if out.len() >= k {
                return out;
            }
            if let Some((id, label)) = list.get(at) {
                if taken.insert(*id) {
                    out.push(Discovery {
                        id: *id,
                        label: label.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Sort best first by `score`, ties to the lower id, so the same input picks the same.
fn by_score(mut scored: Vec<(f32, i64, Label)>) -> Vec<(i64, Label)> {
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, id, l)| (id, l)).collect()
}

/// The regions `title` has and `taste` does not, when they are all on one axis: one step
/// away. Empty when it differs on none, or on two or more.
fn outside(title: &Title, taste: &Taste, unknown: &HashSet<Axis>) -> Vec<Region> {
    let regions = title.regions();
    let mut axes: HashMap<Axis, Vec<Region>> = HashMap::new();
    for r in regions {
        // Only an axis the mode says something about, and the profile is known on: a mode
        // with no countries known says nothing about a title's country.
        if !unknown.contains(&r.axis())
            && taste.regions.iter().any(|t| t.axis() == r.axis())
            && !taste.regions.contains(&r)
        {
            axes.entry(r.axis()).or_default().push(r);
        }
    }
    if axes.len() == 1 {
        let mut only: Vec<Region> = axes.into_values().next().unwrap_or_default();
        only.sort();
        only
    } else {
        Vec::new()
    }
}

/// **Stretch**: close to a mode and outside it on exactly one axis. Best first by closeness,
/// the modes in turn.
pub fn stretch(titles: &[Title], profile: &Profile, k: usize) -> Vec<Discovery> {
    let lists = (0..profile.modes.len())
        .map(|m| {
            let taste = &profile.modes[m];
            by_score(
                titles
                    .iter()
                    .filter_map(|t| {
                        let depth = t.inside(profile, m)?;
                        let region = outside(t, taste, &profile.unknown).into_iter().next()?;
                        Some((depth, t.id, Label::Stretch { mode: m, region }))
                    })
                    .collect(),
            )
        })
        .collect();
    round_robin(lists, k)
}

/// **Bridge**: close to two modes at once, by the nearer of the two (a title deep in one mode
/// and at the edge of the other is a step, not a bridge). The pairs in turn.
pub fn bridge(titles: &[Title], profile: &Profile, k: usize) -> Vec<Discovery> {
    let n = profile.modes.len();
    let mut lists = Vec::new();
    for a in 0..n {
        for b in a + 1..n {
            lists.push(by_score(
                titles
                    .iter()
                    .filter_map(|t| {
                        let depth = t.inside(profile, a)?.min(t.inside(profile, b)?);
                        Some((depth, t.id, Label::Between { a, b }))
                    })
                    .collect(),
            ));
        }
    }
    // The pairs with the most to offer first.
    lists.sort_by_key(|l| std::cmp::Reverse(l.len()));
    round_robin(lists, k)
}

/// The closest a title comes to any mode, as a cosine; -1 without a vector.
fn nearest_mode(title: &Title) -> f32 {
    title.near.iter().copied().fold(-1.0, f32::max)
}

/// **Blind spot**: one region the profile has met nothing from, and up to `k` of its acclaimed
/// titles, nearest the profile's taste first. Only one region: the rail is named after it.
/// The region chosen has at least [`BLIND_ENOUGH`] titles when any has, and of those the one
/// whose best five come nearest the taste; a movement or a country before a decade or a genre
/// at equal closeness, since it says more.
pub fn blind_spot(titles: &[Title], profile: &Profile, k: usize) -> Vec<Discovery> {
    if profile.met_titles < BLIND_MIN_MET {
        return Vec::new();
    }
    let mut by_region: HashMap<Region, Vec<&Title>> = HashMap::new();
    for t in titles {
        if !t.rated_at_least(BLIND_RATING) || t.votes < BLIND_VOTES {
            continue;
        }
        for r in t.regions() {
            if !profile.met.contains(&r) && !profile.unknown.contains(&r.axis()) {
                by_region.entry(r).or_default().push(t);
            }
        }
    }
    let specific = |r: &Region| match r.axis() {
        Axis::Movement => 3,
        Axis::Country => 2,
        Axis::Decade => 1,
        Axis::Genre => 0,
    };
    let mut regions: Vec<(Region, Vec<&Title>, f32)> = by_region
        .into_iter()
        .map(|(r, mut ts)| {
            ts.sort_by(|a, b| {
                nearest_mode(b)
                    .total_cmp(&nearest_mode(a))
                    .then(a.id.cmp(&b.id))
            });
            let best: Vec<f32> = ts.iter().take(5).map(|t| nearest_mode(t)).collect();
            let mean = best.iter().sum::<f32>() / best.len().max(1) as f32;
            (r, ts, mean)
        })
        .collect();
    // A region with enough to fill a short rail first: the rail is named after its region,
    // so it holds one region only.
    let enough = |ts: &Vec<&Title>| ts.len() >= BLIND_ENOUGH.min(k);
    regions.sort_by(|a, b| {
        enough(&b.1)
            .cmp(&enough(&a.1))
            .then(b.2.total_cmp(&a.2))
            .then(specific(&b.0).cmp(&specific(&a.0)))
            .then(a.0.cmp(&b.0))
    });
    let Some((region, ts, _)) = regions.into_iter().next() else {
        return Vec::new();
    };
    ts.into_iter()
        .take(k)
        .map(|t| Discovery {
            id: t.id,
            label: Label::BlindSpot(region.clone()),
        })
        .collect()
}

/// **Canon**: acclaimed and widely seen, close to a mode. Best rated first, the modes in turn.
pub fn canon(titles: &[Title], profile: &Profile, k: usize) -> Vec<Discovery> {
    let lists = (0..profile.modes.len())
        .map(|m| {
            by_score(
                titles
                    .iter()
                    .filter(|t| t.rated_at_least(CANON_RATING) && t.votes >= CANON_VOTES)
                    .filter(|t| t.close_to(profile, m))
                    .map(|t| {
                        let score =
                            t.rating.unwrap_or(0) as f32 + t.inside(profile, m).unwrap_or(0.0);
                        (score, t.id, Label::Canon { mode: m })
                    })
                    .collect(),
            )
        })
        .collect();
    round_robin(lists, k)
}

/// **Deep cut**: highly rated and little seen. Loved by people who loved what the profile
/// loved first (the stronger claim), then close to a mode; the modes in turn.
pub fn deep_cut(titles: &[Title], profile: &Profile, k: usize) -> Vec<Discovery> {
    let cuts: Vec<&Title> = titles.iter().filter(|t| deep(t)).collect();
    let loved = by_score(
        cuts.iter()
            .filter_map(|t| {
                let (from, score) = t
                    .loved_with
                    .iter()
                    .copied()
                    .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))?;
                Some((score, t.id, Label::DeepCutLoved { loved: from }))
            })
            .collect(),
    );
    let mut lists = vec![loved];
    for m in 0..profile.modes.len() {
        lists.push(by_score(
            cuts.iter()
                .filter_map(|t| {
                    let depth = t.inside(profile, m)?;
                    let score = t.rating.unwrap_or(0) as f32 / 100.0 + depth;
                    Some((score, t.id, Label::DeepCutNear { mode: m }))
                })
                .collect(),
        ));
    }
    round_robin(lists, k)
}

/// Every discovery arm's picks, `k` each, no title in two: the arms in [`Arm::ALL`]'s order,
/// each skipping what an earlier one took (and `taken`, what other rails already show).
pub fn all(
    titles: &[Title],
    profile: &Profile,
    k: usize,
    taken: &HashSet<i64>,
) -> Vec<(Arm, Vec<Discovery>)> {
    let mut used: HashSet<i64> = taken.clone();
    let mut out = Vec::new();
    for arm in Arm::ALL {
        let free: Vec<Title> = titles
            .iter()
            .filter(|t| !used.contains(&t.id))
            .cloned()
            .collect();
        let picks = match arm {
            Arm::Familiar => continue,
            Arm::Stretch => stretch(&free, profile, k),
            Arm::Bridge => bridge(&free, profile, k),
            Arm::BlindSpot => blind_spot(&free, profile, k),
            Arm::Canon => canon(&free, profile, k),
            Arm::DeepCut => deep_cut(&free, profile, k),
        };
        used.extend(picks.iter().map(|d| d.id));
        out.push((arm, picks));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator: the crate takes no dependencies (a linear
    /// congruential generator, Knuth's constants).
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
        fn unit(&mut self) -> f32 {
            (self.below(10_000) as f32) / 10_000.0
        }
        fn chance(&mut self, p: f32) -> bool {
            self.unit() < p
        }
    }

    const GENRES: [&str; 8] = [
        "Drama",
        "Comedy",
        "Western",
        "Horror",
        "Sci-Fi",
        "Animation",
        "Crime",
        "Short",
    ];
    const COUNTRIES: [&str; 5] = ["IR", "JP", "FR", "US", "SE"];
    const MOVEMENTS: [&str; 3] = ["Iranian New Wave", "French New Wave", "Dogme 95"];

    fn pick<'a>(rng: &mut Lcg, from: &[&'a str]) -> &'a str {
        from[rng.below(from.len() as u64) as usize]
    }

    fn title(rng: &mut Lcg, id: i64, modes: usize) -> Title {
        let genres: Vec<String> = (0..1 + rng.below(3))
            .map(|_| pick(rng, &GENRES).to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        Title {
            id,
            near: if rng.chance(0.9) {
                (0..modes).map(|_| rng.unit() * 1.4 - 0.4).collect()
            } else {
                Vec::new()
            },
            rating: rng.chance(0.95).then(|| 40 + rng.below(56) as i64),
            votes: [500, 3_000, 9_000, 20_000, 80_000, 400_000][rng.below(6) as usize]
                + rng.below(500) as i64,
            year: rng.chance(0.97).then(|| 1920 + rng.below(105) as i64),
            genres,
            countries: if rng.chance(0.6) {
                vec![pick(rng, &COUNTRIES).to_string()]
            } else {
                Vec::new()
            },
            movements: if rng.chance(0.1) {
                vec![pick(rng, &MOVEMENTS).to_string()]
            } else {
                Vec::new()
            },
            loved_with: if rng.chance(0.3) {
                vec![(1_000_000 + rng.below(8) as i64, rng.unit())]
            } else {
                Vec::new()
            },
        }
    }

    /// A profile of up to five modes over `met` titles, each mode's regions those of a few of
    /// them.
    fn profile(rng: &mut Lcg, met: &[Title], modes: usize) -> Profile {
        let mut p = Profile {
            met_titles: met.len(),
            ..Default::default()
        };
        for t in met {
            p.met.extend(t.regions());
        }
        if rng.chance(0.3) {
            p.unknown.insert(Axis::Country);
        }
        for m in 0..modes {
            let members: Vec<&Title> = met.iter().skip(m).step_by(modes.max(1)).collect();
            p.modes.push(Taste {
                members: members.iter().map(|t| t.id).collect(),
                close: 0.3 + rng.unit() * 0.6,
                regions: members.iter().flat_map(|t| t.regions()).collect(),
            });
        }
        p
    }

    fn world(seed: u64) -> (Vec<Title>, Profile) {
        let mut rng = Lcg(seed);
        let modes = 1 + rng.below(5) as usize;
        let met_count = rng.below(40) as usize;
        let met: Vec<Title> = (0..met_count)
            .map(|i| title(&mut rng, 1_000_000 + i as i64, modes))
            .collect();
        let profile = profile(&mut rng, &met, modes);
        let titles = (0..400).map(|i| title(&mut rng, i, modes)).collect();
        (titles, profile)
    }

    #[test]
    fn every_label_of_every_arm_holds_on_generated_profiles() {
        // E3: a label is a claim, and on 300 generated profiles every pick's claim is true
        // of its title. Seen to fail (2026-11-19), each on seed 1: the arms taking titles
        // 0.1 short of a mode's closeness (a deep cut "close to" a mode it is not), and the
        // blind spot ignoring what the profile has met (a comedy for a profile with comedies).
        let mut picked = HashMap::<Arm, usize>::new();
        for seed in 1..=300 {
            let (titles, profile) = world(seed);
            let by_id: HashMap<i64, &Title> = titles.iter().map(|t| (t.id, t)).collect();
            for (arm, picks) in all(&titles, &profile, 20, &HashSet::new()) {
                assert!(picks.len() <= 20);
                for d in &picks {
                    assert_eq!(d.label.arm(), arm, "seed {seed}: {d:?} in {arm:?}");
                    let t = by_id[&d.id];
                    assert!(
                        holds(&d.label, t, &profile),
                        "seed {seed}: {d:?} does not hold of {t:?}"
                    );
                    *picked.entry(arm).or_default() += 1;
                }
            }
        }
        // And the test is not vacuous: every arm chose something across the profiles.
        for arm in [
            Arm::Stretch,
            Arm::Bridge,
            Arm::BlindSpot,
            Arm::Canon,
            Arm::DeepCut,
        ] {
            assert!(
                picked.get(&arm).copied().unwrap_or(0) > 50,
                "{arm:?}: {picked:?}"
            );
        }
    }

    #[test]
    fn no_title_is_in_two_arms_or_already_shown() {
        for seed in 1..=100 {
            let (titles, profile) = world(seed);
            let shown: HashSet<i64> = (0..400).step_by(7).collect();
            let mut seen = HashSet::new();
            for (_, picks) in all(&titles, &profile, 20, &shown) {
                for d in picks {
                    assert!(
                        !shown.contains(&d.id),
                        "seed {seed}: {} already shown",
                        d.id
                    );
                    assert!(seen.insert(d.id), "seed {seed}: {} twice", d.id);
                }
            }
        }
    }

    fn taste(members: Vec<i64>, close: f32, regions: Vec<Region>) -> Taste {
        Taste {
            members,
            close,
            regions: regions.into_iter().collect(),
        }
    }

    fn drama_1970s() -> Profile {
        let regions = vec![
            Region::Decade(1970),
            Region::Genre("Drama".into()),
            Region::Country("SU".into()),
        ];
        Profile {
            modes: vec![
                taste(vec![1], 0.6, regions.clone()),
                taste(
                    vec![2],
                    0.6,
                    vec![Region::Decade(2000), Region::Genre("Animation".into())],
                ),
            ],
            met: regions
                .into_iter()
                .chain([Region::Decade(2000), Region::Genre("Animation".into())])
                .collect(),
            met_titles: 30,
            unknown: HashSet::new(),
        }
    }

    #[test]
    fn nothing_is_claimed_on_an_axis_not_yet_known() {
        // A film the profile met was never asked of Wikidata: it may be Iranian, so "you have
        // never watched anything from Iran" may be false, and is not said.
        let mut p = drama_1970s();
        let iranian = Title {
            id: 17,
            near: vec![0.8, 0.2],
            rating: Some(80),
            votes: 9_000,
            year: Some(1975),
            genres: vec!["Drama".into()],
            countries: vec!["Iran".into()],
            ..Default::default()
        };
        let label = Label::BlindSpot(Region::Country("Iran".into()));
        assert!(holds(&label, &iranian, &p));
        assert_eq!(
            blind_spot(std::slice::from_ref(&iranian), &p, 5)[0].label,
            label
        );
        let stretch_label = Label::Stretch {
            mode: 0,
            region: Region::Country("Iran".into()),
        };
        assert!(holds(&stretch_label, &iranian, &p));
        p.unknown.insert(Axis::Country);
        assert!(!holds(&label, &iranian, &p), "seen to fail");
        assert!(!holds(&stretch_label, &iranian, &p), "seen to fail");
        assert!(blind_spot(std::slice::from_ref(&iranian), &p, 5).is_empty());
        assert!(stretch(&[iranian], &p, 5).is_empty());
    }

    #[test]
    fn a_stretch_is_close_and_one_axis_out() {
        let p = drama_1970s();
        let nineties = Title {
            id: 10,
            near: vec![0.8, 0.1],
            year: Some(1994),
            genres: vec!["Drama".into()],
            countries: vec!["SU".into()],
            ..Default::default()
        };
        let picks = stretch(std::slice::from_ref(&nineties), &p, 5);
        let label = Label::Stretch {
            mode: 0,
            region: Region::Decade(1990),
        };
        assert_eq!(
            picks,
            vec![Discovery {
                id: 10,
                label: label.clone()
            }]
        );
        assert!(holds(&label, &nineties, &p));
        // Two axes out (a 1990s comedy) is not a step but a leap: not a stretch.
        let leap = Title {
            genres: vec!["Comedy".into()],
            ..nineties.clone()
        };
        assert!(stretch(&[leap], &p, 5).is_empty());
        // Seen to fail: not close enough, and the claim no longer holds.
        let far = Title {
            near: vec![0.55, 0.1],
            ..nineties
        };
        assert!(!holds(&label, &far, &p));
        assert!(stretch(&[far], &p, 5).is_empty());
    }

    #[test]
    fn a_bridge_is_close_to_both() {
        let p = drama_1970s();
        let both = Title {
            id: 11,
            near: vec![0.7, 0.65],
            ..Default::default()
        };
        let one = Title {
            id: 12,
            near: vec![0.9, 0.5],
            ..Default::default()
        };
        let picks = bridge(&[both.clone(), one.clone()], &p, 5);
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].label, Label::Between { a: 0, b: 1 });
        assert!(holds(&picks[0].label, &both, &p));
        assert!(!holds(&picks[0].label, &one, &p), "seen to fail");
    }

    #[test]
    fn a_blind_spot_is_acclaimed_and_unmet() {
        let p = drama_1970s();
        let western = Title {
            id: 13,
            near: vec![0.5, 0.2],
            rating: Some(80),
            votes: 9_000,
            year: Some(1975),
            genres: vec!["Western".into()],
            ..Default::default()
        };
        let picks = blind_spot(std::slice::from_ref(&western), &p, 5);
        assert_eq!(
            picks[0].label,
            Label::BlindSpot(Region::Genre("Western".into()))
        );
        assert!(holds(&picks[0].label, &western, &p));
        // Seen to fail: once a western is met, it is no longer a blind spot.
        let mut met = p.clone();
        met.met.insert(Region::Genre("Western".into()));
        assert!(!holds(&picks[0].label, &western, &met));
        assert!(blind_spot(std::slice::from_ref(&western), &met, 5).is_empty());
        // Nor with too little history to say.
        let thin = Profile { met_titles: 3, ..p };
        assert!(blind_spot(&[western], &thin, 5).is_empty());
    }

    #[test]
    fn a_blind_spot_rail_holds_one_region() {
        // Named after its region, so a second region's titles would make the name false.
        let p = drama_1970s();
        let acclaimed = |id, year, genre: &str, near| Title {
            id,
            near: vec![near, 0.0],
            rating: Some(80),
            votes: 9_000,
            year: Some(year),
            genres: vec![genre.into()],
            ..Default::default()
        };
        let mut titles: Vec<Title> = (0..6)
            .map(|i| acclaimed(100 + i, 1950 + i, "Drama", 0.3))
            .collect();
        // Two westerns nearer the taste than any 1950s drama, but too few to fill a rail.
        titles.push(acclaimed(200, 1975, "Western", 0.9));
        titles.push(acclaimed(201, 1976, "Western", 0.9));
        let picks = blind_spot(&titles, &p, 20);
        assert_eq!(picks.len(), 6);
        assert!(
            picks
                .iter()
                .all(|d| d.label == Label::BlindSpot(Region::Decade(1950))),
            "{picks:?}"
        );
    }

    #[test]
    fn a_movement_is_named_before_a_genre_at_equal_closeness() {
        let p = drama_1970s();
        let t = Title {
            id: 14,
            near: vec![0.5, 0.2],
            rating: Some(80),
            votes: 9_000,
            year: Some(1975),
            genres: vec!["Western".into()],
            movements: vec!["Iranian New Wave".into()],
            ..Default::default()
        };
        let picks = blind_spot(&[t], &p, 5);
        assert_eq!(
            picks[0].label,
            Label::BlindSpot(Region::Movement("Iranian New Wave".into()))
        );
    }

    #[test]
    fn canon_and_deep_cuts_keep_to_their_thresholds() {
        let p = drama_1970s();
        let canon_title = Title {
            id: 15,
            near: vec![0.7, 0.0],
            rating: Some(CANON_RATING),
            votes: CANON_VOTES,
            ..Default::default()
        };
        let picks = canon(std::slice::from_ref(&canon_title), &p, 5);
        assert_eq!(picks[0].label, Label::Canon { mode: 0 });
        let fewer = Title {
            votes: CANON_VOTES - 1,
            ..canon_title.clone()
        };
        assert!(!holds(&picks[0].label, &fewer, &p), "seen to fail");
        assert!(canon(&[fewer], &p, 5).is_empty());

        let cut = Title {
            id: 16,
            near: vec![0.0, 0.0],
            rating: Some(DEEP_RATING),
            votes: DEEP_VOTES.1,
            loved_with: vec![(1, 0.2), (2, 0.4)],
            ..Default::default()
        };
        let picks = deep_cut(std::slice::from_ref(&cut), &p, 5);
        assert_eq!(
            picks[0].label,
            Label::DeepCutLoved { loved: 2 },
            "the strongest"
        );
        let seen_widely = Title {
            votes: DEEP_VOTES.1 + 1,
            ..cut
        };
        assert!(!holds(&picks[0].label, &seen_widely, &p), "seen to fail");
    }

    #[test]
    fn arms_keep_their_keys() {
        for arm in Arm::ALL {
            assert_eq!(Arm::from_key(arm.key()), Some(arm));
        }
        assert_eq!(Arm::from_key("nonsense"), None);
    }
}
