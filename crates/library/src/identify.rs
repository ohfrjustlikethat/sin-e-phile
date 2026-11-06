//! What a file IS: its name, its folders and its length, weighed against the catalogue.
//!
//! No one signal is trusted alone. A name says "Heat"; the year says which Heat; the
//! duration says film or episode and which cut; a series must actually have the episode
//! the name claims. Each is a weak signal in [0, 1], and they are summed with fixed
//! weights. How sure the result is depends on how far the best candidate is ahead of the
//! next: two equally good readings are a question for the user, not a coin toss.
//! `docs/specs/library.md` has the method and the measured numbers.

use filename_parser::{comparable, parse, Parsed};
use sinephile_persistence::repositories::{
    CandidateFacts, EpisodeRepository, LibraryRepository, SearchRepository,
};
use sinephile_persistence::{Db, DbError, IdSource};

use crate::folders::{hint, FolderHint};

/// What is known about a file before it is identified.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evidence {
    /// The file's name.
    pub name: String,
    /// The folders between the library root and the file, outermost first.
    pub folders: Vec<String>,
    /// From the probe.
    pub duration_minutes: Option<f64>,
}

/// The weights of the signals. They sum to 1, so a score is in [0, 1].
pub const W_TITLE: f64 = 0.40;
pub const W_YEAR: f64 = 0.22;
/// "S01E10" cannot be a film: the kind outweighs a runtime that happens to fit.
pub const W_KIND: f64 = 0.18;
pub const W_RUNTIME: f64 = 0.12;
pub const W_POPULARITY: f64 = 0.08;

/// Added when a series has the very episode the name claims; the score is scaled by
/// `EPISODE_MISSING` when it does not.
pub const EPISODE_FOUND: f64 = 0.08;
pub const EPISODE_MISSING: f64 = 0.92;

/// How far ahead the best must be for full confidence: a lead of this much or more.
pub const DECISIVE_LEAD: f64 = 0.12;
/// At or above this confidence a match is accepted without asking; below it, the file
/// goes to the review queue. Set from `eval identify` (docs/eval-results.md, Phase 12):
/// the lowest threshold under 1% wrong among accepted was 0.615, at 0.97% — too close to
/// the line — so it sits where the tuning set measured 0.39%.
pub const ACCEPT_AT: f64 = 0.75;

/// A best score this high is fully plausible; at `IMPLAUSIBLE` or below, not at all.
pub const PLAUSIBLE: f64 = 0.80;
pub const IMPLAUSIBLE: f64 = 0.50;

/// Each signal's reading for one candidate, in [0, 1]; 0.5 is "no evidence either way".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signals {
    pub title: f64,
    pub year: f64,
    pub kind: f64,
    pub runtime: f64,
    pub popularity: f64,
    /// Whether the series has the claimed episode: `None` when there is nothing to check.
    pub episode_found: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub media_item_id: i64,
    pub kind: String,
    pub title: String,
    pub year: Option<i64>,
    pub score: f64,
    pub signals: Signals,
}

/// The episode a file holds, when the best candidate is a series and the catalogue has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeHit {
    pub episode_id: i64,
    /// How many consecutive episodes the file holds.
    pub covers: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Identification {
    pub parsed: Parsed,
    /// What was searched for: from the name, or from the folders when the name says too
    /// little.
    pub title: String,
    pub year: Option<u16>,
    pub season: Option<u32>,
    pub episodes: Vec<u32>,
    /// Best first, at most `KEEP`.
    pub candidates: Vec<Scored>,
    /// In [0, 1]. Above the library's threshold the best is accepted; below, it is asked.
    pub confidence: f64,
    pub episode: Option<EpisodeHit>,
    /// The answer came from the user's own earlier decision.
    pub remembered: bool,
    /// The key under which a decision about this name is remembered.
    pub pattern: String,
}

impl Identification {
    pub fn best(&self) -> Option<&Scored> {
        self.candidates.first()
    }
}

const KEEP: usize = 6;

/// Kinds a file can be. Episodes are reached through their series, never directly.
const FILM_KINDS: &[&str] = &["film", "anime_film"];
const SERIES_KINDS: &[&str] = &["series", "anime_series"];

