//! sin-e-phile — Windows desktop media engine.
//!
//! See `SPEC.md`. This crate is the Rust core; `src/` is the React frontend and
//! the two meet at a **generated** IPC boundary (subtask 1.7).

pub mod commands;
pub mod logging;
pub mod persistence;
pub mod state;
pub mod tiers;

use state::AppState;
use std::sync::OnceLock;
use std::time::Instant;

/// Process start, for an honest cold-start measurement.
///
/// Phase 1's exit criterion is "launches to INTERACTIVE in < 2 s". Timing until
/// the window handle exists would measure window creation, which is a different
/// and flattering number. This is set as early as `run()` begins and read when
/// the frontend reports it has painted.
pub static PROCESS_START: OnceLock<Instant> = OnceLock::new();
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

/// The IPC surface, in one place.
///
/// SPEC.md Phase 1 requires IPC types to be **generated, not hand-written**, with
/// the exit criterion that "changing a Rust command signature breaks the
/// TypeScript build". That property comes from this list: `tauri-specta` derives
/// the TypeScript from these exact signatures, so a changed argument or return
/// type regenerates `src/lib/ipc.ts`, and any call site that no longer matches
/// fails `tsc`.
///
/// Hand-written bindings would drift silently, and the drift would only show up
/// as a runtime error in front of a user.
fn ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::system::get_hardware_profile,
            commands::system::open_sign_up,
            commands::system::set_tier_override,
            commands::system::has_capability,
            commands::system::get_data_dir,
            commands::system::debug_trigger_panic,
            commands::system::frontend_ready,
            commands::search::search,
            commands::assets::optional_assets,
            commands::assets::download_optional_assets,
            commands::sources::sources_overview,
            commands::sources::sources_set_enabled,
            commands::sources::sources_reorder,
            commands::sources::addon_preview,
            commands::sources::addon_install,
            commands::sources::addon_remove,
            commands::sources::catalogue_open,
            commands::sources::links_list,
            commands::sources::links_import,
            commands::sources::links_add,
            commands::sources::links_remove,
            commands::sources::search_backfill,
            commands::torrent::torrent_settings,
            commands::torrent::set_torrent_settings,
            commands::torrent::torrents,
            commands::torrent::torrent_pause,
            commands::torrent::torrent_resume,
            commands::torrent::torrent_remove,
            commands::player::player_open,
            commands::player::player_layout,
            commands::player::player_action,
            commands::player::player_pause_still,
            commands::player::player_resume_video,
            commands::player::player_close,
            commands::player::player_state,
            commands::player::dev_autoplay,
            commands::player::dev_log,
            commands::player::dev_probe_script,
            commands::player::dev_player_prop,
            commands::playback::item_detail,
            commands::playback::prepare_play,
            commands::playback::save_position,
            commands::playback::rate_title,
            commands::playback::title_rating,
            commands::subtitles::subtitle_settings,
            commands::subtitles::set_subtitle_settings,
            commands::subtitles::set_opensubtitles_key,
            commands::subtitles::clear_opensubtitles_key,
            commands::subtitles::subtitle_nudge,
            commands::subtitles::subtitle_offers,
            commands::subtitles::subtitle_choose,
            commands::subtitles::subtitle_load,
            commands::subtitles::subtitle_realign,
            commands::series::series_detail,
            commands::series::season_episodes,
            commands::series::episode_neighbours,
            commands::overlay::pause_details,
            commands::overlay::filmography,
            commands::player::player_chapters,
            commands::player::player_stats,
            commands::thumbs::scrub_thumbs,
            commands::thumbs::scrub_sheet,
            commands::library::library_folders,
            commands::library::add_library_folder,
            commands::library::remove_library_folder,
            commands::library::rescan_library_folder,
            commands::library::library_review_count,
            commands::library::pick_library_folder,
            commands::library::library_review,
            commands::library::resolve_review,
            commands::library::not_a_title,
            commands::downloads::downloads,
            commands::downloads::download_pause,
            commands::downloads::download_resume,
            commands::downloads::download_cancel,
            commands::downloads::downloads_folder,
            commands::downloads::download_advice,
            commands::downloads::download_title,
            commands::downloads::download_priority,
            commands::downloads::download_limit,
            commands::downloads::download_tonight,
            commands::downloads::downloads_settings,
            commands::downloads::set_downloads_settings,
            commands::downloads::set_qbittorrent,
            commands::downloads::qbittorrent_check,
            commands::downloads::downloads_storage,
            commands::downloads::download_clear,
            commands::profiles::profiles,
            commands::profiles::create_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::switch_profile,
            commands::profiles::set_profile_pin,
            commands::imports::pick_import_file,
            commands::imports::import_history,
            commands::taste::taste_input,
            commands::taste::set_taste_input,
            commands::taste::taste_preview,
            commands::taste::taste_round,
            commands::catalogue::catalogue_status,
            commands::catalogue::refresh_catalogue,
            commands::catalogue::tmdb_status,
            commands::catalogue::set_tmdb_key,
            commands::catalogue::clear_tmdb_key,
            commands::taste::taste_profile,
            commands::taste::correct_taste,
            commands::home::home,
            commands::playback::end_viewing,
            commands::playback::record_signal,
            commands::home::onboarding_done,
            commands::home::finish_onboarding,
        ])
        // `AssetProgress` travels as an EVENT payload, so no command signature mentions it
        // and specta would not export it. Registered explicitly rather than hand-written in
        // TypeScript: a hand-written payload type is exactly the silent drift the generated
        // boundary exists to prevent.
        .typ::<commands::assets::AssetProgress>()
        .typ::<commands::torrent::TorrentProgress>()
        .typ::<commands::player::PlayerStateView>()
        .typ::<commands::library::LibraryScanEvent>()
}

