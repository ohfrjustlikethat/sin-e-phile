//! Ratings for any title (ADR-0056): half stars, a show by its seasons, Continue watching's
//! removal, and the rate-a-batch screen. **Thin**: the rules are the repository's and
//! `sinephile_taste::rate`'s.

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::ProfileRepository;

use crate::commands::home::{home_cards, refresh_model, HomeCard};
use crate::state::AppState;

/// One rated season.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SeasonRating {
    pub season: i32,
    /// 1 to 10: half stars.
    pub halves: i32,
}

/// A show's ratings: each rated season, and their average ("your average").
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ShowRatings {
    pub seasons: Vec<SeasonRating>,
    /// In half stars, 1 to 10; `None` until a season is rated.
    pub average: Option<f64>,
}

/// Rate a film (or anything rated as a whole) in half stars, 1 to 10; `None` clears it.
#[tauri::command]
#[specta::specta]
pub async fn rate_title(
    state: State<'_, AppState>,
    id: i32,
    halves: Option<i32>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    ProfileRepository::new(&db)
        .rate(profile, i64::from(id), halves.map(i64::from))
        .await
        .map_err(|e| e.to_string())?;
    refresh_model(&state, profile);
    Ok(())
}

/// A title's own rating, in half stars.
#[tauri::command]
#[specta::specta]
pub async fn title_rating(state: State<'_, AppState>, id: i32) -> Result<Option<i32>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    Ok(ProfileRepository::new(&db)
        .rating(profile, i64::from(id))
        .await
        .map_err(|e| e.to_string())?
        .map(|h| h as i32))
}

/// Rate one season of a show in half stars, 1 to 10; `None` clears it.
#[tauri::command]
#[specta::specta]
pub async fn rate_season(
    state: State<'_, AppState>,
    series: i32,
    season: i32,
    halves: Option<i32>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    ProfileRepository::new(&db)
        .rate_season(
            profile,
            i64::from(series),
            i64::from(season),
            halves.map(i64::from),
        )
        .await
        .map_err(|e| e.to_string())?;
    refresh_model(&state, profile);
    Ok(())
}

/// A show's rated seasons and their average.
#[tauri::command]
#[specta::specta]
pub async fn show_ratings(state: State<'_, AppState>, series: i32) -> Result<ShowRatings, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    let seasons: Vec<SeasonRating> = ProfileRepository::new(&db)
        .season_ratings(profile, i64::from(series))
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(season, halves)| SeasonRating {
            season: season as i32,
            halves: halves as i32,
        })
        .collect();
    let average = (!seasons.is_empty())
        .then(|| seasons.iter().map(|s| f64::from(s.halves)).sum::<f64>() / seasons.len() as f64);
    Ok(ShowRatings { seasons, average })
}

/// How many titles this profile has rated: Home offers the rate-a-batch screen below ten.
#[tauri::command]
#[specta::specta]
pub async fn rated_count(state: State<'_, AppState>) -> Result<i32, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    Ok(ProfileRepository::new(&db)
        .rated_count(profile)
        .await
        .map_err(|e| e.to_string())? as i32)
}

/// The next films to rate one after another.
#[tauri::command]
#[specta::specta]
pub async fn to_rate(state: State<'_, AppState>, n: u32) -> Result<Vec<HomeCard>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    let kids = crate::commands::profiles::kids(&state).await;
    let ids = sinephile_taste::rate::batch(&db, profile, kids, n as usize)
        .await
        .map_err(|e| e.to_string())?;
    home_cards(&db, &ids).await
}

/// "Haven't seen it" on the rate-a-batch screen.
#[tauri::command]
#[specta::specta]
pub async fn skip_rating(state: State<'_, AppState>, id: i32) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    ProfileRepository::new(&db)
        .skip_rating(profile, i64::from(id))
        .await
        .map_err(|e| e.to_string())
}

/// Whether a film or a show is in this profile's Continue watching now (its page offers to
/// remove it).
#[tauri::command]
#[specta::specta]
pub async fn in_continue_watching(state: State<'_, AppState>, id: i32) -> Result<bool, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    Ok(sinephile_taste::continuing::continuing(&db, profile, 100)
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .any(|c| c.card == i64::from(id)))
}

/// Remove a film or a show from Continue watching: hidden, its place kept, back once it is
/// played again.
#[tauri::command]
#[specta::specta]
pub async fn remove_from_continue(state: State<'_, AppState>, id: i32) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profile = state.profile().await?;
    ProfileRepository::new(&db)
        .hide_from_continue(profile, i64::from(id))
        .await
        .map_err(|e| e.to_string())
}
