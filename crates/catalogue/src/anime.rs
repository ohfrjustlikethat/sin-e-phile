//! AniList → catalogue ingestion (Phase 4, subtask 4.4).
//!
//! The matcher in [`crate::matching`] decides *whether* an AniList title refers to a
//! catalogue entry. This module is what runs it over the whole anime catalogue and
//! writes the results — and, just as importantly, what **counts the refusals**.
//!
//! # Why the refusals are the deliverable
//!
//! Exit criterion E5 hand-checks fifty anime titles. A run that reported only
//! "matched 14,000" would tell nobody whether those matches are right, and would
//! hide the two failure modes that actually matter: a title the catalogue does not
//! have at all, and a title that several catalogue entries could equally be. So
//! every outcome is counted and a sample of each is kept by name, which is the list
//! a human then checks by hand.
//!
//! # Transactions and the network
//!
//! Each page is fetched inside the step's transaction, which looks alarming — a
//! network round trip with a database transaction open. It is safe here because
//! SQLite's `BEGIN` is deferred: the transaction takes no lock until its first
//! statement, and the fetch happens before any of them. The alternative, fetching
//! outside and writing inside, would put a page in memory with no checkpoint
//! covering it, which is precisely the guarantee [`crate::job`] exists to provide.

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use sinephile_metadata_api::{AniList, Media};

use crate::imdb::current_year;
use crate::job::{Batch, Job, JobError, SqliteTx};
use crate::matching::{
    match_title, normalise, split_season, title_forms, Candidate, MatchKind, NoMatch, TitleIndex,
};

/// AniList's maximum. Asking for more is silently truncated, which would look like
/// the catalogue ending early.
const PER_PAGE: i64 = 50;

/// How many titles to keep by name for hand-checking. E5 checks fifty; keeping four
/// times that leaves room to pick a representative sample rather than whichever
/// fifty happened to come first.
const SAMPLE_LIMIT: usize = 200;

/// Whether a matched entry actually wrote, or found the catalogue item already
/// spoken for by an earlier (more popular) AniList entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claim {
    Written,
    AlreadyClaimed,
}

/// What a run found. Every AniList entry lands in exactly one of the outcome counts.
#[derive(Debug, Default, Clone)]
pub struct Report {
    pub seen: i64,
    pub matched: i64,
    pub not_in_catalogue: i64,
    pub ambiguous: i64,
    pub year_conflict: i64,
    /// Matched, but the catalogue item was already claimed by an earlier AniList
    /// entry — almost always a later season of a series IMDb lists once. Not a
    /// failure: the mapping that survives is the popular one, which is the right one.
    pub already_claimed: i64,

    /// How the matches were made. A run that is mostly `SeasonAware` is a run to be
    /// suspicious of: season-stripping is the loosest rule here.
    pub exact_title_and_year: i64,
    pub exact_title_year_unknown: i64,
    pub season_aware: i64,

    /// Every entry that did not match, in full, for the hand-check.
    ///
    /// NOT a first-N sample. The sweep runs year-ascending, so the first two hundred
    /// unmatched entries are all obscure shorts from 1955-1965 — and E5 asks
    /// specifically for long-running shonen, split-cour seasons and films tied to
    /// series, none of which can appear in a list like that. A first-N sample of an
    /// ordered sweep is a sample of the order, not of the population.
    pub unmatched: Vec<Unmatched>,
    /// `(anilist title, the form that matched, catalogue id)`.
    pub matched_samples: Vec<(String, String, i64)>,
}

/// One AniList entry that found no catalogue home, with everything needed to check it
/// by hand or to measure why.
#[derive(Debug, Clone)]
pub struct Unmatched {
    pub anilist_id: i64,
    pub romaji: String,
    pub english: String,
    pub native: String,
    pub year: Option<i64>,
    pub format: String,
    pub reason: String,
}

impl Unmatched {
    fn tsv(&self) -> String {
        let year = self.year.map(|y| y.to_string()).unwrap_or_default();
        // Tabs and newlines inside a title would corrupt the file. Neither occurs in
        // AniList's data today, and stripping them costs nothing against the chance
        // that a single title silently shifts every column after it.
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.anilist_id,
            clean(&self.romaji),
            clean(&self.english),
            clean(&self.native),
            year,
            clean(&self.format),
            clean(&self.reason),
        )
    }

    pub const HEADER: &'static str = "anilist_id\tromaji\tenglish\tnative\tyear\tformat\treason";
}