/// How much the name says this is an episode: 1 for "S01E02", 0 for a film's name.
fn episode_likelihood(p: &Parsed, bare_number: bool) -> f64 {
    if bare_number {
        0.5
    } else if p.season.is_some() || !p.episodes.is_empty() || p.date.is_some() {
        1.0
    } else if !p.absolute_episodes.is_empty() {
        0.85
    } else {
        0.0
    }
}

/// A file title that names nothing: "movie.mkv", "VTS_01_1.vob", "05.mkv".
fn says_nothing(title: &str) -> bool {
    let c = comparable(title);
    c.is_empty()
        || matches!(c.as_str(), "movie" | "film" | "video" | "main" | "feature")
        || c.starts_with("vts ")
        || c.chars().all(|ch| ch.is_ascii_digit() || ch == ' ')
}

/// A title less a trailing country ("The Office (US)" -> "The Office"), if it has one.
fn without_country(title: &str) -> Option<String> {
    let c = comparable(title);
    let (rest, last) = c.rsplit_once(' ')?;
    matches!(last, "us" | "uk" | "au" | "ca" | "nz" | "ie").then(|| rest.to_string())
}

/// Word overlap (Dice) on comparable titles, 1 when equal.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (comparable(a), comparable(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let wa: Vec<&str> = a.split(' ').collect();
    let mut wb: Vec<&str> = b.split(' ').collect();
    let total = wa.len() + wb.len();
    let mut common = 0;
    for w in wa {
        if let Some(i) = wb.iter().position(|x| *x == w) {
            wb.swap_remove(i);
            common += 1;
        }
    }
    2.0 * common as f64 / total as f64
}

fn year_signal(want: Option<u16>, have: Option<i64>) -> f64 {
    match (want, have) {
        (Some(w), Some(h)) => match (i64::from(w) - h).abs() {
            0 => 1.0,
            1 => 0.6,
            _ => 0.0,
        },
        _ => 0.5,
    }
}

/// A film's runtime is one number; a series' is a typical episode, and finales and
/// pilots run long, so a series is judged more loosely.
fn runtime_signal(minutes: Option<f64>, runtime: Option<i64>, covers: usize, series: bool) -> f64 {
    let (Some(d), Some(r)) = (minutes, runtime) else {
        return 0.5;
    };
    if r <= 0 {
        return 0.5;
    }
    let per = d / covers.max(1) as f64;
    let off = (per - r as f64).abs() / r as f64;
    if series {
        return if off <= 0.25 {
            1.0
        } else if off <= 0.6 {
            0.6
        } else {
            0.3
        };
    }
    if off <= 0.08 {
        1.0
    } else if off <= 0.2 {
        0.7
    } else if off <= 0.4 {
        0.3
    } else {
        0.0
    }
}

fn popularity(votes: Option<i64>) -> f64 {
    let v = votes.unwrap_or(0).max(0) as f64;
    ((v + 1.0).log10() / 6.0).min(1.0)
}

/// The key a decision is remembered under: the comparable title, the year if any, and
/// whether the name is an episode's.
pub fn pattern(title: &str, year: Option<u16>, episode: bool) -> String {
    format!(
        "{}|{}|{}",
        comparable(title),
        year.map(|y| y.to_string()).unwrap_or_default(),
        if episode { "series" } else { "movie" }
    )
}

/// Confidence: how far the best is ahead of the next, times how plausible the best is
/// on its own. A runaway winner that barely fits is not confident; neither is a perfect
/// fit with a twin.
pub fn confidence(best: f64, next: f64) -> f64 {
    let lead = ((best - next) / DECISIVE_LEAD).clamp(0.0, 1.0);
    let plausible = ((best - IMPLAUSIBLE) / (PLAUSIBLE - IMPLAUSIBLE)).clamp(0.0, 1.0);
    lead * plausible
}

