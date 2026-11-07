//! Taste input (SPEC.md Phase 14). **Thin**: the statements and the seed are
//! `sinephile_taste`'s; this stores them for the profile watching and shows what the seed
//! points at.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::{CredentialRepository, ProfileRepository};
use sinephile_taste::Statements;

use crate::state::AppState;

const STATEMENTS: &str = "taste.statements";
const LOVE: &str = "taste.love";

/// Each -1, 0 or 1; `None` when not answered (see `sinephile_taste::Statements`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct StatementsView {
    pub pace: Option<i32>,
    pub ambiguity: Option<i32>,
    pub era: Option<i32>,
    pub subtitles: Option<i32>,
    pub runtime: Option<i32>,
    pub form: Option<i32>,
}

impl From<Statements> for StatementsView {
    fn from(s: Statements) -> Self {
        let v = |x: Option<i8>| x.map(i32::from);
        Self {
            pace: v(s.pace),
            ambiguity: v(s.ambiguity),
            era: v(s.era),
            subtitles: v(s.subtitles),
            runtime: v(s.runtime),
            form: v(s.form),
        }
    }
}

impl From<StatementsView> for Statements {
    fn from(s: StatementsView) -> Self {
        let v = |x: Option<i32>| {
            x.and_then(|n| i8::try_from(n).ok())
                .filter(|n| (-1..=1).contains(n))
        };
        Self {
            pace: v(s.pace),
            ambiguity: v(s.ambiguity),
            era: v(s.era),
            subtitles: v(s.subtitles),
            runtime: v(s.runtime),
            form: v(s.form),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TasteInputView {
    pub statements: StatementsView,
    pub love: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn taste_input(state: State<'_, AppState>) -> Result<TasteInputView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let creds = CredentialRepository::new(&db);
    let raw = creds.get(me, STATEMENTS).await.map_err(|e| e.to_string())?;
    let love = creds.get(me, LOVE).await.map_err(|e| e.to_string())?;
    Ok(TasteInputView {
        statements: Statements::parse(raw.as_deref()).into(),
        love: love.filter(|l| !l.is_empty()),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn set_taste_input(
    state: State<'_, AppState>,
    statements: StatementsView,
    love: Option<String>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let creds = CredentialRepository::new(&db);
    let s: Statements = statements.into();
    creds
        .set(me, STATEMENTS, &s.to_json())
        .await
        .map_err(|e| e.to_string())?;
    let love = love.unwrap_or_default();
    let love: String = love.trim().chars().take(2_000).collect();
    creds.set(me, LOVE, &love).await.map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TastePick {
    pub id: i32,
    pub title: String,
    pub year: Option<i32>,
    pub made_by: Option<String>,
}

/// The titles nearest the profile's taste so far (what it liked, and what it said it
/// loves), leaving out what it has seen. Empty without the vector half.
#[tauri::command]
#[specta::specta]
pub async fn taste_preview(
    state: State<'_, AppState>,
    limit: Option<i32>,
) -> Result<Vec<TastePick>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let Some(engine) = state.engine() else {
        return Ok(Vec::new());
    };
    let me = state.profile().await?;
    let liked = ProfileRepository::new(&db)
        .liked(me)
        .await
        .map_err(|e| e.to_string())?;
    let love = CredentialRepository::new(&db)
        .get(me, LOVE)
        .await
        .map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(12).clamp(1, 60) as usize;
    let seen: HashSet<i64> = liked.iter().map(|(id, _)| *id).collect();
    let ids = {
        let mut engine = engine.lock().await;
        let Some(semantic) = engine.semantic_mut() else {
            return Ok(Vec::new());
        };
        match sinephile_taste::seed(semantic, &liked, love.as_deref()) {
            Some(seed) => sinephile_taste::nearest(semantic, &seed, limit * 3, &seen),
            None => return Ok(Vec::new()),
        }
    };
    // A kids profile sees only what suits children.
    let ids = if crate::commands::profiles::kids(&state).await {
        let ok = sinephile_persistence::repositories::MediaRepository::new(&db)
            .suitable_for_children(&ids)
            .await
            .map_err(|e| e.to_string())?;
        ids.into_iter().filter(|id| ok.contains(id)).collect()
    } else {
        ids
    };
    let ids: Vec<i64> = ids.into_iter().take(limit).collect();
    let media = sinephile_persistence::repositories::MediaRepository::new(&db);
    let mut cards = sinephile_search_engine::card::cards(&db, &ids)
        .await
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for id in ids {
        if let Some(m) = media.by_id(id).await.map_err(|e| e.to_string())? {
            out.push(TastePick {
                id: id as i32,
                title: m.primary_title,
                year: m.release_year.map(|y| y as i32),
                made_by: cards.remove(&id).and_then(|c| c.made_by),
            });
        }
    }
    Ok(out)
}