impl Report {
    fn record(
        &mut self,
        media: &Media,
        outcome: &Result<crate::matching::Match, NoMatch>,
        claim: Option<Claim>,
    ) {
        self.seen += 1;
        let name = display_name(media);

        if claim == Some(Claim::AlreadyClaimed) {
            self.already_claimed += 1;
            self.unmatched
                .push(unmatched(media, "already claimed by an earlier entry"));
            return;
        }

        match outcome {
            Ok(m) => {
                self.matched += 1;
                match m.kind {
                    MatchKind::ExactTitleAndYear => self.exact_title_and_year += 1,
                    MatchKind::ExactTitleYearUnknown => self.exact_title_year_unknown += 1,
                    MatchKind::SeasonAware => self.season_aware += 1,
                }
                if self.matched_samples.len() < SAMPLE_LIMIT {
                    self.matched_samples
                        .push((name, m.matched_on.clone(), m.media_item_id));
                }
            }
            Err(reason) => {
                let why = match reason {
                    NoMatch::NotInCatalogue => {
                        self.not_in_catalogue += 1;
                        "not in catalogue".to_string()
                    }
                    NoMatch::Ambiguous { candidates } => {
                        self.ambiguous += 1;
                        format!("ambiguous ({candidates} candidates)")
                    }
                    NoMatch::YearConflict { catalogue, anilist } => {
                        self.year_conflict += 1;
                        format!("year conflict (catalogue {catalogue}, anilist {anilist})")
                    }
                };
                self.unmatched.push(unmatched(media, &why));
            }
        }
    }

    /// An evenly spread selection of the unmatched entries, for printing.
    ///
    /// Spread rather than truncated: the sweep is year-ascending, so the first `n`
    /// unmatched entries are all from the 1950s and tell you nothing about the
    /// catalogue as a whole.
    pub fn unmatched_spread(&self, n: usize) -> Vec<&Unmatched> {
        if self.unmatched.len() <= n || n == 0 {
            return self.unmatched.iter().collect();
        }
        let step = self.unmatched.len() as f64 / n as f64;
        (0..n)
            .map(|i| &self.unmatched[(i as f64 * step) as usize])
            .collect()
    }

    /// The share of AniList entries that found a catalogue home.
    ///
    /// `already_claimed` counts in the denominator and not the numerator, which makes
    /// this a deliberately pessimistic number: a later season IS resolved to the right
    /// series, it simply cannot hold the id. E5 reads the counts, not this.
    pub fn match_rate(&self) -> f64 {
        if self.seen == 0 {
            return 0.0;
        }
        self.matched as f64 / self.seen as f64
    }
}

fn unmatched(media: &Media, reason: &str) -> Unmatched {
    Unmatched {
        anilist_id: media.id,
        romaji: media.title.romaji.clone().unwrap_or_default(),
        english: media.title.english.clone().unwrap_or_default(),
        native: media.title.native.clone().unwrap_or_default(),
        year: media.season_year,
        format: media.format.clone().unwrap_or_default(),
        reason: reason.to_string(),
    }
}

fn display_name(media: &Media) -> String {
    media
        .title
        .romaji
        .as_deref()
        .or(media.title.english.as_deref())
        .or(media.title.native.as_deref())
        .unwrap_or("(untitled)")
        .to_string()
}

/// The first year worth sweeping. AniList's earliest entries are from the 1900s, but
/// anime as a broadcast catalogue starts in the 1960s; earlier years cost a request
/// each and return nothing. Cheap enough to start well before the real beginning.
const FIRST_SEASON_YEAR: i64 = 1940;

/// Where a run is up to: a year and a page within that year.
///
/// Serialised into the step cursor as `year:page`. A page number alone was enough
/// while the sweep was one flat list; it is not enough now, and a cursor that cannot
/// express the position is a resume that silently restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    year: i64,
    page: i64,
}

impl Cursor {
    fn parse(raw: Option<&str>) -> Self {
        let start = Cursor {
            year: FIRST_SEASON_YEAR,
            page: 1,
        };
        let Some(raw) = raw else { return start };
        // An unparseable cursor restarts the sweep rather than failing. Re-running is
        // idempotent — every write is an upsert and the first claim on an item stands
        // — so redoing work is safe where guessing at a position is not.
        match raw.split_once(':') {
            Some((year, page)) => match (year.parse(), page.parse()) {
                (Ok(year), Ok(page)) => Cursor { year, page },
                _ => start,
            },
            None => start,
        }
    }