/// Identify one file.
pub async fn identify(db: &Db, e: &Evidence) -> Result<Identification, DbError> {
    let parsed = parse(&e.name);
    let folder = hint(&e.folders);
    let bare = says_nothing(&parsed.title);

    // What to search for: the name's title, or the folder's when the name says nothing.
    let from_folder = bare && !folder.title.is_empty();
    let (title, mut year) = if from_folder {
        (folder.title.clone(), folder.year)
    } else {
        (parsed.title.clone(), parsed.year)
    };
    // The folder's year for the same title ("Breaking Bad (2008)/Breaking.Bad.S01E01").
    if year.is_none() && !folder.title.is_empty() && comparable(&folder.title) == comparable(&title)
    {
        year = folder.year;
    }

    // Season and episodes, with the folder's season for "Season 1/Episode 3.mkv".
    let mut season = parsed.season;
    let mut episodes: Vec<u32> = parsed.episodes.clone();
    if season.is_none() && episodes.is_empty() && !parsed.absolute_episodes.is_empty() {
        if let Some(s) = folder.season {
            season = Some(s);
            episodes = parsed.absolute_episodes.clone();
        }
    }
    if season.is_none() && !episodes.is_empty() {
        season = folder.season;
    }

    // A bare number in a folder that names a film with its year ("Moonlight (2016)/01.mkv")
    // is neutral between film and episode; otherwise the name decides.
    let bare_number = bare && from_folder && folder.season.is_none();
    let likelihood = episode_likelihood(&parsed, bare_number);
    // "[Group] Title - 05": fansub naming, which is anime's.
    let anime_style = e.name.trim_start().starts_with('[');
    let is_episode = likelihood >= 0.5 && !bare_number || season.is_some();

    let key = pattern(&title, year, is_episode);
    let mut out = Identification {
        parsed: parsed.clone(),
        title: title.clone(),
        year,
        season,
        episodes: episodes.clone(),
        candidates: Vec::new(),
        confidence: 0.0,
        episode: None,
        remembered: false,
        pattern: key.clone(),
    };
    if title.is_empty() {
        return Ok(out);
    }

    let library = LibraryRepository::new(db);
    let search = SearchRepository::new(db);

    // Candidates: the exact title in any variant, then word and trigram matches.
    let mut ids: Vec<i64> = Vec::new();
    let mut add = |hits: Vec<sinephile_persistence::repositories::Hit>| {
        for h in hits {
            let wanted =
                FILM_KINDS.contains(&h.kind.as_str()) || SERIES_KINDS.contains(&h.kind.as_str());
            if wanted && !ids.contains(&h.media_item_id) {
                ids.push(h.media_item_id);
            }
        }
    };
    // "The Office (US)", "The Office UK": the catalogue's title has no country.
    let bare_title = without_country(&title);
    let mut exact = search.exact_title(&title, 20).await?;
    let mut words = search.keyword(&title, 15).await?;
    if let Some(bare) = &bare_title {
        exact.extend(search.exact_title(bare, 20).await?);
        words.extend(search.keyword(bare, 15).await?);
    }
    // The trigram search finds misspellings, and costs ~100x the others on the full
    // catalogue (`eval identify --timing`): only when nothing close was found.
    let close = exact
        .iter()
        .chain(&words)
        .any(|h| title_similarity(&title, &h.title) >= 0.8);
    add(exact);
    add(words);
    if !close {
        add(search.fuzzy(&title, 8).await?);
    }

    let remembered = library.recall(&key).await?;
    if let Some(id) = remembered {
        if !ids.contains(&id) {
            ids.insert(0, id);
        }
    }

    let facts = library.candidate_facts(&ids).await?;
    let covers = episodes.len().max(parsed.absolute_episodes.len()).max(1);
    let mut scored: Vec<Scored> = facts
        .iter()
        .map(|f| {
            score(
                f,
                &title,
                year,
                likelihood,
                anime_style,
                e.duration_minutes,
                covers,
            )
        })
        .collect();
    scored.sort_by(|a, b| b.score.total_cmp(&a.score));

    // Does the series have the episode the name claims? Checked for the leading few.
    let episodes_repo = EpisodeRepository::new(db);
    for s in scored.iter_mut().take(3) {
        if !SERIES_KINDS.contains(&s.kind.as_str()) {
            continue;
        }
        let found =
            find_episode(&episodes_repo, s.media_item_id, season, &episodes, &parsed).await?;
        s.signals.episode_found = found.map(|f| f.is_some());
        match found {
            Some(Some(_)) => s.score = (s.score + EPISODE_FOUND).min(1.0),
            // Claimed but not there: a different series, or a catalogue gap. Either way
            // less likely, not ruled out.
            Some(None) => s.score *= EPISODE_MISSING,
            None => {}
        }
    }
    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    scored.truncate(KEEP);

    if let Some(id) = remembered {
        // The user's own decision outranks every signal.
        if let Some(i) = scored.iter().position(|s| s.media_item_id == id) {
            let s = scored.remove(i);
            scored.insert(0, s);
            out.remembered = true;
        }
    }

    out.confidence = if out.remembered {
        1.0
    } else {
        match scored.as_slice() {
            [] => 0.0,
            [only] => confidence(only.score, 0.0),
            [a, b, ..] => confidence(a.score, b.score),
        }
    };

    if let Some(best) = scored.first() {
        if SERIES_KINDS.contains(&best.kind.as_str()) {
            if let Some(Some(id)) = find_episode(
                &episodes_repo,
                best.media_item_id,
                season,
                &episodes,
                &parsed,
            )
            .await?
            {
                out.episode = Some(EpisodeHit {
                    episode_id: id,
                    covers: covers as i64,
                });
            }
        }
    }
    out.candidates = scored;
    Ok(out)
}