pub fn run() {
    let _ = PROCESS_START.set(Instant::now());

    // Held for the process lifetime: dropping it stops the background log writer
    // and the final lines would be lost.
    let _log_guard = logging::init();

    let builder = ipc_builder();

    // Regenerate the TypeScript bindings on every debug run, so they cannot be
    // stale relative to the Rust. Release builds never write into the source tree.
    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default()
                .header("// GENERATED by tauri-specta. Do not edit.\n// Regenerated on every debug run of the Rust backend (see lib.rs).\n"),
            // Anchored to this crate, not the working directory: run from anywhere else,
            // a relative path wrote a stray copy and left the real bindings stale.
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc.ts"),
        )
        .expect("failed to export TypeScript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            app.manage(AppState::new());
            tauri::async_runtime::spawn(commands::torrent::progress_loop(app.handle().clone()));
            tauri::async_runtime::spawn(commands::player::state_loop(app.handle().clone()));
            tauri::async_runtime::spawn(commands::library::library_loop(app.handle().clone()));
            tauri::async_runtime::spawn(commands::downloads::downloads_loop(app.handle().clone()));
            tauri::async_runtime::spawn(commands::home::warm_models(app.handle().clone()));

            // THE DATABASE OPENS OFF THE STARTUP PATH, and the catalogue refreshes
            // itself behind it (D37, ADR-0030). Both on a background task, because
            // SPEC.md 2.3 budgets cold start and neither of these is worth a
            // millisecond of it: the window is what the user is waiting for.
            //
            // The refresh is automatic and silent by the author's decision. A prompt
            // would put a question about a dataset in front of someone who opened the
            // app to watch something. When IMDb has not republished — which is most
            // launches — the whole thing costs one HEAD request.
            let db_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let dir = match sinephile_persistence::paths::data_dir(
                    sinephile_persistence::DataLocation::Development,
                ) {
                    Ok(dir) => dir,
                    Err(error) => {
                        tracing::error!(%error, "no data directory; the catalogue is unavailable");
                        return;
                    }
                };

                let db = match sinephile_persistence::Db::open_in(&dir).await {
                    Ok(db) => std::sync::Arc::new(db),
                    Err(error) => {
                        // The app keeps running. Search will report that the catalogue
                        // is unavailable, which is a far better outcome than refusing
                        // to start over a locked file.
                        tracing::error!(%error, "could not open the database");
                        return;
                    }
                };
                db_handle.state::<AppState>().set_data_dir(dir.clone());
                db_handle
                    .state::<AppState>()
                    .set_db(std::sync::Arc::clone(&db));
                tracing::info!("database open");

                // The search engine, built the way the eval harness builds it: hybrid
                // when the model and index are present, keyword-only otherwise. The
                // keyword-only path is not a fallback for convenience — it is the Tier 0
                // floor (SPEC.md §8, ADR-0014), and it is the same code path.
                db_handle
                    .state::<AppState>()
                    .set_engine(crate::commands::assets::engine_for(&dir));

                // The catalogue keeps itself current from here on: checked now and every
                // hour, an interrupted refresh finished first (Phase 16).
                tauri::async_runtime::spawn(crate::commands::catalogue::keep_current(
                    db_handle.clone(),
                ));
            });

            // The window stays hidden (tauri.conf.json `visible: false`) until the
            // frontend calls `frontend_ready`, so the user never sees an empty
            // frame during cold start. A watchdog reveals it anyway if the
            // frontend fails, because a permanently invisible window would be a
            // far worse failure than an ugly one.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                if let Some(w) = handle.get_webview_window("main") {
                    if !w.is_visible().unwrap_or(true) {
                        tracing::error!("frontend never reported ready; showing the window anyway");
                        let _ = w.show();
                    }
                }
            });

            tracing::info!(version = env!("CARGO_PKG_VERSION"), "started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