    fn encode(self) -> String {
        format!("{}:{}", self.year, self.page)
    }
}

/// Page the anime catalogue, match each entry, and write what matched.
///
/// # Why this sweeps year by year
///
/// AniList refuses to paginate past 5,000 entries, so a flat popularity sweep reaches
/// the top 5,000 anime and then returns `400` forever — which is exactly how the first
/// full run ended. Partitioning by `seasonYear` keeps every individual sweep far below
/// the cap.
///
/// Years are walked ASCENDING and, within a year, entries come back id-ordered. Both
/// halves of that mean the same thing — EARLIER FIRST — and together they settle the
/// first-claim-wins rule completely: when two AniList entries resolve to one catalogue
/// item, the one that keeps the mapping is the earlier of the two. For a series listed
/// once by IMDb and per-season by AniList, that is season one.
///
/// Id order is also what makes the sweep complete rather than merely long; see
/// [`AniList::page`] for why a popularity-ordered offset sweep silently loses entries.
///
/// Entries with no `seasonYear` at all are not reached by this sweep. They are reached
/// by the unfiltered popularity pass (`last_year: None`), which is bounded by the same
/// 5,000 cap — so an undated entry outside the 5,000 most popular is not ingested. It
/// also could not be matched with any confidence, since the year is half the evidence.
///
/// `unmatched_to` is where every unmatched entry is appended as TSV. It is written
/// PER PAGE rather than at the end, so an interrupted sweep keeps what it found — the
/// same reason the checkpoint exists. E5's hand-check reads this file.
///
/// `max_pages` bounds a run, for tests and for a quick first pass. The client is taken
/// by `Arc` rather than by reference because the step's closure outlives this stack
/// frame as far as the type system can tell — see [`AniList::owned`].
pub async fn ingest(
    job: &mut Job<'_>,
    client: Arc<AniList<'static>>,
    max_pages: Option<i64>,
    unmatched_to: Option<&Path>,
) -> Result<Report, JobError> {
    let last_year = current_year() + 1;
    let report = Arc::new(Mutex::new(Report::default()));
    let sink = Arc::clone(&report);
    let fetched = Arc::new(Mutex::new(0i64));

    // Truncate and write the header once, here, rather than on first append: a run
    // that matched everything should leave an empty file, not last run's file.
    let log = match unmatched_to {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?;
            }
            let mut file = std::fs::File::create(path)
                .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?;
            writeln!(file, "{}", Unmatched::HEADER)
                .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?;
            Some(Arc::new(Mutex::new(file)))
        }
        None => None,
    };

    job.run_step("anilist.catalogue", move |tx, cursor| {
        let sink = Arc::clone(&sink);
        let client = Arc::clone(&client);
        let fetched = Arc::clone(&fetched);
        let log = log.clone();
        Box::pin(async move {
            let at = Cursor::parse(cursor.as_deref());
            if at.year > last_year {
                return Ok(Batch::finished(0));
            }
            {
                let mut fetched = fetched.lock().expect("page count lock");
                if max_pages.is_some_and(|max| *fetched >= max) {
                    return Ok(Batch::finished(0));
                }
                *fetched += 1;
            }

            let page = client
                .page(at.page, PER_PAGE, Some(at.year))
                .await
                .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?;

            let mut fresh = Vec::new();
            for media in &page.media {
                let outcome = resolve(tx, media).await?;
                let claim = match &outcome {
                    Ok(matched) => Some(write_match(tx, media, matched).await?),
                    Err(_) => None,
                };
                let mut report = sink.lock().expect("report lock");
                let before = report.unmatched.len();
                report.record(media, &outcome, claim);
                fresh.extend_from_slice(&report.unmatched[before..]);
            }

            if let Some(log) = &log {
                let mut file = log.lock().expect("unmatched log lock");
                for entry in &fresh {
                    writeln!(file, "{}", entry.tsv())
                        .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?;
                }
            }

            // A year with nothing left moves to the next one. The sweep ends only when
            // the years run out, never when a single year does — most years before the
            // 1960s are empty, and stopping at the first empty one would stop at 1940.
            let next = if page.page_info.has_next_page && !page.media.is_empty() {
                Cursor {
                    year: at.year,
                    page: at.page + 1,
                }
            } else {
                Cursor {
                    year: at.year + 1,
                    page: 1,
                }
            };

            let items = page.media.len() as i64;
            Ok(if next.year > last_year {
                Batch::finished(items)
            } else {
                Batch::more(next.encode(), items)
            })
        })
    })
    .await?;

    let report = report.lock().expect("report lock").clone();
    Ok(report)
}

