//! The discovery rails (SPEC.md Phase 18): what the five discovery arms
//! (`sinephile_recommend::strategy`) choose from, and their labels in words.
//!
//! The arms decide; this gathers and describes. The pool is three things:
//!
//! - **Each taste mode's thousand nearest titles** in the meaning index, asked of the whole
//!   index (what the profile has met included), so the thousandth's cosine is what "close"
//!   means for that mode ([`CLOSE_RANK`]). Stretches, bridges, the canon and deep cuts near
//!   a mode are all among them.
//! - **The acclaimed** (`MediaRepository::acclaimed`): what blind spots and the canon are
//!   drawn from, wherever they sit in meaning.
//! - **What people who loved the same loved**, Phase 17's neighbours: deep cuts the matrix
//!   finds where meaning does not.
//!
//! The profile is described from what it has met: every region (decade, genre, country,
//! movement) of every title, an episode counting as its series, and which axes are not yet
//! known because Wikidata has not been asked about something it met (ADR-0055).

use std::collections::{HashMap, HashSet};

use sinephile_persistence::repositories::{
    CredentialRepository, DiscoveryRepository, EpisodeRepository, Facts, MediaRepository,
    RegionRepository,
};
use sinephile_persistence::{Db, DbError};
use sinephile_recommend::bandit::{Beliefs, Context, Outcome, Trial};
use sinephile_recommend::strategy::{
    self, Arm, Axis, Discovery, Label, Profile, Region, Taste, Title,
};

use crate::hybrid::Meaning;
use crate::modes::Mode;

/// "Close to a mode": among its thousand nearest titles in the meaning index.
pub const CLOSE_RANK: usize = 1_000;

/// The most acclaimed films in the pool: 3,600 or so clear the lower of the two bars.
const ACCLAIMED: i64 = 6_000;

