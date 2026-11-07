//! Home, first version, and first-run onboarding (SPEC.md Phase 14). **Thin**: the rails
//! are `sinephile_taste::home`'s; this fills in what a card shows.

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::{
    CredentialRepository, MediaRepository, ProfileRepository,
};

use crate::state::AppState;

/// Set once the first-run wizard is finished or skipped; app-wide, not per profile.
const ONBOARDED: &str = "onboarding.done";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct HomeCard {
    pub id: i32,
    pub title: String,
    pub year: Option<i32>,
    pub made_by: Option<String>,
    pub runtime_minutes: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct RailView {
    pub key: String,
    pub title: String,
    pub items: Vec<HomeCard>,
}

#[tauri::command]
#[specta::specta]
pub async fn home(state: State<'_, AppState>) -> Result<Vec<RailView>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let kids = crate::commands::profiles::kids(&state).await;
    let love = CredentialRepository::new(&db)
        .get(me, "taste.love")
        .await
        .map_err(|e| e.to_string())?;
    let rails = match state.engine() {
        Some(engine) => {
            let mut engine = engine.lock().await;
            sinephile_taste::home::rails(&db, engine.semantic_mut(), me, kids, love.as_deref())
                .await
        }
        None => sinephile_taste::home::rails(&db, None, me, kids, love.as_deref()).await,
    }
    .map_err(|e| e.to_string())?;

    let ids: Vec<i64> = rails.iter().flat_map(|r| r.items.iter().copied()).collect();
    let mut cards = sinephile_search_engine::card::cards(&db, &ids)
        .await
        .map_err(|e| e.to_string())?;
    let media = MediaRepository::new(&db);
    let mut out = Vec::new();
    for r in rails {
        let mut items = Vec::new();
        for id in r.items {
            let Some(m) = media.by_id(id).await.map_err(|e| e.to_string())? else {
                continue;
            };
            let card = cards.remove(&id).unwrap_or_default();
            items.push(HomeCard {
                id: id as i32,
                title: m.primary_title,
                year: m.release_year.map(|y| y as i32),
                made_by: card.made_by,
                runtime_minutes: m.runtime_minutes.map(|r| r as i32),
            });
        }
        out.push(RailView {
            key: r.key.into(),
            title: r.title.into(),
            items,
        });
    }
    Ok(out)
}

/// Whether the first-run wizard has been finished or skipped.
#[tauri::command]
#[specta::specta]
pub async fn onboarding_done(state: State<'_, AppState>) -> Result<bool, String> {
    let db = state.db().ok_or("the library is still opening")?;
    Ok(ProfileRepository::new(&db)
        .setting(ONBOARDED)
        .await
        .map_err(|e| e.to_string())?
        .is_some())
}

#[tauri::command]
#[specta::specta]
pub async fn finish_onboarding(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    ProfileRepository::new(&db)
        .set_setting(ONBOARDED, &sinephile_downloads::now_iso())
        .await
        .map_err(|e| e.to_string())
}
