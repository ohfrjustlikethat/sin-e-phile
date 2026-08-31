use crate::state::AppState;
use crate::tiers::{Capability, HardwareProfile, Tier};
use tauri::State;

/// The detected hardware profile and the tier in force.
///
/// Drives the Settings screen (§8: shown "in plain language. This is a nice UI
/// moment, not an apology").
#[tauri::command]
#[specta::specta]
pub fn get_hardware_profile(state: State<'_, AppState>) -> HardwareProfile {
    state.hardware_profile()
}

/// Override the detected tier, or clear the override with `None`.
///
/// §8 requires a manual override. Someone whose GPU is misdetected, or who wants
/// to see the Tier 0 experience, should not have to fight the app.
#[tauri::command]
#[specta::specta]
pub fn set_tier_override(state: State<'_, AppState>, tier: Option<Tier>) -> HardwareProfile {
    state.set_tier_override(tier)
}

/// Whether a capability is available under the effective tier.
///
/// The ONLY way a feature may ask about hardware (§8).
#[tauri::command]
#[specta::specta]
pub fn has_capability(state: State<'_, AppState>, capability: Capability) -> bool {
    state.effective_tier().allows(capability)
}

/// Where portable data lives, for the Settings screen to show honestly.
#[tauri::command]
#[specta::specta]
pub fn get_data_dir() -> String {
    crate::logging::data_dir().to_string_lossy().into_owned()
}

/// Deliberately panic, to prove the crash handler works.
///
/// Phase 1 exit criterion: "a deliberately-triggered panic writes a crash log and
/// shows a graceful error screen." An exit criterion that cannot be exercised is
/// not evidence, so the trigger ships — in debug builds only.
#[tauri::command]
#[specta::specta]
pub fn debug_trigger_panic() {
    #[cfg(debug_assertions)]
    panic!("deliberate panic from debug_trigger_panic (Phase 1 exit criterion)");
}

/// The frontend has painted. Reveal the window and record cold start.
///
/// This is what makes Phase 1's "< 2 s to interactive" a real measurement rather
/// than a measurement of window creation.
#[tauri::command]
#[specta::specta]
pub fn frontend_ready(window: tauri::Window) -> u32 {
    let elapsed_ms = crate::PROCESS_START
        .get()
        .map(|t| t.elapsed().as_millis().min(u32::MAX as u128) as u32)
        .unwrap_or(0);

    tracing::info!(cold_start_ms = elapsed_ms, "frontend interactive");
    let _ = window.show();
    let _ = window.set_focus();
    elapsed_ms
}