fn unit(v: &[f32]) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n == 0.0 {
        Vec::new()
    } else {
        v.iter().map(|x| x / n).collect()
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A title as the arms see it, from its facts.
fn title_of(f: &Facts, near: Vec<f32>, loved_with: Vec<(i64, f32)>) -> Title {
    Title {
        id: f.id,
        near,
        rating: f.rating,
        votes: f.votes.unwrap_or(0),
        year: f.year,
        genres: f.genres.clone(),
        countries: f.countries.clone(),
        movements: f.movements.clone(),
        loved_with,
    }
}

/// What the arms choose from and the profile they choose for.
pub struct Gathered {
    pub profile: Profile,
    pub titles: Vec<Title>,
}

/// What gathering reads that does not depend on the profile, kept between calls. A Home
/// keeps one for one build; the evaluation keeps one across thousands of users, whose pools
/// overlap. (Facts change at most hourly, when countries are asked.)
#[derive(Default)]
pub struct Cache {
    facts: HashMap<i64, Facts>,
    acclaimed: Option<Vec<i64>>,
}

impl Cache {
    /// Read the facts of whichever of `ids` are not kept yet.
    async fn read(&mut self, media: &MediaRepository<'_>, ids: &[i64]) -> Result<(), DbError> {
        let mut missing: Vec<i64> = ids
            .iter()
            .copied()
            .filter(|id| !self.facts.contains_key(id))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        for f in media.facts(&missing).await? {
            self.facts.insert(f.id, f);
        }
        Ok(())
    }
}

/// Gather for a profile: `modes` and `met` from its taste model, `loved_with` Phase 17's
/// neighbours of what it loved (`hybrid::loved_with`).
pub async fn gather(
    db: &Db,
    semantic: &mut (dyn Meaning + Send),
    modes: &[Mode],
    met: &HashSet<i64>,
    loved_with: &HashMap<i64, Vec<(i64, f32)>>,
    cache: &mut Cache,
) -> Result<Gathered, DbError> {
    let media = MediaRepository::new(db);

    // What it met, an episode as its series.
    let met_ids: Vec<i64> = met.iter().copied().collect();
    let series: HashMap<i64, i64> = EpisodeRepository::new(db)
        .series_of_many(&met_ids)
        .await?
        .into_iter()
        .collect();
    let mut met_titles: Vec<i64> = met_ids
        .iter()
        .map(|id| series.get(id).copied().unwrap_or(*id))
        .collect();
    met_titles.sort_unstable();
    met_titles.dedup();
    cache.read(&media, &met_titles).await?;
    let met_facts = &cache.facts;
    let regions_of = |id: &i64| -> Vec<Region> {
        met_facts
            .get(id)
            .map(|f| title_of(f, Vec::new(), Vec::new()).regions())
            .unwrap_or_default()
    };
    let mut profile = Profile {
        met: met_titles.iter().flat_map(regions_of).collect(),
        met_titles: met_titles.len(),
        ..Default::default()
    };
    if !RegionRepository::new(db)
        .unasked(&met_titles)
        .await?
        .is_empty()
    {
        profile.unknown.extend([Axis::Country, Axis::Movement]);
    }

    // Each mode's thousand nearest: the close, and how close is close.
    let everything = HashSet::new();
    let mut pool: Vec<i64> = Vec::new();
    let mut seen: HashSet<i64> = met.iter().chain(&met_titles).copied().collect();
    for mode in modes {
        let nearest = semantic.nearest(&mode.direction, CLOSE_RANK, &everything);
        let close = nearest
            .last()
            .and_then(|id| semantic.vector(*id))
            .map_or(1.0, |v| dot(&mode.direction, &unit(&v)));
        for id in nearest {
            if seen.insert(id) {
                pool.push(id);
            }
        }
        // The members' regions, an episode as its series as above.
        let members: Vec<i64> = mode
            .members
            .iter()
            .map(|id| series.get(id).copied().unwrap_or(*id))
            .collect();
        profile.modes.push(Taste {
            members: mode.members.clone(),
            close,
            regions: members.iter().flat_map(regions_of).collect(),
        });
    }

    // The acclaimed, and what people who loved the same loved.
    let acclaimed = match &cache.acclaimed {
        Some(ids) => ids.clone(),
        None => {
            let ids = media
                .acclaimed(
                    strategy::BLIND_RATING.min(strategy::CANON_RATING),
                    strategy::BLIND_VOTES.min(strategy::CANON_VOTES),
                    ACCLAIMED,
                )
                .await?;
            cache.acclaimed = Some(ids.clone());
            ids
        }
    };
    let mut loved: Vec<i64> = loved_with.keys().copied().collect();
    loved.sort_unstable();
    for id in acclaimed.into_iter().chain(loved) {
        if seen.insert(id) {
            pool.push(id);
        }
    }

    cache.read(&media, &pool).await?;
    let titles = pool
        .iter()
        .filter_map(|id| cache.facts.get(id))
        .map(|f| {
            let near = semantic
                .vector(f.id)
                .map(|v| unit(&v))
                .filter(|v| !v.is_empty())
                .map(|v| modes.iter().map(|m| dot(&m.direction, &v)).collect())
                .unwrap_or_default();
            let with = loved_with.get(&f.id).cloned().unwrap_or_default();
            title_of(f, near, with)
        })
        .collect();
    Ok(Gathered { profile, titles })
}

/// Where a profile's Comfort to Adventure setting is kept, 0 (Comfort) to 1 (Adventure).
pub const BUDGET_KEY: &str = "discovery.budget";
/// Until a profile moves it: the middle.
pub const BUDGET_DEFAULT: f64 = 0.5;
/// How long "not right now" keeps a title off Home.
pub const NOT_NOW_DAYS: f64 = 14.0;
/// The slots a Home shares out between the arms (`bandit::allocate`): the taste rail shows at
/// most twenty of the familiar's, and discovery between twelve (the floor) and 48.
pub const SLOTS: usize = 60;
/// The fewest slots a discovery rail is given: a rail's worth.
pub const LEAST: usize = 6;

/// A profile's Comfort to Adventure setting.
pub async fn budget(db: &Db, profile: i64) -> Result<f64, DbError> {
    Ok(CredentialRepository::new(db)
        .get(profile, BUDGET_KEY)
        .await?
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .map_or(BUDGET_DEFAULT, |v| v.clamp(0.0, 1.0)))
}

/// Set it, 0 to 1.
pub async fn set_budget(db: &Db, profile: i64, value: f64) -> Result<(), DbError> {
    let value = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        BUDGET_DEFAULT
    };
    CredentialRepository::new(db)
        .set(profile, BUDGET_KEY, &format!("{value:.2}"))
        .await
}

