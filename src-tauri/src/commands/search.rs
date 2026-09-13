//! The search IPC surface. **Thin, as `SPEC.md` §7 requires** — every decision here
//! belongs to `crates/search-engine`, and this file converts types and nothing else.

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::{CatalogueRepository, MatchReason};

use crate::state::AppState;

/// One result, as the UI needs it.
///
/// A DTO rather than re-exporting the engine's `Hit`: the engine's type carries a BM25
/// score whose scale is meaningless outside the ranker, and a UI that received it would
/// eventually display it.
/// # Why the numbers are annotated
///
/// **Specta refuses to export `i64`, and it is right to.** A JavaScript number is a
/// double, so anything past 2^53 arrives silently wrong — the failure would be the wrong
/// film opening, with no error anywhere. The objection is answered rather than
/// suppressed: catalogue ids are SQLite rowids over 2.7 million titles, nine orders of
/// magnitude below that ceiling, and years are four digits.
///
/// If an id could ever exceed 2^53 these must become strings.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SearchHit {
    #[specta(type = i32)]
    pub id: i64,
    pub title: String,
    #[specta(type = Option<i32>)]
    pub year: Option<i64>,
    pub kind: String,
    /// Why this is on the page — the "why this matched" hint §15 asks for.
    pub why: Why,
}

/// The engine's [`MatchReason`], in the words a person would use.
///
/// Translated here rather than in the UI because the mapping is a product decision, not
/// a rendering one, and a second copy of it in TypeScript would drift.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// The query is this title.
    ExactTitle,
    /// The words matched.
    Keyword,
    /// Matched after typo tolerance.
    Fuzzy,
    /// Close in meaning, possibly sharing no words at all.
    Semantic,
    /// Both halves found it — the strongest signal the engine has.
    Both,
    /// Retrieved because it satisfies a filter the query asked for.
    Filter,
}

impl From<MatchReason> for Why {
    fn from(reason: MatchReason) -> Self {
        match reason {
            MatchReason::ExactTitle => Why::ExactTitle,
            MatchReason::Keyword => Why::Keyword,
            MatchReason::Fuzzy => Why::Fuzzy,
            MatchReason::Semantic => Why::Semantic,
            MatchReason::Both => Why::Both,
            MatchReason::Filter => Why::Filter,
        }
    }
}

/// What a search returned, **and what the catalogue is**.
///
/// # Why readiness travels with the results
///
/// "No results" means something entirely different at 3% ingested than at 100%, and a
/// screen that cannot tell the difference will confidently inform someone their film
/// does not exist. Rather than trust every caller to remember to ask, the answer carries
/// its own context — the UI cannot render the lie because it never holds the data alone.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    /// Titles searchable right now.
    #[specta(type = i32)]
    pub searchable_titles: i64,
    /// Is the catalogue still being built? If so, "nothing found" is not an answer.
    pub catalogue_partial: bool,
    /// Is the semantic half available? Without it, a query like "films about grief"
    /// matches words rather than meaning, and the UI should not imply otherwise.
    pub semantic: bool,
    /// Not ready yet — the database or the model is still loading. Distinct from
    /// "found nothing", and the two must never render the same.
    pub ready: bool,
}

impl SearchResponse {
    fn not_ready() -> Self {
        Self {
            hits: Vec::new(),
            searchable_titles: 0,
            catalogue_partial: true,
            semantic: false,
            ready: false,
        }
    }
}

/// Search the catalogue.
///
/// `limit` is `i32` rather than `i64`: it is clamped to 1..=100, and a function
/// parameter cannot carry the specta annotation a struct field can.
#[tauri::command]
#[specta::specta]
pub async fn search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i32>,
) -> Result<SearchResponse, String> {
    let (Some(db), Some(engine)) = (state.db(), state.engine()) else {
        // Still starting. Not an error, and emphatically not "no results".
        return Ok(SearchResponse::not_ready());
    };

    let limit = i64::from(limit.unwrap_or(20).clamp(1, 100));
    let mut engine = engine.lock().await;
    let hits = engine
        .search(&db, &query, limit)
        .await
        .map_err(|e| e.to_string())?;

    let readiness = CatalogueRepository::new(&db)
        .readiness()
        .await
        .map_err(|e| e.to_string())?;

    Ok(SearchResponse {
        hits: hits
            .into_iter()
            .map(|hit| SearchHit {
                id: hit.media_item_id,
                title: hit.title,
                year: hit.year,
                kind: hit.kind,
                why: hit.why.into(),
            })
            .collect(),
        searchable_titles: readiness.searchable_titles(),
        catalogue_partial: readiness.is_partial(),
        semantic: engine.is_hybrid(),
        ready: true,
    })
}
