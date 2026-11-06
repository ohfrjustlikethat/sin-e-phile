//! The detail page and Play (SPEC.md Phase 8). **Thin**: what plays and from where is
//! `crates/playback`; this gathers the pieces it needs and converts types.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use sinephile_persistence::repositories::profiles::PlaybackPosition;
use sinephile_persistence::repositories::{MediaRepository, ProfileRepository};
use sinephile_sources::backends::all;
use sinephile_sources::overview;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CreditView {
    pub name: String,
    pub role: String,
    pub character: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ItemDetail {
    #[specta(type = i32)]
    pub id: i64,
    pub title: String,
    #[specta(type = Option<i32>)]
    pub year: Option<i64>,
    pub kind: String,
    #[specta(type = Option<i32>)]
    pub runtime_minutes: Option<i64>,
    pub synopsis: Option<String>,
    pub credits: Vec<CreditView>,
}

#[tauri::command]
#[specta::specta]
pub async fn item_detail(
    state: State<'_, AppState>,
    id: i32,
) -> Result<Option<ItemDetail>, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let media = MediaRepository::new(&db);
    let Some(item) = media
        .by_id(i64::from(id))
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let credits = media
        .credits(item.id, 14)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(ItemDetail {
        id: item.id,
        title: item.primary_title,
        year: item.release_year,
        kind: item.kind.as_str().to_string(),
        runtime_minutes: item.runtime_minutes,
        synopsis: item.synopsis,
        credits: credits
            .into_iter()
            .map(|c| CreditView {
                name: c.name,
                role: c.role,
                character: c.character,
            })
            .collect(),
    }))
}

/// What Play will open, decided now (see `sinephile_playback::plan`).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct PlayPlanView {
    pub title: String,
    pub open: String,
    pub start: Option<f64>,
    /// The source's display name.
    pub from: String,
    pub offered: u32,
    /// Audio languages some honest copy carries, primary subtags, most common first.
    pub languages: Vec<String>,
    /// The film's own language, if known: an original soundtrack versus a dub.
    pub original_language: Option<String>,
    /// Qualities on offer, best first, as wire names (`"full_hd"`).
    pub qualities: Vec<String>,
    /// When the chosen language or quality is not to be had, why, in words.
    pub unmet: Option<String>,
    /// "Why this source", one line under Play.
    pub why: String,
    /// Every candidate, best first, with its score term by term: the expert panel.
    pub candidates: Vec<CandidateView>,
}

/// One candidate as the expert panel shows it (SPEC.md Phase 9 E5).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CandidateView {
    pub title: String,
    /// The source's display name.
    pub from: String,
    pub transport: String,
    /// `1080p`, or `unknown quality`.
    pub quality: String,
    /// `h264`, or `unknown`.
    pub codec: String,
    pub total: f64,
    pub sunk: Option<String>,
    pub unmet: Option<String>,
    pub local: bool,
    /// The one that opens.
    pub chosen: bool,
    pub terms: Vec<TermView>,
}

/// A score term exactly as `sinephile_selection::Term` holds it.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TermView {
    pub name: String,
    pub value: f64,
    pub weight: f64,
    pub why: String,
}

/// What the last plan would fall back to, for `player_open` (SPEC.md Phase 9, 9.4). Kept
/// here rather than sent through the page: the page only ever needs the address.
pub(crate) struct Planned {
    pub open: String,
    pub backend: String,
    pub fallbacks: Vec<source_protocol::SourceCandidate>,
}

pub(crate) static PLANNED: Mutex<Option<Planned>> = Mutex::new(None);

/// The sources as configured now: the same resolver the plan used.
pub(crate) async fn resolver(
    state: &AppState,
) -> Result<
    (
        sinephile_sources::Resolver,
        sinephile_sources::backends::Context,
    ),
    String,
> {
    let db = state.db().ok_or("the library is still opening")?;
    let (ctx, _) = overview::context_from(&db, state.http.clone(), state.untrusted_http.clone())
        .await
        .map_err(|e| e.to_string())?;
    let resolver = sinephile_sources::registry::resolver_from(&db, all(&ctx))
        .await
        .map_err(|e| e.to_string())?;
    Ok((resolver, ctx))
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_play(
    state: State<'_, AppState>,
    id: i32,
    language: Option<String>,
    quality: Option<String>,
) -> Result<PlayPlanView, String> {
    let db = state.db().ok_or("the library is still opening")?;
    let (resolver, ctx) = resolver(&state).await?;
    // The engine starts here if a swarm might be needed; a failure to start it only
    // means swarms are skipped, not that nothing can play.
    let rt = state.torrent().await.ok();
    let torrent = rt.as_ref().map(|r| (&r.engine, &r.server));
    let tier = state.effective_tier();
    let choice = sinephile_playback::Context {
        machine: sinephile_playback::machine(
            tier.allows(crate::tiers::Capability::HighResPlayback),
            state.hardware_profile().hardware_decode,
        ),
        audio_language: language,
        wanted: quality
            .as_deref()
            .and_then(sinephile_selection::resolution_from_code),
        ..Default::default()
    };
    let plan = sinephile_playback::plan_with(&db, &resolver, torrent, i64::from(id), choice)
        .await
        .map_err(|e| e.to_string())?;
    *PLANNED.lock().expect("planned") = Some(Planned {
        open: plan.open.clone(),
        backend: plan.backend.clone(),
        fallbacks: plan.fallbacks(),
    });
    Ok(PlayPlanView {
        from: overview::display_name(&plan.backend, &ctx.addons),
        title: plan.title,
        open: plan.open,
        start: plan.start,
        offered: plan.offered as u32,
        languages: plan.offers.languages,
        original_language: plan.original_language,
        qualities: plan
            .offers
            .qualities
            .into_iter()
            .map(|r| sinephile_selection::resolution_code(r).to_string())
            .collect(),
        why: plan
            .ranked
            .get(plan.chosen)
            .map(|(c, s)| sinephile_selection::why(c, s))
            .unwrap_or_default(),
        candidates: plan
            .ranked
            .iter()
            .enumerate()
            .map(|(i, (c, s))| CandidateView {
                title: c.title.clone(),
                from: overview::display_name(&c.provenance.backend, &ctx.addons),
                transport: format!("{:?}", c.transport).to_lowercase(),
                quality: sinephile_selection::resolution_label(c.quality.resolution).into(),
                codec: format!("{:?}", c.codec).to_lowercase(),
                total: s.total,
                sunk: s.sunk.clone(),
                unmet: s.unmet.clone(),
                local: s.local,
                chosen: i == plan.chosen,
                terms: s
                    .terms
                    .iter()
                    .map(|t| TermView {
                        name: t.name.into(),
                        value: t.value,
                        weight: t.weight,
                        why: t.why.clone(),
                    })
                    .collect(),
            })
            .collect(),
        unmet: plan.score.unmet,
    })
}

/// Remember where the viewer is, for resume. Called periodically and on leaving.
#[tauri::command]
#[specta::specta]
pub async fn save_position(
    state: State<'_, AppState>,
    id: i32,
    position: f64,
    duration: Option<f64>,
) -> Result<(), String> {
    let db = state.db().ok_or("the library is still opening")?;
    let profiles = ProfileRepository::new(&db);
    let profile = profiles.ensure_default().await.map_err(|e| e.to_string())?;
    profiles
        .save_position(
            profile,
            &PlaybackPosition {
                media_item_id: i64::from(id),
                position_seconds: position.max(0.0) as i64,
                duration_seconds: duration.map(|d| d as i64),
                audio_language: None,
                subtitle_language: None,
            },
        )
        .await
        .map_err(|e| e.to_string())
}