/// Record a dismissal (`"not_for_me"`, `"not_right_now"` or `"already_seen"`) said at `hour`
/// (local, 0 to 23), in that part of the day.
pub async fn dismiss(
    db: &Db,
    profile: i64,
    id: i64,
    kind: &str,
    hour: Option<u32>,
) -> Result<(), DbError> {
    let context = hour.map_or(Context::Evening, Context::of_hour);
    DiscoveryRepository::new(db)
        .dismiss(profile, id, kind, context.key())
        .await
}

/// An event's name as the bandit's outcome.
fn outcome(event: &str) -> Option<Outcome> {
    Some(match event {
        "completed" => Outcome::Completed,
        "played" => Outcome::Played,
        "abandoned" => Outcome::AbandonedEarly,
        "watchlisted" => Outcome::Watchlisted,
        "not_for_me" => Outcome::NotForMe,
        "not_right_now" => Outcome::NotRightNow,
        "already_seen" => Outcome::AlreadySeen,
        _ => return None,
    })
}

/// What the bandit knows of a profile at `today` (days since 1970), and the titles Home
/// leaves out for it: "not for me" and "already seen" always, "not right now" for
/// [`NOT_NOW_DAYS`]. Each shown title counts once, by the strongest thing that came of it
/// (`Outcome::strongest`), in the part of the day that happened in, aged from when.
pub async fn memory(db: &Db, profile: i64, today: f64) -> Result<(Beliefs, HashSet<i64>), DbError> {
    let repo = DiscoveryRepository::new(db);
    let age = |at: &str| crate::model::day_of(at).map_or(0.0, |d| (today - d).max(0.0));
    let mut by_title: HashMap<i64, Vec<(Arm, Context, Outcome, f64)>> = HashMap::new();
    for h in repo.happened(profile).await? {
        let (Some(arm), Some(context), Some(o)) = (
            Arm::from_key(&h.arm),
            Context::from_key(&h.context),
            outcome(&h.event),
        ) else {
            continue;
        };
        by_title
            .entry(h.media_item_id)
            .or_default()
            .push((arm, context, o, age(&h.at)));
    }
    let mut trials = Vec::new();
    for events in by_title.values() {
        let outcomes: Vec<Outcome> = events.iter().map(|e| e.2).collect();
        let Some(strongest) = Outcome::strongest(&outcomes) else {
            continue;
        };
        if let Some(&(arm, context, outcome, age_days)) = events.iter().find(|e| e.2 == strongest) {
            trials.push(Trial {
                arm,
                context,
                outcome,
                age_days,
            });
        }
    }
    let hidden = repo
        .dismissed(profile)
        .await?
        .into_iter()
        .filter(|(_, kind, at)| kind != "not_right_now" || age(at) < NOT_NOW_DAYS)
        .map(|(id, _, _)| id)
        .collect();
    Ok((Beliefs::from_trials(&trials), hidden))
}

/// One discovery rail: the arm, its title, its picks.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveryRail {
    pub arm: Arm,
    pub title: String,
    pub picks: Vec<Discovery>,
}

/// The fewest picks worth a rail: three cards look like an accident.
pub const MIN_PICKS: usize = 5;

/// Every discovery arm's rail of up to `k`, none repeating a title in `taken` or another
/// rail, in [`Arm::ALL`]'s order; an arm with fewer than [`MIN_PICKS`] is left out.
pub fn rails(gathered: &Gathered, k: usize, taken: &HashSet<i64>) -> Vec<DiscoveryRail> {
    let plan: Vec<(Arm, usize)> = Arm::ALL
        .into_iter()
        .filter(|a| *a != Arm::Familiar)
        .map(|a| (a, k))
        .collect();
    planned(gathered, &plan, taken)
}