fn score(
    f: &CandidateFacts,
    title: &str,
    year: Option<u16>,
    likelihood: f64,
    anime_style: bool,
    minutes: Option<f64>,
    covers: usize,
) -> Scored {
    let series = SERIES_KINDS.contains(&f.kind.as_str());
    // Fansub naming makes a live-action namesake less likely, not impossible.
    let anime_fit = if anime_style && !f.kind.starts_with("anime") {
        0.6
    } else {
        1.0
    };
    let bare = without_country(title);
    let t = f
        .titles
        .iter()
        .map(|known| {
            let with = title_similarity(title, known);
            // Without the country, a near-perfect match is still not quite the name.
            let without = bare
                .as_deref()
                .map_or(0.0, |b| 0.95 * title_similarity(b, known));
            with.max(without)
        })
        .fold(0.0, f64::max);
    let signals = Signals {
        title: t,
        year: year_signal(year, f.release_year),
        kind: anime_fit * if series { likelihood } else { 1.0 - likelihood },
        runtime: runtime_signal(minutes, f.runtime_minutes, covers, series),
        popularity: popularity(f.rating_votes),
        episode_found: None,
    };
    let score = W_TITLE * signals.title
        + W_YEAR * signals.year
        + W_KIND * signals.kind
        + W_RUNTIME * signals.runtime
        + W_POPULARITY * signals.popularity;
    Scored {
        media_item_id: f.media_item_id,
        kind: f.kind.clone(),
        title: f.primary_title.clone(),
        year: f.release_year,
        score,
        signals,
    }
}

/// The episode's id: `None` when there is nothing to look up, `Some(None)` when the
/// series lacks it.
async fn find_episode(
    repo: &EpisodeRepository<'_>,
    series_id: i64,
    season: Option<u32>,
    episodes: &[u32],
    parsed: &Parsed,
) -> Result<Option<Option<i64>>, DbError> {
    if let (Some(s), Some(e)) = (season, episodes.first()) {
        let hit = repo
            .resolve_seasonal(series_id, IdSource::Imdb, i64::from(s), i64::from(*e))
            .await?;
        return Ok(Some(hit.map(|h| h.episode_id)));
    }
    if let Some(n) = parsed.absolute_episodes.first() {
        let hit = repo
            .resolve_absolute(series_id, Some(IdSource::Anilist), i64::from(*n))
            .await?;
        // No free source publishes absolute numbers (ADR-0031): a miss is no evidence.
        return Ok(hit.map(|h| Some(h.episode_id)));
    }
    Ok(None)
}

/// The folder reading, exposed for callers that show it.
pub fn folder_hint(folders: &[String]) -> FolderHint {
    hint(folders)
}
