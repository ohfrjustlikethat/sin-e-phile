//! Structured logging and the crash handler — SPEC.md Phase 1, subtask 1.10.
//!
//! §2.5: everything lives in `data/` next to the executable. §2.7: nothing
//! leaves the machine — crash reports are written locally and that is the end of
//! it. Both are properties of this module rather than promises made elsewhere.

use std::panic;
use std::path::{Path, PathBuf};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// The portable data directory: `data/` beside the executable (§2.5).
///
/// Falls back to the current directory only when the executable path cannot be
/// resolved, which in practice means a test harness.
pub fn data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("data")
}

pub fn init() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = data_dir().join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("could not create {}: {e}", log_dir.display());
        return None;
    }

    let appender = tracing_appender::rolling::daily(&log_dir, "sin-e-phile.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_env("SINEPHILE_LOG")
        .unwrap_or_else(|_| EnvFilter::new("sin_e_phile_lib=debug,warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false)
                .with_target(true),
        )
        // A second layer to stderr, but only in development. A release build
        // writes to the file alone.
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_filter(tracing_subscriber::filter::filter_fn(|_| {
                    cfg!(debug_assertions)
                })),
        )
        .init();

    install_panic_handler();
    tracing::info!(dir = %log_dir.display(), "logging started");

    // Phase 1 exit criterion: "a deliberately-triggered panic writes a crash log
    // and shows a graceful error screen". An exit criterion that cannot be
    // exercised on demand is not evidence, so the trigger ships — debug only.
    #[cfg(debug_assertions)]
    if std::env::var_os("SINEPHILE_PANIC_TEST").is_some() {
        panic!("deliberate panic: SINEPHILE_PANIC_TEST is set");
    }

    Some(guard)
}

/// Write a crash report locally on panic.
///
/// Deliberately does NOT phone home (§2.7). The report says where it went so a
/// user can choose to send it, which is a different thing from sending it.
fn install_panic_handler() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let dir = data_dir().join("crashes");
        let _ = std::fs::create_dir_all(&dir);

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("crash-{stamp}.txt"));

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());

        // Built line by line rather than with a multi-line string literal: a
        // `\`-continued literal keeps its source indentation, which lands in the
        // file and makes the report look broken to whoever opens it.
        let report = [
            "sin-e-phile crash report".to_string(),
            "------------------------".to_string(),
            format!("version   {}", env!("CARGO_PKG_VERSION")),
            format!("location  {location}"),
            format!("message   {message}"),
            String::new(),
            "backtrace".to_string(),
            format!("{}", std::backtrace::Backtrace::force_capture()),
            String::new(),
            "This file was written locally and sent nowhere (SPEC.md 2.7).".to_string(),
        ]
        .join("
");

        let _ = std::fs::write(&path, &report);
        tracing::error!(location = %location, message = %message, report = %path.display(), "PANIC");
        eprintln!("\npanic. crash report written to {}\n", path.display());

        previous(info);
    }));
}