/// The rails of a plan (the bandit's: `bandit::allocate`), in its order, each arm its number
/// of picks; an arm with fewer than [`MIN_PICKS`] is left out.
pub fn planned(
    gathered: &Gathered,
    plan: &[(Arm, usize)],
    taken: &HashSet<i64>,
) -> Vec<DiscoveryRail> {
    strategy::choose(&gathered.titles, &gathered.profile, plan, taken)
        .into_iter()
        .filter(|(_, picks)| picks.len() >= MIN_PICKS)
        .map(|(arm, picks)| DiscoveryRail {
            arm,
            title: rail_title(arm, &picks),
            picks,
        })
        .collect()
}

/// A genre as "a western", for "Like Stalker, but a western".
fn genre_one(genre: &str) -> String {
    match genre {
        "Action" => "an action film".into(),
        "Adventure" => "an adventure".into(),
        "Animation" => "animated".into(),
        "Biography" => "a biography".into(),
        "Children" => "for children".into(),
        "Comedy" => "a comedy".into(),
        "Crime" => "a crime film".into(),
        "Documentary" => "a documentary".into(),
        "Drama" => "a drama".into(),
        "Family" => "a family film".into(),
        "Fantasy" => "a fantasy".into(),
        "Film-Noir" => "a film noir".into(),
        "History" => "a historical film".into(),
        "Horror" => "horror".into(),
        "Music" => "about music".into(),
        "Musical" => "a musical".into(),
        "Mystery" => "a mystery".into(),
        "Romance" => "a romance".into(),
        "Sci-Fi" => "science fiction".into(),
        "Sport" => "about sport".into(),
        "Thriller" => "a thriller".into(),
        "War" => "a war film".into(),
        "Western" => "a western".into(),
        other => other.to_lowercase(),
    }
}

/// A genre as "westerns", for "Your blind spot: westerns".
fn genre_many(genre: &str) -> String {
    match genre {
        "Action" => "action films".into(),
        "Adventure" => "adventure films".into(),
        "Animation" => "animation".into(),
        "Biography" => "biographies".into(),
        "Children" => "children's films".into(),
        "Comedy" => "comedies".into(),
        "Crime" => "crime films".into(),
        "Documentary" => "documentaries".into(),
        "Drama" => "dramas".into(),
        "Family" => "family films".into(),
        "Fantasy" => "fantasy".into(),
        "Film-Noir" => "film noir".into(),
        "History" => "historical films".into(),
        "Horror" => "horror".into(),
        "Music" => "films about music".into(),
        "Musical" => "musicals".into(),
        "Mystery" => "mysteries".into(),
        "Romance" => "romances".into(),
        "Sci-Fi" => "science fiction".into(),
        "Sport" => "sports films".into(),
        "Thriller" => "thrillers".into(),
        "War" => "war films".into(),
        "Western" => "westerns".into(),
        other => other.to_lowercase(),
    }
}

/// A rail's name, from what its picks are. A blind spot's names its region; every other
/// arm's names what the arm does, and each card says the rest.
pub fn rail_title(arm: Arm, picks: &[Discovery]) -> String {
    match arm {
        Arm::Familiar => "Where your taste points".into(),
        Arm::Stretch => "A stretch, but we think you're ready".into(),
        Arm::Bridge => "Between your tastes".into(),
        Arm::BlindSpot => match picks.first().map(|d| &d.label) {
            Some(Label::BlindSpot(Region::Decade(d))) => {
                format!("You've never watched anything from the {d}s")
            }
            Some(Label::BlindSpot(Region::Genre(g))) => {
                format!("Your blind spot: {}", genre_many(g))
            }
            Some(Label::BlindSpot(Region::Country(c) | Region::Movement(c))) => {
                format!("Your blind spot: {c}")
            }
            _ => "Your blind spots".into(),
        },
        Arm::Canon => "Acclaimed, and not yet seen".into(),
        Arm::DeepCut => "Highly rated, little seen".into(),
    }
}

/// 12400 as "12,400".
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(d);
    }
    out
}

