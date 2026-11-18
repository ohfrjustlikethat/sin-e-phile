//! Meaning for what is new (ADR-0054 step 2): each launch, the titles the catalogue gained
//! after the published meaning file get a synopsis and a vector in the companion graph
//! beside the published one, so search by meaning and the recommender reach them the
//! launch they arrive.
//!
//! # In order
//!
//! 1. **Who waits**: core titles past the published file's last id with no vector yet, most
//!    voted first, at most [`PER_LAUNCH`] ([`MeaningRepository::awaiting`]). A title asked
//!    about in the last [`RETRY_DAYS`] that nobody could describe waits for the next turn.
//! 2. **A synopsis**, from the first source that has one long enough to index
//!    ([`MIN_SYNOPSIS`], the published file's own rule): TMDB's overview with the
//!    profile's key, AniList's description for anime (no key), else the Wikipedia lead
//!    found through Wikidata (ADR-0033's two steps, asked for these titles only). A shorter
//!    text is still kept as the title's synopsis when it has none, for its page, but is not
//!    embedded.
//! 3. **A vector**: the document the published file's titles were embedded from
//!    ([`crate::embed::document_for`]), embedded by the model the app carries, into the
//!    companion graph, which is saved every [`SAVE_EVERY`] titles and at the end.
//!
//! Every tier: ADR-0015 keeps a weak machine from embedding the catalogue, hundreds of
//! thousands of documents; a launch's new titles are tens to a few thousand.

use std::collections::HashMap;

use sinephile_metadata_api::anilist::AniList;
use sinephile_metadata_api::tmdb::Tmdb;
use sinephile_metadata_api::wikipedia::{Wikipedia, EXTRACTS_PER_REQUEST, MAPPINGS_PER_REQUEST};
use sinephile_persistence::repositories::{Awaiting, MeaningRepository, MIN_SYNOPSIS};
use sinephile_persistence::Db;
use sinephile_vector_index::Companion;

use crate::embed::DocumentEmbedder;
use crate::job::JobError;

/// Titles one launch describes and embeds, the rest the next (ADR-0054).
pub const PER_LAUNCH: usize = 2_000;

/// How long a title nobody could describe waits before it is asked about again: a new
/// film's article is often written in the weeks after it comes out.
pub const RETRY_DAYS: i64 = 7;

/// Vectors between saves of the companion graph, so a launch closed halfway keeps most of
/// what it made.
pub const SAVE_EVERY: usize = 200;

/// Where a synopsis can come from, each optional: TMDB needs the profile's key, and a
/// machine offline has none of them (what is described already is still embedded).
#[derive(Default)]
pub struct Sources<'a> {
    pub tmdb: Option<&'a Tmdb>,
    pub anilist: Option<&'a AniList<'a>>,
    pub wikipedia: Option<&'a Wikipedia<'a>>,
}

/// What a launch did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    /// Titles that had their turn.
    pub waiting: usize,
    /// Of those, asked for a synopsis.
    pub asked: usize,
    /// Described this launch, by source.
    pub tmdb: usize,
    pub anilist: usize,
    pub wikipedia: usize,
    /// Given a vector.
    pub embedded: usize,
}

impl Report {
    pub fn described(&self) -> usize {
        self.tmdb + self.anilist + self.wikipedia
    }
}

fn step(e: impl std::fmt::Display) -> JobError {
    JobError::step("meaning", e.to_string())
}

/// Long enough to index (the published file's rule).
fn indexable(text: &str) -> bool {
    text.trim().chars().count() >= MIN_SYNOPSIS
}