/// Which catalogue entry, if any, this AniList title is.
async fn resolve(
    tx: &mut SqliteTx<'_>,
    media: &Media,
) -> Result<Result<crate::matching::Match, NoMatch>, JobError> {
    let forms = title_forms(
        media.title.romaji.as_deref(),
        media.title.english.as_deref(),
        media.title.native.as_deref(),
    );
    if forms.is_empty() {
        return Ok(Err(NoMatch::NotInCatalogue));
    }

    // Both the whole title and its season-stripped base, because the catalogue may
    // carry either: AniList lists "Kaguya-sama: Love is War Season 2" where IMDb has
    // one series called "Kaguya-sama: Love is War".
    let mut lookups: Vec<String> = Vec::with_capacity(forms.len() * 2);
    for form in &forms {
        for key in [normalise(form), split_season(form).0] {
            if !key.is_empty() && !lookups.contains(&key) {
                lookups.push(key);
            }
        }
    }

    let mut index = TitleIndex::new();
    for candidate in candidates(tx, &lookups).await? {
        index.insert(candidate);
    }

    // AniList's format is what makes "Naruto" — one series and two unrelated films —
    // answerable at all. It only ever breaks a tie; see `match_title`.
    Ok(match_title(
        &index,
        &forms,
        media.season_year,
        Some(media.media_kind()),
    ))
}

/// Catalogue rows whose normalised title is any of `forms`.
///
/// The transaction-bound twin of [`crate::normalise::candidates`]. It reads through
/// the step's transaction rather than the pool so that it sees the same snapshot the
/// writes in this batch are landing in.
async fn candidates(tx: &mut SqliteTx<'_>, forms: &[String]) -> Result<Vec<Candidate>, JobError> {
    if forms.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = vec!["?"; forms.len()].join(", ");
    let sql = format!(
        "SELECT DISTINCT t.media_item_id, t.title, m.release_year, m.kind
         FROM titles t
         JOIN media_items m ON m.id = t.media_item_id
         WHERE t.normalised IN ({placeholders})"
    );

    let mut query = sqlx::query_as::<_, (i64, String, Option<i64>, String)>(&sql);
    for form in forms {
        query = query.bind(form);
    }

    Ok(query
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?
        .into_iter()
        .map(|(media_item_id, title, year, kind)| Candidate {
            media_item_id,
            title,
            year,
            kind,
        })
        .collect())
}