/// A pick's label in words, naming only what is true of it (E3): `titles` names the profile's
/// titles, `facts` the pick's own rating and votes.
pub fn note(
    pick: &Discovery,
    profile: &Profile,
    titles: &HashMap<i64, String>,
    facts: Option<&Title>,
    nearest: &Nearest,
) -> String {
    // A taste is named by its loved title nearest the pick, else by its strongest: a taste
    // made of Tokyo Story and three Ghibli films is not "close to Tokyo Story" for Kiki's
    // Delivery Service (seen in the real window, 2026-11-20).
    let mode_name = |m: usize| -> Option<&str> {
        nearest
            .get(&(m, pick.id))
            .or_else(|| profile.modes.get(m).and_then(|t| t.members.first()))
            .and_then(|id| titles.get(id))
            .map(String::as_str)
    };
    let rated = || {
        facts.and_then(|t| {
            t.rating.map(|r| {
                format!(
                    "Rated {:.1} by {} people",
                    r as f64 / 10.0,
                    thousands(t.votes)
                )
            })
        })
    };
    match &pick.label {
        Label::Stretch { mode, region } => {
            let step = match region {
                Region::Decade(d) => format!("from the {d}s"),
                Region::Genre(g) => genre_one(g),
                Region::Country(c) => format!("from {c}"),
                Region::Movement(m) => format!("from the {m}"),
            };
            match mode_name(*mode) {
                Some(name) => format!("Like {name}, but {step}"),
                None => format!("Close to your taste, but {step}"),
            }
        }
        Label::Between { a, b } => match (mode_name(*a), mode_name(*b)) {
            (Some(a), Some(b)) => format!("Between {a} and {b}"),
            _ => "Between two of your tastes".into(),
        },
        Label::BlindSpot(_) => rated().unwrap_or_else(|| "Acclaimed".into()),
        Label::Canon { mode } => match mode_name(*mode) {
            Some(name) => format!("Acclaimed, and close to {name}"),
            None => "Acclaimed, and close to your taste".into(),
        },
        Label::DeepCutNear { mode } => match mode_name(*mode) {
            Some(name) => format!("Highly rated, little seen, close to {name}"),
            None => "Highly rated, little seen, close to your taste".into(),
        },
        Label::DeepCutLoved { loved } => match titles.get(loved) {
            Some(name) => format!("Loved by people who loved {name}, and little seen"),
            None => "Loved by people who share your taste, and little seen".into(),
        },
    }
}

/// For each pick whose label names a taste, by (mode, pick): the loved title of that taste
/// nearest the pick in meaning.
pub type Nearest = HashMap<(usize, i64), i64>;

/// The loved titles a taste's label is said by: of each named mode's first [`MEMBERS_ASKED`]
/// members, the one nearest each pick.
pub fn nearest_members(
    semantic: &mut (dyn Meaning + Send),
    profile: &Profile,
    rails: &[DiscoveryRail],
) -> Nearest {
    let mut vectors: HashMap<i64, Vec<f32>> = HashMap::new();
    let mut vector = |id: i64, semantic: &mut (dyn Meaning + Send)| -> Option<Vec<f32>> {
        if let Some(v) = vectors.get(&id) {
            return Some(v.clone());
        }
        let v = unit(&semantic.vector(id)?);
        vectors.insert(id, v.clone());
        Some(v)
    };
    let mut out = Nearest::new();
    for d in rails.iter().flat_map(|r| &r.picks) {
        let modes: Vec<usize> = match d.label {
            Label::Stretch { mode, .. } | Label::Canon { mode } | Label::DeepCutNear { mode } => {
                vec![mode]
            }
            Label::Between { a, b } => vec![a, b],
            Label::BlindSpot(_) | Label::DeepCutLoved { .. } => continue,
        };
        let Some(at) = vector(d.id, semantic) else {
            continue;
        };
        for m in modes {
            let Some(taste) = profile.modes.get(m) else {
                continue;
            };
            let best = taste
                .members
                .iter()
                .take(MEMBERS_ASKED)
                .filter_map(|id| Some((*id, dot(&at, &vector(*id, semantic)?))))
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
            if let Some((id, _)) = best {
                out.insert((m, d.id), id);
            }
        }
    }
    out
}

/// How many of a taste's members, strongest first, a label may be said by.
pub const MEMBERS_ASKED: usize = 30;

