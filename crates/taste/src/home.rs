//! Home's rails (Phase 14, ordered by the author in Phase 16; Phase 19 overhauls Home):
//! what a profile has started; new episodes of the shows it follows; what is in its
//! library; this year's new releases, by its taste; where its taste points; what it saved;
//! and what is widely loved, which needs nothing but the offline catalogue, so Home is never
//! empty, with no key, no sources and no history. A kids profile's rails hold only what
//! suits children. Empty rails are left out rather than shown empty.

use std::collections::{HashMap, HashSet};

use sinephile_persistence::repositories::{
    CredentialRepository, DiscoveryRepository, EpisodeRepository, LibraryRepository,
    MediaRepository, ProfileRepository,
};
use sinephile_persistence::{Db, DbError};
use sinephile_recommend::bandit::Context;
use sinephile_recommend::strategy::Arm;
use sinephile_search_engine::Semantic;

/// Today, UTC, as "YYYY-MM-DD": what the taste model and the discovery engine age things by.
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let days = secs.div_euclid(86_400);
    // Only the day matters here: a date the model can read.
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Where `sinephile_catalogue::now` keeps a profile's "showing now" list.
const NOW_KEY: &str = "tmdb.now";

/// The profile's "showing now" list, in TMDB's order; empty without a key or a list.
async fn now_showing(db: &Db, profile: i64) -> Result<Vec<i64>, DbError> {
    let raw = sinephile_persistence::repositories::CredentialRepository::new(db)
        .get(profile, NOW_KEY)
        .await?;
    Ok(raw
        .and_then(|r| serde_json::from_str::<serde_json::Value>(&r).ok())
        .and_then(|v| {
            v["ids"]
                .as_array()
                .map(|a| a.iter().filter_map(serde_json::Value::as_i64).collect())
        })
        .unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rail {
    /// "continue" | "new_episodes" | "library" | "new" | "rising" | "taste" | a discovery
    /// arm's key ("stretch", "bridge", "blind_spot", "canon", "deep_cut") | "watchlist" |
    /// "loved"
    pub key: &'static str,
    /// A blind spot's names its region ("Your blind spot: Iranian New Wave"), so not fixed.
    pub title: String,
    pub items: Vec<i64>,
    /// A line for an item beyond its own facts: "S2 E5" for a series with a new episode.
    pub notes: HashMap<i64, String>,
    /// How far into it, 0 to 1: Continue watching's bar (Phase 16).
    pub progress: HashMap<i64, f64>,
}

/// New episodes count if the catalogue gained them in this many days.
const NEW_EPISODE_DAYS: i64 = 30;
/// A new release has this many votes: it has come out and people are watching it.
const NEW_RELEASE_VOTES: i64 = 1_000;

const PER_RAIL: usize = 20;

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
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// Home's "Where your taste points" alone (Phase 17): what the profile's settings preview
/// under the same name, so the two never disagree. (It was Phase 14's single averaged seed,
/// which named different films: seen in the real window, 17.6.)
pub async fn taste_rail(
    db: &Db,
    semantic: Option<&mut Semantic>,
    profile: i64,
    kids: bool,
    love: Option<&str>,
    cache: &crate::model::ModelCache,
) -> Result<Vec<i64>, DbError> {
    Ok(rails(db, semantic, profile, kids, love, cache)
        .await?
        .into_iter()
        .find(|r| r.key == "taste")
        .map(|r| r.items)
        .unwrap_or_default())
}

/// Home's rails as built, and what they were built from: [`recent`] gives them back while
/// that is still so.
pub(crate) struct Built {
    fingerprint: String,
    context: Context,
    kids: bool,
    love: Option<String>,
    at: std::time::Instant,
    rails: Vec<Rail>,
}

/// How long rails stay current without a change to the profile.
pub const RECENT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// The rails Home last built for this profile, if they are still what it would build now:
/// nothing the taste model reads has changed (the signal fingerprint), the same part of the
/// day, the same "what you love", and built within [`RECENT`]. Surprise me picks from these
/// rather than building Home again (the author, 2026-11-21: "very slow"; 377 ms a press warm,
/// 3.7 s cold, waiting on the search engine besides).
pub async fn recent(
    db: &Db,
    profile: i64,
    kids: bool,
    love: Option<&str>,
    cache: &crate::model::ModelCache,
    ask: Ask,
) -> Result<Option<Vec<Rail>>, DbError> {
    let context = ask.hour.map_or(Context::Evening, Context::of_hour);
    let fingerprint = ProfileRepository::new(db)
        .signal_fingerprint(profile)
        .await?;
    let homes = cache
        .homes
        .lock()
        .map_err(|_| DbError::Invalid("poisoned".into()))?;
    Ok(homes
        .get(&profile)
        .filter(|b| {
            b.fingerprint == fingerprint
                && b.context == context
                && b.kids == kids
                && b.love.as_deref() == love
                && b.at.elapsed() < RECENT
        })
        .map(|b| b.rails.clone()))
}

/// What a Home is asked for beyond the profile (Phase 18).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ask {
    /// The viewer's local hour, 0 to 23: the part of the day the discovery engine's bandit
    /// learns and chooses in. `None` is taken as the evening.
    pub hour: Option<u32>,
}

/// Home's rails at an unknown hour: [`rails_at`] with nothing asked.
pub async fn rails(
    db: &Db,
    semantic: Option<&mut Semantic>,
    profile: i64,
    kids: bool,
    love: Option<&str>,
    cache: &crate::model::ModelCache,
) -> Result<Vec<Rail>, DbError> {
    rails_at(db, semantic, profile, kids, love, cache, Ask::default()).await
}

/// Home's rails, for what `ask` says.
pub async fn rails_at(
    db: &Db,
    semantic: Option<&mut Semantic>,
    profile: i64,
    kids: bool,
    love: Option<&str>,
    cache: &crate::model::ModelCache,
    ask: Ask,
) -> Result<Vec<Rail>, DbError> {
    let hour = ask.hour;
    // A kids profile leaves out much of what is chosen, so more is chosen.
    let more = if kids { 6 } else { 1 };
    let profiles = ProfileRepository::new(db);
    // What these rails are built from, for `recent`.
    let fingerprint = profiles.signal_fingerprint(profile).await?;
    let media = MediaRepository::new(db);
    let suits = |ids: Vec<i64>| async move {
        if !kids {
            return Ok::<_, DbError>(ids);
        }
        let ok = MediaRepository::new(db).suitable_for_children(&ids).await?;
        Ok(ids.into_iter().filter(|id| ok.contains(id)).collect())
    };

    // One card per film or show (Phase 16): a show's card is the show, saying which episode.
    let continuing = crate::continuing::continuing(db, profile, PER_RAIL).await?;
    let started: Vec<i64> = continuing.iter().map(|c| c.card).collect();
    let continue_notes: HashMap<i64, String> = continuing
        .iter()
        .filter_map(|c| c.note.clone().map(|n| (c.card, n)))
        .collect();
    let continue_progress: HashMap<i64, f64> = continuing
        .iter()
        .filter_map(|c| c.progress.map(|p| (c.card, p)))
        .collect();
    let liked = profiles.liked(profile).await?;
    let saved = profiles.watchlist(profile).await?;
    let today = crate::model::day_of(&now_iso()).unwrap_or(0.0);
    // What the discovery engine has learned of this profile, and what it dismissed: a
    // dismissed title is never recommended, in any rail (Phase 18).
    let (beliefs, hidden) = crate::discover::memory(db, profile, today).await?;
    let seen: HashSet<i64> = liked
        .iter()
        .map(|(id, _)| *id)
        .chain(continuing.iter().flat_map(|c| [c.card, c.plays]))
        .chain(hidden.iter().copied())
        .collect();
    let context = hour.map_or(Context::Evening, Context::of_hour);

    // The taste model (Phase 15), with the profile's corrections: built once, for the new
    // releases and the taste rail.
    let year = (today / 365.2425 + 1970.0).floor() as i64;
    let mut model = None;
    if let Some(s) = semantic {
        let (mut modes, mut met) = cache.get(db, s, profile, today).await?;
        met.extend(seen.iter().copied());
        crate::describe::apply(
            &mut modes,
            &crate::describe::corrections(db, profile).await?,
        );
        if let Some(text) = love.map(str::trim).filter(|t| !t.is_empty()) {
            if let Ok(v) = s.embedder.embed_query(text) {
                crate::model::add_words(&mut modes, v);
            }
        }
        model = Some((s, modes, met));
    }

    // New episodes of followed series: one card per series, opening the series.
    let mut episode_notes = HashMap::new();
    let new_episodes: Vec<i64> = EpisodeRepository::new(db)
        .new_episodes(profile, NEW_EPISODE_DAYS, year - 1, PER_RAIL as i64)
        .await?
        .into_iter()
        .map(|(series, _, season, number)| {
            let note = match (season, number) {
                (Some(s), Some(n)) => format!("New: S{s} E{n}"),
                _ => "New episode".to_string(),
            };
            episode_notes.insert(series, note);
            series
        })
        .collect();

    // The library, most recently identified first.
    let library = LibraryRepository::new(db)
        .recently_added(PER_RAIL as i64)
        .await?;

    // This year's releases (last year's too while this year's are few), by taste where
    // the model has a vector for them, by votes otherwise. The catalogue has years, not
    // dates; daily refreshes keep the votes moving with what people watch.
    let mut released = media.new_releases(year, NEW_RELEASE_VOTES, 300).await?;
    if released.len() < PER_RAIL {
        released = media.new_releases(year - 1, NEW_RELEASE_VOTES, 300).await?;
    }
    let new_releases: Vec<i64> = match &model {
        Some((s, modes, met)) if !modes.is_empty() => {
            let fresh: Vec<&(i64, i64)> = released
                .iter()
                .filter(|(id, _)| !met.contains(id))
                .collect();
            let max = fresh.iter().map(|(_, v)| *v).max().unwrap_or(1);
            let with: Vec<(i64, Vec<f32>, i64)> = fresh
                .iter()
                .filter_map(|(id, v)| {
                    let vec = s.index.vector(*id)?;
                    Some((*id, vec.iter().map(|x| f32::from(*x)).collect(), *v))
                })
                .collect();
            let mut ranked = crate::model::rank(
                modes,
                &with,
                max,
                crate::model::POPULARITY_PRIOR,
                PER_RAIL * 6,
            );
            let have: HashSet<i64> = ranked.iter().copied().collect();
            ranked.extend(
                fresh
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| !have.contains(id)),
            );
            ranked
        }
        _ => released
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !seen.contains(id))
            .collect(),
    };

    // With the profile's own TMDB key, what is in cinemas and on the air this week comes
    // first (`sinephile_catalogue::now`); the catalogue alone knows only the year.
    let showing: Vec<i64> = now_showing(db, profile)
        .await?
        .into_iter()
        .filter(|id| !seen.contains(id))
        .collect();
    let new_releases: Vec<i64> = if showing.is_empty() {
        new_releases
    } else {
        let have: HashSet<i64> = showing.iter().copied().collect();
        showing
            .into_iter()
            .chain(new_releases.into_iter().filter(|id| !have.contains(id)))
            .collect()
    };

    // Rising now (ADR-0054 step 6): what gained the most ratings at the catalogue's last
    // refresh, kept by the refresh itself; the profile's own titles left out.
    let mut rising_notes = HashMap::new();
    let rising: Vec<i64> = media
        .kept_rising()
        .await?
        .into_iter()
        .filter(|(id, _)| !seen.contains(id))
        .map(|(id, gained)| {
            rising_notes.insert(id, format!("+{} ratings", thousands(gained)));
            id
        })
        .collect();

    // Where the profile's taste points (Phase 17): the hybrid ranker over meaning, people who
    // loved the same, and popularity, with the profile's axes; each pick says why.
    let mut taste_notes: HashMap<i64, String> = HashMap::new();
    // And the discovery rails (Phase 18): each arm's picks, none in another rail, as many as
    // the bandit's plan gives each; the taste rail as many as the plan gives the familiar.
    let mut discovery: Vec<(crate::discover::DiscoveryRail, HashMap<i64, String>)> = Vec::new();
    let mut limits: HashMap<&'static str, usize> = HashMap::new();
    // And today's mood shelves (Phase 18): found by meaning, none repeating another rail.
    let mut moods: Vec<(crate::mood::Mood, Vec<i64>)> = Vec::new();
    // Mutably: the meaning index is held across database awaits, which a shared reference
    // could only do if it were Sync; it is Send.
    let taste = match &mut model {
        Some((s, modes, met)) => {
            let modes: &[crate::modes::Mode] = modes;
            let mut met = met.clone();
            met.extend(new_releases.iter().take(PER_RAIL).copied());
            let weighed = crate::modes::weigh(&crate::model::signals(db, profile, today).await?);
            let said = crate::Statements::parse(
                CredentialRepository::new(db)
                    .get(profile, crate::STATEMENTS)
                    .await?
                    .as_deref(),
            );
            let axes = crate::axes::axes(db, &weighed, &said).await?;
            let library_titles = LibraryRepository::new(db);
            let playable: HashSet<i64> = library_titles
                .library_titles(false)
                .await?
                .into_iter()
                .chain(library_titles.library_titles(true).await?)
                .collect();
            // The bandit's plan first (Phase 18): it says how many the familiar gets. The same
            // plan all hour, so Home does not reshuffle at every refresh.
            let seed = (profile as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ ((today as u64) * 24 + u64::from(hour.unwrap_or(20)));
            let plan = sinephile_recommend::bandit::allocate(
                &beliefs,
                context,
                crate::discover::budget(db, profile).await?,
                crate::discover::SLOTS,
                crate::discover::LEAST,
                &mut sinephile_recommend::bandit::Rng::new(seed),
            );
            // The familiar's whole share is shown: the taste rail, and past its twenty a second
            // rail. (Showing only twenty of thirty gave discovery more of Home than the plan
            // meant: docs/eval-results.md, 18.2.)
            let familiar = plan.first().map_or(PER_RAIL, |(_, n)| *n);
            limits.insert("taste", familiar.min(PER_RAIL));
            limits.insert("taste_more", familiar.saturating_sub(PER_RAIL));
            for (arm, n) in plan.iter().filter(|(a, _)| *a != Arm::Familiar) {
                limits.insert(arm.key(), (*n).min(PER_RAIL));
            }
            let want = familiar.max(1) * more;
            let (picks, makers) =
                crate::hybrid::rail(db, &mut **s, modes, &met, &weighed, &axes, &playable, want)
                    .await?;
            // The titles the reasons name: what was loved, and what each mode is made of.
            let named: Vec<i64> = picks
                .iter()
                .flat_map(|p| match &p.reason {
                    sinephile_recommend::rank::Reason::LovedWith(ids) => ids.clone(),
                    sinephile_recommend::rank::Reason::MadeBy { loved, .. } => loved.clone(),
                    _ => Vec::new(),
                })
                .chain(modes.iter().flat_map(|m| m.members.iter().take(2).copied()))
                .collect();
            let titles: HashMap<i64, String> = media
                .facts(&named)
                .await?
                .into_iter()
                .map(|f| (f.id, f.title))
                .collect();
            for p in &picks {
                taste_notes.insert(p.id, crate::hybrid::note(p, modes, &titles, &makers));
            }
            let picked: Vec<i64> = picks.into_iter().map(|p| p.id).collect();

            // Never the same title in two rails: everything Home shows above or beside them.
            let taken: HashSet<i64> = met
                .iter()
                .chain(&picked)
                .chain(&saved)
                .chain(&library)
                .chain(&new_episodes)
                .chain(rising.iter().take(PER_RAIL))
                .copied()
                .collect();
            let loved = crate::hybrid::loved_with(db, &weighed, &met).await?;
            let mut facts = crate::discover::Cache::default();
            let gathered =
                crate::discover::gather(db, &mut **s, modes, &met, &loved, &mut facts).await?;
            // A kids profile asks each arm for more: what does not suit
            // children, or runs longer, is left out below.
            let asked: Vec<(Arm, usize)> = plan.iter().map(|(arm, n)| (*arm, n * more)).collect();
            let rails = crate::discover::planned(&gathered, &asked, &taken);
            let nearest = crate::discover::nearest_members(&mut **s, &gathered.profile, &rails);
            let named = crate::discover::named(&rails, &gathered.profile, &nearest);
            let names: HashMap<i64, String> = media
                .facts(&named)
                .await?
                .into_iter()
                .map(|f| (f.id, f.title))
                .collect();
            let by_id: HashMap<i64, &sinephile_recommend::strategy::Title> =
                gathered.titles.iter().map(|t| (t.id, t)).collect();
            for rail in rails {
                let notes = rail
                    .picks
                    .iter()
                    .map(|d| {
                        let facts = by_id.get(&d.id).copied();
                        let words =
                            crate::discover::note(d, &gathered.profile, &names, facts, &nearest);
                        (d.id, words)
                    })
                    .collect();
                discovery.push((rail, notes));
            }

            let mut shelved: HashSet<i64> = taken.clone();
            shelved.extend(
                discovery
                    .iter()
                    .flat_map(|(r, _)| r.picks.iter().map(|d| d.id)),
            );
            for mood in crate::mood::today(today as i64) {
                let Some(vector) = cache
                    .moods
                    .get(&mood, |text| s.embedder.embed_query(text).ok())
                else {
                    continue;
                };
                let ids = crate::mood::shelf(
                    db,
                    &mut **s,
                    &mood,
                    &vector,
                    modes,
                    &shelved,
                    PER_RAIL * more,
                )
                .await?;
                shelved.extend(ids.iter().copied());
                moods.push((mood, ids));
            }
            picked
        }
        None => Vec::new(),
    };

    let shown: HashSet<i64> = seen
        .iter()
        .chain(taste.iter())
        .chain(
            discovery
                .iter()
                .flat_map(|(r, _)| r.picks.iter().map(|d| &d.id)),
        )
        .chain(moods.iter().flat_map(|(_, ids)| ids.iter()))
        .chain(saved.iter())
        .chain(new_releases.iter().take(PER_RAIL))
        .copied()
        .collect();
    let loved: Vec<i64> = media
        .most_voted_films(if kids { 2_000 } else { 200 })
        .await?
        .into_iter()
        .filter(|id| !shown.contains(id))
        .collect();

    let mut out = Vec::new();
    // Each rail: its key, its name, its titles, and the notes its cards may carry.
    type Listed<'a> = (
        &'static str,
        String,
        Vec<i64>,
        Option<&'a HashMap<i64, String>>,
    );
    let mut lists: Vec<Listed> = vec![
        (
            "continue",
            "Continue watching".into(),
            started,
            Some(&continue_notes),
        ),
        (
            "new_episodes",
            "New episodes of your shows".into(),
            new_episodes,
            Some(&episode_notes),
        ),
        ("library", "Your library".into(), library, None),
        ("new", "New releases".into(), new_releases, None),
        ("rising", "Rising now".into(), rising, Some(&rising_notes)),
        (
            "taste",
            "Where your taste points".into(),
            taste.clone(),
            Some(&taste_notes),
        ),
        (
            "taste_more",
            "More where your taste points".into(),
            taste,
            Some(&taste_notes),
        ),
    ];
    for (rail, notes) in &discovery {
        lists.push((
            rail.arm.key(),
            rail.title.clone(),
            rail.picks.iter().map(|d| d.id).collect(),
            Some(notes),
        ));
    }
    for (mood, ids) in &moods {
        lists.push((mood.key, mood.title.to_string(), ids.clone(), None));
    }
    lists.push(("watchlist", "On your watchlist".into(), saved, None));
    lists.push(("loved", "Widely loved".into(), loved, None));
    let mut recorded: Vec<(i64, &'static str)> = Vec::new();
    // What the taste rail shows, which its second rail continues after.
    let mut in_taste: HashSet<i64> = HashSet::new();
    for (key, title, ids, from) in lists {
        let limit = limits.get(key).copied().unwrap_or(PER_RAIL);
        let ids: Vec<i64> = if key == "taste_more" {
            ids.into_iter()
                .filter(|id| !in_taste.contains(id))
                .collect()
        } else {
            ids
        };
        let items: Vec<i64> = suits(ids).await?.into_iter().take(limit).collect();
        if key == "taste" {
            in_taste.extend(items.iter().copied());
        }
        let arm = if key == "taste" || key == "taste_more" {
            Some(Arm::Familiar)
        } else {
            Arm::from_key(key)
        };
        // A discovery rail of two or three looks like an accident.
        let short =
            arm.is_some_and(|a| a != Arm::Familiar) && items.len() < crate::discover::MIN_PICKS;
        if !items.is_empty() && !short {
            if let Some(arm) = arm {
                recorded.extend(items.iter().map(|id| (*id, arm.key())));
            }
            let notes = from.map_or_else(HashMap::new, |from| {
                items
                    .iter()
                    .filter_map(|id| from.get(id).map(|n| (*id, n.clone())))
                    .collect()
            });
            let progress = if key == "continue" {
                items
                    .iter()
                    .filter_map(|id| continue_progress.get(id).map(|p| (*id, *p)))
                    .collect()
            } else {
                HashMap::new()
            };
            out.push(Rail {
                key,
                title,
                items,
                notes,
                progress,
            });
        }
    }
    // Which arm showed what, and when: what the bandit learns from (Phase 18).
    DiscoveryRepository::new(db)
        .record_shown(profile, &recorded, context.key())
        .await?;
    if let Ok(mut homes) = cache.homes.lock() {
        homes.insert(
            profile,
            Built {
                fingerprint,
                context,
                kids,
                love: love.map(str::to_string),
                at: std::time::Instant::now(),
                rails: out.clone(),
            },
        );
    }
    Ok(out)
}