/// Write everything a confirmed match tells us.
async fn write_match(
    tx: &mut SqliteTx<'_>,
    media: &Media,
    matched: &crate::matching::Match,
) -> Result<Claim, JobError> {
    let step = "anilist.catalogue";
    let media_item_id = matched.media_item_id;

    // FIRST MATCH WINS, and the reason is structural rather than cautious.
    //
    // AniList lists every season as its own entry; IMDb lists one series containing
    // them. So "Attack on Titan Season 2" season-strips to "attack on titan" and
    // matches the SAME catalogue item as season 1 — correctly. But the schema allows
    // one AniList id per item, so the second write would overwrite the first, and the
    // second set of titles would overwrite season 1's romaji with season 2's.
    // Silently, and the catalogue would end up believing the series is called
    // "Attack on Titan Season 2".
    //
    // The pages are sorted by popularity, so the entry that wins is the one a user is
    // most likely to have meant. Seasons are the episode table's problem (migration
    // 0003), not this module's.
    if let Some(existing) = claimed_by(tx, media_item_id).await? {
        if existing != media.id {
            return Ok(Claim::AlreadyClaimed);
        }
    }

    // The kind becomes anime_film or anime_series. This is the ONLY place in the
    // pipeline where that happens, and it happens because AniList says so — not
    // because a title looked Japanese or a genre said "Animation". IMDb cannot
    // distinguish anime from any other animation, so without this the anime-specific
    // half of the app has nothing to select on.
    //
    // sort_title becomes the romaji: SPEC.md §6.2's reason for that column is that
    // anime is alphabetised by romaji, not by native script or a localised title.
    sqlx::query(
        "UPDATE media_items
            SET kind = ?, sort_title = COALESCE(?, sort_title), updated_at = datetime('now')
          WHERE id = ?",
    )
    .bind(media.media_kind())
    .bind(media.title.romaji.as_deref())
    .bind(media_item_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| JobError::step(step, e.to_string()))?;

    // `confidence` records HOW the mapping was made, which the schema asks for and
    // which matters here more than anywhere else: these are TITLE matches, not id
    // matches. A season-aware match survived a looser rule than an exact one, and a
    // later phase resolving a conflict between two sources can only weigh that if we
    // wrote down which it was.
    let confidence = match matched.kind {
        MatchKind::ExactTitleAndYear => 1.0,
        MatchKind::ExactTitleYearUnknown => 0.8,
        MatchKind::SeasonAware => 0.6,
    };
    upsert_external_id(
        tx,
        media_item_id,
        "anilist",
        &media.id.to_string(),
        confidence,
    )
    .await?;
    if let Some(mal) = media.id_mal {
        // Free cross-mapping: AniList carries MAL's id, and nothing else gives us one
        // without a second dataset. Subtask 4.5 needs exactly this.
        upsert_external_id(tx, media_item_id, "mal", &mal.to_string(), confidence).await?;
    }

    // The three title forms, as ASSERTED FACTS. `akas` already guessed at romaji and
    // native by looking at the script of the text (see akas.rs); AniList knows.
    //
    // The uniqueness key is (item, variant, TITLE) — migration 0007 changed it from
    // (item, variant, language, region), and 0001's comment describing the old shape
    // is history rather than truth. So AniList's romaji does not displace the guessed
    // one unless the text is identical; where they differ, both rows survive. That is
    // the right outcome and not merely the tolerable one: two romanisations of one
    // title are two things a user might type, and search should find it either way.
    // The upsert therefore only backfills `normalised` on a row that already exists.
    for (title, variant, language) in [
        (media.title.romaji.as_deref(), "romaji", None),
        (media.title.native.as_deref(), "native", None),
        (media.title.english.as_deref(), "english", Some("en")),
    ] {
        let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO titles (media_item_id, title, variant, language, region, normalised)
             VALUES (?, ?, ?, ?, NULL, ?)
             ON CONFLICT (media_item_id, variant, title)
             DO UPDATE SET normalised = excluded.normalised",
        )
        .bind(media_item_id)
        .bind(title)
        .bind(variant)
        .bind(language)
        // Normalised here rather than left for the backfill: a NULL normalised form
        // is invisible to the matcher, so a title inserted now and normalised later
        // is a title nothing can match against in between.
        .bind(normalise(title))
        .execute(&mut **tx)
        .await
        .map_err(|e| JobError::step(step, e.to_string()))?;
    }

    Ok(Claim::Written)
}

/// Which AniList entry already owns this catalogue item, if any.
async fn claimed_by(tx: &mut SqliteTx<'_>, media_item_id: i64) -> Result<Option<i64>, JobError> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT external_id FROM external_ids WHERE media_item_id = ? AND source = 'anilist'",
    )
    .bind(media_item_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?;
    Ok(existing.and_then(|id| id.parse().ok()))
}

async fn upsert_external_id(
    tx: &mut SqliteTx<'_>,
    media_item_id: i64,
    source: &str,
    external_id: &str,
    confidence: f64,
) -> Result<(), JobError> {
    // PRIMARY KEY (media_item_id, source) means one AniList id per item, and a UNIQUE
    // index on (source, external_id) means one item per AniList id. The second is the
    // one that bites: two AniList entries matching the same catalogue item — a series
    // and its recap special, say — collide, and the collision is the schema catching
    // a bad match rather than a bug to route around.
    sqlx::query(
        "INSERT INTO external_ids (media_item_id, source, external_id, confidence)
         VALUES (?, ?, ?, ?)
         ON CONFLICT (media_item_id, source)
         DO UPDATE SET external_id = excluded.external_id, confidence = excluded.confidence",
    )
    .bind(media_item_id)
    .bind(source)
    .bind(external_id)
    .bind(confidence)
    .execute(&mut **tx)
    .await
    .map_err(|e| JobError::step("anilist.catalogue", e.to_string()))?;
    Ok(())
}
