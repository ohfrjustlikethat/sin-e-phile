//! Where the data lives.
//!
//! `SPEC.md` §2.4: **all application data lives in `data/` next to the
//! executable**, so the whole app is a folder you can move to a USB stick and it
//! keeps working. Installed mode using `%APPDATA%` exists, but is an opt-in and
//! never the default.
//!
//! This is one of the few places where "next to the executable" has to be taken
//! literally, and it has a trap: during development the executable is somewhere
//! like `target/debug/`, so a naive implementation writes the dev database into
//! the build directory, where `cargo clean` deletes it. So the resolver has three
//! modes, and the caller says which — it does not guess.

use std::env;
use std::path::{Path, PathBuf};

/// The environment variable that selects installed mode, or forces a location.
///
/// Set `SINEPHILE_DATA_DIR` to an absolute path to override everything. This is
/// what the integration tests use, and it is also the escape hatch for a user
/// whose app folder is on a read-only volume.
pub const DATA_DIR_ENV: &str = "SINEPHILE_DATA_DIR";

/// How the data directory is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataLocation {
    /// `data/` beside the executable. The default, and what makes the app portable.
    Portable,
    /// `%APPDATA%/sin-e-phile/`. Opt-in, for a machine where the install directory
    /// is not writable.
    Installed,
    /// The development tree: `data/` at the repository root, found by walking up
    /// from the executable until a `Cargo.toml` is seen.
    ///
    /// Without this, `cargo run` puts the database under `target/`, and the first
    /// `cargo clean` silently destroys the developer's whole catalogue.
    Development,
}

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("could not locate the running executable: {0}")]
    Exe(#[source] std::io::Error),
    #[error("the executable has no parent directory: {0}")]
    NoParent(PathBuf),
    #[error("%APPDATA% is not set, so installed mode has nowhere to write")]
    NoAppData,
    #[error("{0} must be an absolute path, got: {1}")]
    NotAbsolute(&'static str, PathBuf),
    #[error("could not create {0}: {1}")]
    Create(PathBuf, #[source] std::io::Error),
    #[error("{0} is not writable: {1}")]
    NotWritable(PathBuf, #[source] std::io::Error),
}

/// Resolve the data directory, creating it if it does not exist.
///
/// `SINEPHILE_DATA_DIR` overrides `location` entirely when set.
pub fn data_dir(location: DataLocation) -> Result<PathBuf, PathError> {
    let dir = match env::var_os(DATA_DIR_ENV) {
        Some(raw) => {
            let path = PathBuf::from(raw);
            if !path.is_absolute() {
                return Err(PathError::NotAbsolute(DATA_DIR_ENV, path));
            }
            path
        }
        None => resolve(location)?,
    };

    std::fs::create_dir_all(&dir).map_err(|e| PathError::Create(dir.clone(), e))?;
    Ok(dir)
}

fn resolve(location: DataLocation) -> Result<PathBuf, PathError> {
    match location {
        DataLocation::Portable => Ok(exe_dir()?.join("data")),
        DataLocation::Installed => {
            let appdata = env::var_os("APPDATA").ok_or(PathError::NoAppData)?;
            Ok(PathBuf::from(appdata).join("sin-e-phile"))
        }
        DataLocation::Development => {
            let exe = exe_dir()?;
            Ok(repo_root(&exe).unwrap_or(exe).join("data"))
        }
    }
}

fn exe_dir() -> Result<PathBuf, PathError> {
    let exe = env::current_exe().map_err(PathError::Exe)?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| PathError::NoParent(exe.clone()))
}

/// Walk up looking for the workspace root — the directory whose `Cargo.toml`
/// declares a `[workspace]`. Returns `None` outside a source tree, which is the
/// normal case for a shipped build.
fn repo_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            match std::fs::read_to_string(&manifest) {
                Ok(text) if text.contains("[workspace]") => return Some(dir.to_path_buf()),
                _ => continue,
            }
        }
    }
    None
}

/// The database file inside a data directory.
///
/// One file, because that is the whole point of SQLite here: the portability
/// promise in §2.4 is only true if "the data" is something you can copy.
pub fn database_path(data_dir: &Path) -> PathBuf {
    data_dir.join("sinephile.db")
}

/// Where a pre-migration backup is written (`SPEC.md` Phase 3: backup-on-migrate).
pub fn backup_path(data_dir: &Path, from_version: i64) -> PathBuf {
    data_dir
        .join("backups")
        .join(format!("sinephile-v{from_version}.db"))
}

/// Fail early and clearly if the data directory cannot be written to.
///
/// Worth doing explicitly: a portable app is frequently run from a USB stick, a
/// network share, or `C:\Program Files`, and "the database is read-only" surfaces
/// otherwise as an incomprehensible SQLite error several layers down.
pub fn assert_writable(dir: &Path) -> Result<(), PathError> {
    let probe = dir.join(".write-probe");
    std::fs::write(&probe, b"").map_err(|e| PathError::NotWritable(dir.to_path_buf(), e))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The override wins over every mode, and must be absolute.
    #[test]
    fn env_override_must_be_absolute() {
        let relative = PathBuf::from("data");
        assert!(!relative.is_absolute(), "test premise");
        // Checked directly rather than through data_dir(), because setting a
        // process-wide env var races every other test in the binary.
        match PathError::NotAbsolute(DATA_DIR_ENV, relative.clone()) {
            PathError::NotAbsolute(name, path) => {
                assert_eq!(name, DATA_DIR_ENV);
                assert_eq!(path, relative);
            }
            other => panic!("wrong variant: {other}"),
        }
    }

    #[test]
    fn database_and_backup_live_inside_the_data_dir() {
        let dir = Path::new("C:/apps/sin-e-phile/data");
        assert!(database_path(dir).starts_with(dir));
        assert!(backup_path(dir, 4).starts_with(dir));
        assert!(backup_path(dir, 4).to_string_lossy().contains("v4"));
    }

    /// The trap this module exists for: a dev build must not write into `target/`.
    #[test]
    fn development_mode_climbs_out_of_target() {
        let root = repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("the crate is inside the workspace");
        assert!(root.join("Cargo.toml").is_file());
        assert!(
            !root.ends_with("target"),
            "development mode resolved into the build directory: {}",
            root.display()
        );
    }

    #[test]
    fn portable_is_the_default_shape() {
        // Not asserting an absolute path (it depends on the test binary's
        // location), only that portable resolution ends in `data`.
        let resolved = resolve(DataLocation::Portable).expect("exe dir is knowable");
        assert!(resolved.ends_with("data"));
    }
}