/// The profile's titles a note may name: each mode's first member, the members nearest the
/// picks, and the loved titles deep cuts point to.
pub fn named(rails: &[DiscoveryRail], profile: &Profile, nearest: &Nearest) -> Vec<i64> {
    profile
        .modes
        .iter()
        .filter_map(|t| t.members.first().copied())
        .chain(nearest.values().copied())
        .chain(rails.iter().flat_map(|r| {
            r.picks.iter().filter_map(|d| match d.label {
                Label::DeepCutLoved { loved } => Some(loved),
                _ => None,
            })
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile {
            modes: vec![
                Taste {
                    members: vec![1, 3],
                    close: 0.6,
                    regions: HashSet::new(),
                },
                Taste {
                    members: vec![2],
                    close: 0.6,
                    regions: HashSet::new(),
                },
            ],
            met_titles: 20,
            ..Default::default()
        }
    }

    fn titles() -> HashMap<i64, String> {
        [(1, "Stalker"), (2, "Spirited Away"), (3, "Mirror")]
            .map(|(i, t)| (i, t.to_string()))
            .into()
    }

    fn pick(label: Label) -> Discovery {
        Discovery { id: 9, label }
    }

    #[test]
    fn each_label_says_what_it_claims() {
        let p = profile();
        let t = titles();
        let say = |label| note(&pick(label), &p, &t, None, &Nearest::new());
        assert_eq!(
            say(Label::Stretch {
                mode: 0,
                region: Region::Decade(1990)
            }),
            "Like Stalker, but from the 1990s"
        );
        assert_eq!(
            say(Label::Stretch {
                mode: 0,
                region: Region::Genre("Western".into())
            }),
            "Like Stalker, but a western"
        );
        assert_eq!(
            say(Label::Stretch {
                mode: 1,
                region: Region::Country("Iran".into())
            }),
            "Like Spirited Away, but from Iran"
        );
        assert_eq!(
            say(Label::Between { a: 0, b: 1 }),
            "Between Stalker and Spirited Away"
        );
        assert_eq!(
            say(Label::Canon { mode: 1 }),
            "Acclaimed, and close to Spirited Away"
        );
        assert_eq!(
            say(Label::DeepCutNear { mode: 0 }),
            "Highly rated, little seen, close to Stalker"
        );
        assert_eq!(
            say(Label::DeepCutLoved { loved: 3 }),
            "Loved by people who loved Mirror, and little seen"
        );
        assert_eq!(
            say(Label::DeepCutLoved { loved: 404 }),
            "Loved by people who share your taste, and little seen"
        );
        assert_eq!(
            say(Label::Canon { mode: 7 }),
            "Acclaimed, and close to your taste"
        );
        let rated = Title {
            rating: Some(81),
            votes: 25_400,
            ..Default::default()
        };
        assert_eq!(
            note(
                &pick(Label::BlindSpot(Region::Decade(1950))),
                &p,
                &t,
                Some(&rated),
                &Nearest::new()
            ),
            "Rated 8.1 by 25,400 people"
        );
    }

    #[test]
    fn a_taste_is_named_by_its_title_nearest_the_pick() {
        let p = profile();
        let t = titles();
        let nearest: Nearest = [((0, 9), 3)].into();
        assert_eq!(
            note(&pick(Label::Canon { mode: 0 }), &p, &t, None, &nearest),
            "Acclaimed, and close to Mirror"
        );
        assert_eq!(
            note(&pick(Label::Between { a: 0, b: 1 }), &p, &t, None, &nearest),
            "Between Mirror and Spirited Away"
        );
    }

    #[test]
    fn a_blind_spot_rail_names_its_region() {
        let rail = |region| rail_title(Arm::BlindSpot, &[pick(Label::BlindSpot(region))]);
        assert_eq!(
            rail(Region::Decade(1950)),
            "You've never watched anything from the 1950s"
        );
        assert_eq!(
            rail(Region::Genre("Western".into())),
            "Your blind spot: westerns"
        );
        assert_eq!(
            rail(Region::Movement("Iranian New Wave".into())),
            "Your blind spot: Iranian New Wave"
        );
        assert_eq!(
            rail(Region::Country("Iran".into())),
            "Your blind spot: Iran"
        );
        assert_eq!(rail_title(Arm::BlindSpot, &[]), "Your blind spots");
    }
}