/// The synopses found, each title's first long enough to index, else its first at all.
#[derive(Default)]
struct Texts {
    long: HashMap<i64, (String, &'static str)>,
    short: HashMap<i64, (String, &'static str)>,
}

impl Texts {
    fn keep(&mut self, id: i64, text: String, source: &'static str) {
        if indexable(&text) {
            self.long.entry(id).or_insert((text, source));
        } else if !text.trim().is_empty() {
            self.short.entry(id).or_insert((text, source));
        }
    }
}

/// CPU work on the app's runtime: on a multi-threaded runtime the worker hands its other
/// tasks to the rest before it blocks; anywhere else (a test's single thread) it just runs.
fn heavy<T>(work: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current().map(|h| h.runtime_flavor()) {
        Ok(tokio::runtime::RuntimeFlavor::MultiThread) => tokio::task::block_in_place(work),
        _ => work(),
    }
}

/// Bring the meaning of what is new up to date. `after` is the published file's last
/// catalogue id; `companion` the graph beside it, opened for writing.
pub async fn keep_current(
    db: &Db,
    embedder: &mut (dyn DocumentEmbedder + Send),
    companion: &mut Companion,
    after: i64,
    sources: &Sources<'_>,
    limit: usize,
) -> Result<Report, JobError> {
    let repo = MeaningRepository::new(db);
    // A companion that is gone (deleted, or a new model's) holds none of what the
    // database says was embedded: those titles are made again.
    if companion.is_empty() {
        repo.forget_embedded().await.map_err(step)?;
    }
    let waiting = repo
        .awaiting(after, RETRY_DAYS, limit as i64)
        .await
        .map_err(step)?;
    let mut report = Report {
        waiting: waiting.len(),
        ..Default::default()
    };

    // 2. A synopsis for each that has none, from the first source with one to index.
    let undescribed: Vec<&Awaiting> = waiting.iter().filter(|w| !w.described).collect();
    let mut texts = Texts::default();
    if let Some(tmdb) = sources.tmdb {
        for w in &undescribed {
            let Some(imdb) = w.imdb_id.as_deref() else {
                continue;
            };
            match tmdb.overview(imdb).await {
                Ok(Some(text)) => texts.keep(w.id, text, "tmdb"),
                Ok(None) => {}
                // A refused key or an unreachable TMDB leaves the other sources to it.
                Err(error) => {
                    tracing::info!(%error, "meaning: TMDB stopped answering");
                    break;
                }
            }
        }
    }
    if let Some(anilist) = sources.anilist {
        let ids: Vec<(i64, i64)> = undescribed
            .iter()
            .filter(|w| !texts.long.contains_key(&w.id))
            .filter_map(|w| Some((w.anilist_id?, w.id)))
            .collect();
        let by_anilist: HashMap<i64, i64> = ids.iter().copied().collect();
        for group in ids.chunks(50) {
            let asked: Vec<i64> = group.iter().map(|(a, _)| *a).collect();
            match anilist.descriptions(&asked).await {
                Ok(described) => {
                    for (anilist_id, text) in described {
                        if let Some(&id) = by_anilist.get(&anilist_id) {
                            texts.keep(id, text, "anilist");
                        }
                    }
                }
                Err(error) => {
                    tracing::info!(%error, "meaning: AniList stopped answering");
                    break;
                }
            }
        }
    }
    if let Some(wiki) = sources.wikipedia {
        let by_imdb: HashMap<&str, i64> = undescribed
            .iter()
            .filter(|w| !texts.long.contains_key(&w.id))
            .filter_map(|w| Some((w.imdb_id.as_deref()?, w.id)))
            .collect();
        let imdb_ids: Vec<String> = by_imdb.keys().map(|s| s.to_string()).collect();
        let mut articles: Vec<(String, i64)> = Vec::new();
        let mut reached = true;
        for group in imdb_ids.chunks(MAPPINGS_PER_REQUEST) {
            match wiki.mappings_for(group).await {
                Ok(mappings) => articles.extend(
                    mappings
                        .into_iter()
                        .filter_map(|m| Some((m.article, *by_imdb.get(m.imdb_id.as_str())?))),
                ),
                Err(error) => {
                    tracing::info!(%error, "meaning: Wikidata stopped answering");
                    reached = false;
                    break;
                }
            }
        }
        for group in articles
            .chunks(EXTRACTS_PER_REQUEST)
            .take_while(|_| reached)
        {
            let titles: Vec<String> = group.iter().map(|(a, _)| a.clone()).collect();
            match wiki.extracts(&titles).await {
                Ok(extracts) => {
                    for (article, id) in group {
                        // Paired on the title asked for: a moved article answers under
                        // its new name (`Extract::answers`).
                        if let Some(e) = extracts
                            .iter()
                            .find(|e| e.answers().eq_ignore_ascii_case(article))
                        {
                            texts.keep(*id, e.text.clone(), "wikipedia");
                        }
                    }
                }
                Err(error) => {
                    tracing::info!(%error, "meaning: Wikipedia stopped answering");
                    break;
                }
            }
        }
    }
    for (id, (text, source)) in &texts.long {
        repo.described(*id, text, source).await.map_err(step)?;
        match *source {
            "tmdb" => report.tmdb += 1,
            "anilist" => report.anilist += 1,
            _ => report.wikipedia += 1,
        }
    }
    // A short text is better than none on the title's page, but it is not embedded, and
    // the title is asked about again in a week.
    for (id, (text, source)) in &texts.short {
        if !texts.long.contains_key(id) {
            repo.described(*id, text, source).await.map_err(step)?;
        }
    }
    let asked: Vec<i64> = undescribed.iter().map(|w| w.id).collect();
    if sources.tmdb.is_some() || sources.anilist.is_some() || sources.wikipedia.is_some() {
        repo.asked(&asked).await.map_err(step)?;
        report.asked = asked.len();
    }

    // 3. A vector for each that can be indexed now.
    let ready: Vec<i64> = waiting
        .iter()
        .filter(|w| w.described || texts.long.contains_key(&w.id))
        .map(|w| w.id)
        .collect();
    let mut since_save: Vec<i64> = Vec::new();
    for id in ready {
        let Some(document) = crate::embed::document_for(db, id).await? else {
            continue;
        };
        let vector = heavy(|| embedder.embed(&document))?;
        let quantised = sinephile_embedding::quantise(&vector);
        companion.insert(id, &quantised.values).map_err(step)?;
        since_save.push(id);
        report.embedded += 1;
        if since_save.len() >= SAVE_EVERY {
            companion.save().map_err(step)?;
            repo.embedded(&since_save).await.map_err(step)?;
            since_save.clear();
        }
        tokio::task::yield_now().await;
    }
    if !since_save.is_empty() {
        companion.save().map_err(step)?;
        repo.embedded(&since_save).await.map_err(step)?;
    }
    Ok(report)
}
