//! Profiles (SPEC.md Phase 14). **Thin**: the rules (never the last profile, the default
//! handed on, PINs wrapped like keys) are `sinephile_persistence`'s; this checks the PIN
//! before a switch and tells the page.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use sinephile_persistence::repositories::{
    CredentialRepository, NewProfile, Profile, ProfileRepository,
};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileView {
    pub id: i32,
    pub name: String,
    pub is_default: bool,
    pub avatar_colour: Option<String>,
    pub is_kids: bool,
    pub has_pin: bool,
    /// The one watching now.
    pub active: bool,
}

fn view(p: Profile, active: i64) -> ProfileView {
    ProfileView {
        id: p.id as i32,
        active: p.id == active,
        name: p.name,
        is_default: p.is_default,
        avatar_colour: p.avatar_colour,
        is_kids: p.is_kids,
        has_pin: p.has_pin,
    }
}

fn friendly(e: sinephile_persistence::DbError) -> String {
    let text = e.to_string();
    if text.contains("UNIQUE") {
        "a profile already has that name".into()
    } else {
        text
    }
}

/// Every profile, the most recently used first.
#[tauri::command]
#[specta::specta]
pub async fn profiles(state: State<'_, AppState>) -> Result<Vec<ProfileView>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let active = state.profile().await?;
    let all = ProfileRepository::new(&db)
        .all()
        .await
        .map_err(|e| e.to_string())?;
    Ok(all.into_iter().map(|p| view(p, active)).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn create_profile(
    state: State<'_, AppState>,
    name: String,
    avatar_colour: Option<String>,
    is_kids: bool,
    pin: Option<String>,
) -> Result<ProfileView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    if name.trim().is_empty() {
        return Err("a profile needs a name".into());
    }
    let repo = ProfileRepository::new(&db);
    let id = repo
        .create_with(&NewProfile {
            name: &name,
            avatar_colour: avatar_colour.as_deref(),
            is_kids,
            kids_limit: None,
        })
        .await
        .map_err(friendly)?;
    if let Some(pin) = pin.filter(|p| !p.is_empty()) {
        if let Err(e) = CredentialRepository::new(&db).set_pin(id, &pin).await {
            // Not half-made: a profile whose PIN was refused is not left behind.
            let _ = repo.delete(id).await;
            return Err(e.to_string());
        }
    }
    let active = state.profile().await?;
    let p = repo
        .get(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("the profile was not recorded")?;
    Ok(view(p, active))
}

/// Name, avatar and kids setting. Only the profile watching may change itself, or any
/// profile when the one watching is not a kids profile.
#[tauri::command]
#[specta::specta]
pub async fn update_profile(
    state: State<'_, AppState>,
    id: i32,
    name: String,
    avatar_colour: Option<String>,
    is_kids: bool,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    if name.trim().is_empty() {
        return Err("a profile needs a name".into());
    }
    let repo = ProfileRepository::new(&db);
    let active = state.profile().await?;
    let me = repo.get(active).await.map_err(|e| e.to_string())?;
    if me.is_some_and(|m| m.is_kids) {
        return Err("a kids profile cannot change profiles".into());
    }
    repo.update(
        i64::from(id),
        &NewProfile {
            name: &name,
            avatar_colour: avatar_colour.as_deref(),
            is_kids,
            kids_limit: None,
        },
    )
    .await
    .map_err(friendly)
}

/// Delete a profile and its history, ratings, watchlist and settings. Never the last;
/// never from a kids profile. Deleting the one watching switches to the default.
#[tauri::command]
#[specta::specta]
pub async fn delete_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i32,
) -> Result<bool, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let repo = ProfileRepository::new(&db);
    let active = state.profile().await?;
    if repo
        .get(active)
        .await
        .map_err(|e| e.to_string())?
        .is_some_and(|m| m.is_kids)
    {
        return Err("a kids profile cannot change profiles".into());
    }
    let deleted = repo
        .delete(i64::from(id))
        .await
        .map_err(|e| e.to_string())?;
    if deleted && i64::from(id) == active {
        let next = repo
            .default_profile()
            .await
            .map_err(|e| e.to_string())?
            .ok_or("no profile left")?;
        state.set_profile(next.id);
        crate::commands::downloads::rebuild_manager().await;
        let _ = app.emit("profile-changed", next.id as i32);
    }
    Ok(deleted)
}

/// Switch who is watching. A PIN is checked first; a wrong one changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn switch_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i32,
    pin: Option<String>,
) -> Result<ProfileView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let id = i64::from(id);
    let repo = ProfileRepository::new(&db);
    let p = repo
        .get(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("that profile no longer exists")?;
    let opens = CredentialRepository::new(&db)
        .verify_pin(id, pin.as_deref().unwrap_or(""))
        .await
        .map_err(|e| e.to_string())?;
    if !opens {
        return Err("that PIN is not right".into());
    }
    state.set_profile(id);
    repo.touch(id).await.map_err(|e| e.to_string())?;
    // The user's qBittorrent is configured per profile.
    crate::commands::downloads::rebuild_manager().await;
    let _ = app.emit("profile-changed", id as i32);
    Ok(view(p, id))
}

/// Set or clear the watching profile's own PIN (`None` clears it).
#[tauri::command]
#[specta::specta]
pub async fn set_profile_pin(
    state: State<'_, AppState>,
    pin: Option<String>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let me = state.profile().await?;
    let creds = CredentialRepository::new(&db);
    match pin.filter(|p| !p.is_empty()) {
        Some(p) => creds.set_pin(me, &p).await.map_err(|e| e.to_string()),
        None => creds
            .clear_pin(me)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
    }
}

/// Whether the watching profile is a kids profile: catalogue surfaces filter by it.
pub(crate) async fn kids(state: &AppState) -> bool {
    let (Some(db), Ok(me)) = (state.db(), state.profile().await) else {
        return false;
    };
    ProfileRepository::new(&db)
        .get(me)
        .await
        .ok()
        .flatten()
        .is_some_and(|p| p.is_kids)
}
