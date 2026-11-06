//! Scrub-bar thumbnails (SPEC.md Phase 11). **Thin**: making the sheets is
//! `sinephile_player::thumbs`; this starts it for the film that opened and hands the page
//! a sheet when it asks.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use sinephile_player::thumbs::{self, Sheets};

/// Where the playing film's sheets are (or will be).
static CURRENT: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SheetsView {
    pub interval: f64,
    pub cols: u32,
    pub rows: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub count: u32,
}

impl From<Sheets> for SheetsView {
    fn from(s: Sheets) -> Self {
        Self {
            interval: s.interval,
            cols: s.cols,
            rows: s.rows,
            tile_width: s.tile_width,
            tile_height: s.tile_height,
            count: s.count,
        }
    }
}

/// After the first frame: for a file on this machine, make its sheets in the background
/// unless they exist. A stream gets none (sampling it would download the film).
pub(crate) fn start_for(target: &str, size: Option<(i64, i64)>) {
    let film = std::path::Path::new(target);
    if !film.is_file() {
        *CURRENT.lock().expect("thumbs") = None;
        return;
    }
    // Keyed by content, so a moved file keeps its sheets.
    let key = sinephile_subtitles::hash::of_file(film)
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("{:x}", target.len()));
    let dir = crate::logging::data_dir()
        .join("cache")
        .join("thumbs")
        .join(key);
    *CURRENT.lock().expect("thumbs") = Some(dir.clone());
    if thumbs::finished(&dir).is_some() {
        return;
    }
    let aspect = size
        .filter(|(w, _)| *w > 0)
        .map(|(w, h)| h as f64 / w as f64)
        .unwrap_or(9.0 / 16.0);
    let film = film.to_path_buf();
    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        match thumbs::generate(&film, &dir, aspect) {
            Ok(s) => tracing::info!(
                sheets = s.count,
                ms = t0.elapsed().as_millis(),
                "scrub thumbnails made"
            ),
            Err(e) => tracing::warn!(error = %e, "scrub thumbnails failed"),
        }
    });
}

/// The playing film's sheets, once made.
#[tauri::command]
#[specta::specta]
pub async fn scrub_thumbs() -> Result<Option<SheetsView>, String> {
    let dir = CURRENT.lock().expect("thumbs").clone();
    Ok(dir.and_then(|d| thumbs::finished(&d)).map(SheetsView::from))
}

/// One sheet, as a `data:` URI.
#[tauri::command]
#[specta::specta]
pub async fn scrub_sheet(index: u32) -> Result<String, String> {
    let dir = CURRENT
        .lock()
        .expect("thumbs")
        .clone()
        .ok_or("no thumbnails for this film")?;
    let bytes = std::fs::read(thumbs::sheet_path(&dir, index)).map_err(|e| e.to_string())?;
    Ok(sinephile_player::still::data_uri(&bytes))
}
