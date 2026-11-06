//! The pause overlay's data (SPEC.md Phase 11). **Thin**: credits and filmographies are
//! `MediaRepository`'s, "more like this" is the search engine's.
//!
//! Fetched when playback starts, not when pause is pressed, so the overlay paints at once.
//! What is not here, deliberately: trivia, goofs and soundtrack have no free, legal
//! source (IMDb's datasets omit them; scraping is ruled out by §2.1), and Rotten Tomatoes
//! has no free API. Headshots need the user's own TMDB key (Phase 4); without one the
//! cast is typographic.

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::{EpisodeRepository, MediaRepository};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct PersonView {
    pub person_id: i32,
    pub name: String,
    /// For the cast: who they play. For the crew: what they did.
    pub part: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct RatingView {
    pub source: String,
    /// Out of ten.
    pub score: f64,
    pub votes: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TitleView {
    pub id: i32,
    pub title: String,
    pub year: Option<i32>,
    pub kind: String,
    /// Their part in it, in a filmography.
    pub part: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct PauseDetails {
    pub cast: Vec<PersonView>,
    pub crew: Vec<PersonView>,
    pub ratings: Vec<RatingView>,
    /// Empty without the semantic index (Tier 0 before it is downloaded).
    pub more_like_this: Vec<TitleView>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The roles that are crew, in the order the overlay names them.
const CREW: [&str; 6] = [
    "director",
    "creator",
    "writer",
    "composer",
    "cinematographer",
    "editor",
];

#[tauri::command]
#[specta::specta]
pub async fn pause_details(state: State<'_, AppState>, id: i32) -> Result<PauseDetails, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let media = MediaRepository::new(&db);
    let mut item_id = i64::from(id);
    // An episode's own credits are often a handful; its series has the cast.
    let mut credits = media.credits(item_id, 60).await.map_err(err)?;
    if let Some((series, _, _)) = EpisodeRepository::new(&db)
        .placement(item_id)
        .await
        .map_err(err)?
    {
        if credits.len() < 4 {
            credits = media.credits(series, 60).await.map_err(err)?;
        }
        item_id = series;
    }
    let cast = credits
        .iter()
        .filter(|c| c.role == "actor")
        .take(24)
        .map(|c| PersonView {
            person_id: c.person_id as i32,
            name: c.name.clone(),
            part: c.character.clone(),
        })
        .collect();
    let mut crew: Vec<PersonView> = Vec::new();
    for role in CREW {
        for c in credits.iter().filter(|c| c.role == role).take(3) {
            crew.push(PersonView {
                person_id: c.person_id as i32,
                name: c.name.clone(),
                part: Some(role.to_string()),
            });
        }
    }
    let item = media.by_id(item_id).await.map_err(err)?;
    let ratings = item
        .as_ref()
        .and_then(|i| i.rating.map(|r| (r, i.rating_votes)))
        .map(|(r, votes)| RatingView {
            source: "IMDb".into(),
            score: r as f64 / 10.0,
            votes: votes.map(|v| v.min(i64::from(i32::MAX)) as i32),
        })
        .into_iter()
        .collect();

    let more_like_this = match (
        state.engine(),
        sinephile_catalogue::embed::document_for(&db, item_id).await,
    ) {
        (Some(engine), Ok(Some(document))) => engine
            .lock()
            .await
            .similar(&db, &document, item_id, 8)
            .await
            .map_err(err)?
            .unwrap_or_default()
            .into_iter()
            .map(|h| TitleView {
                id: h.media_item_id as i32,
                title: h.title,
                year: h.year.map(|y| y as i32),
                kind: h.kind,
                part: None,
            })
            .collect(),
        _ => vec![],
    };
    Ok(PauseDetails {
        cast,
        crew,
        ratings,
        more_like_this,
    })
}

/// A cast or crew member's other work, from the overlay.
#[tauri::command]
#[specta::specta]
pub async fn filmography(
    state: State<'_, AppState>,
    person_id: i32,
) -> Result<Vec<TitleView>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    Ok(MediaRepository::new(&db)
        .filmography(i64::from(person_id), 40)
        .await
        .map_err(err)?
        .into_iter()
        .map(|a| TitleView {
            id: a.media_item_id as i32,
            title: a.title,
            year: a.year.map(|y| y as i32),
            kind: a.kind,
            part: a.character.or(Some(a.role)),
        })
        .collect())
}
