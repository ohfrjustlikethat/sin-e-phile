//! Scrub-bar thumbnails (SPEC.md Phase 11): sprite sheets made in the background by FFmpeg.
//!
//! One frame every [`INTERVAL`] seconds, [`WIDTH`] pixels wide, tiled [`COLS`]×[`ROWS`] to
//! a sheet, so a two-hour film is eight JPEGs rather than 720 files. The page shows a tile
//! by offsetting the sheet; [`tile_at`] is the arithmetic.
//!
//! **Without stuttering playback** (Phase 11 E2): FFmpeg runs at Windows' below-normal
//! priority on one thread, and decodes keyframes only (`-skip_frame nokey`), which is a
//! small fraction of the work of decoding the film. Only for files on this machine: on a
//! stream, sampling frames would download the whole film.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

pub const INTERVAL: f64 = 10.0;
pub const WIDTH: u32 = 160;
pub const COLS: u32 = 10;
pub const ROWS: u32 = 10;

/// What a finished set of sheets holds, written beside them as `sheets.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheets {
    pub interval: f64,
    pub cols: u32,
    pub rows: u32,
    /// One tile's size, in pixels.
    pub tile_width: u32,
    pub tile_height: u32,
    /// How many sheets.
    pub count: u32,
}

/// The tile for time `t`: (sheet index, column, row).
pub fn tile_at(t: f64, s: &Sheets) -> (u32, u32, u32) {
    let n = (t.max(0.0) / s.interval).floor() as u32;
    let per = s.cols * s.rows;
    let sheet = (n / per).min(s.count.saturating_sub(1));
    let i = if n / per > sheet { per - 1 } else { n % per };
    (sheet, i % s.cols, i / s.cols)
}

pub fn sheet_path(dir: &Path, index: u32) -> PathBuf {
    dir.join(format!("sheet-{:03}.jpg", index + 1))
}

/// Read a finished set, if `dir` holds one.
pub fn finished(dir: &Path) -> Option<Sheets> {
    let text = std::fs::read_to_string(dir.join("sheets.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Make the sheets for `film` into `dir`. Blocks until FFmpeg is done; run it on a
/// background thread. `aspect` is the film's height / width, for the tile height.
pub fn generate(film: &Path, dir: &Path, aspect: f64) -> std::io::Result<Sheets> {
    std::fs::create_dir_all(dir)?;
    let tile_height = ((f64::from(WIDTH) * aspect / 2.0).round() as u32 * 2).max(2);
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-nostdin",
        "-v",
        "error",
        "-y",
        "-threads",
        "1",
        "-skip_frame",
        "nokey",
        "-i",
    ])
    .arg(film)
    .args([
        "-an",
        "-sn",
        "-vf",
        &format!("fps=1/{INTERVAL},scale={WIDTH}:{tile_height},tile={COLS}x{ROWS}"),
        "-q:v",
        "6",
    ])
    .arg(dir.join("sheet-%03d.jpg"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "ffmpeg exited with {status}"
        )));
    }
    let count = (1..)
        .take_while(|i| sheet_path(dir, *i - 1).is_file())
        .count() as u32;
    let sheets = Sheets {
        interval: INTERVAL,
        cols: COLS,
        rows: ROWS,
        tile_width: WIDTH,
        tile_height,
        count,
    };
    std::fs::write(
        dir.join("sheets.json"),
        serde_json::to_string(&sheets).map_err(std::io::Error::other)?,
    )?;
    Ok(sheets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheets(count: u32) -> Sheets {
        Sheets {
            interval: 10.0,
            cols: 10,
            rows: 10,
            tile_width: 160,
            tile_height: 90,
            count,
        }
    }

    #[test]
    fn a_time_finds_its_tile() {
        let s = sheets(8);
        assert_eq!(tile_at(0.0, &s), (0, 0, 0));
        assert_eq!(tile_at(9.9, &s), (0, 0, 0));
        assert_eq!(tile_at(10.0, &s), (0, 1, 0));
        assert_eq!(
            tile_at(125.0, &s),
            (0, 2, 1),
            "the 13th frame: second row, third column"
        );
        assert_eq!(
            tile_at(1000.0, &s),
            (1, 0, 0),
            "the 101st frame starts the second sheet"
        );
    }

    #[test]
    fn past_the_last_frame_stays_on_it() {
        let s = sheets(1);
        assert_eq!(tile_at(5_000.0, &s), (0, 9, 9));
        assert_eq!(tile_at(-3.0, &s), (0, 0, 0));
    }
}
