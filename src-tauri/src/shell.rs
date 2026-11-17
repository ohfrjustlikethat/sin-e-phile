//! Handing an address or a folder to Windows (Phase 16): an add-on's own settings page in the
//! browser, and (item 14) a finished download's folder in Explorer. Thin, like the rest of
//! `src-tauri`: the one decision, what may be opened, is made by the caller.

use std::process::Command;

/// Open a web address in the person's browser. Only `https` and `http` are accepted, so a
/// manifest can never make the app run anything else.
pub fn open_in_browser(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only web addresses are opened in the browser".into());
    }
    // The documented shell handler for URLs: no command line is parsed, so nothing in the
    // address can become an argument.
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open the browser: {e}"))
}

/// Show a file in Explorer, selected (a finished download, Phase 16). The path must be a
/// file that exists, so Explorer is only ever asked to point at that one file.
pub fn show_in_explorer(path: &std::path::Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("the file is no longer where it was downloaded".into());
    }
    // Explorer wants `/select,"<path>"` exactly; Rust would quote the whole argument, which
    // it misreads. Raw is safe: a Windows path cannot contain a quote.
    use std::os::windows::process::CommandExt;
    let path = path.to_string_lossy();
    if path.contains('"') {
        return Err("that path cannot be shown".into());
    }
    Command::new("explorer")
        .raw_arg(format!("/select,\"{path}\""))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open Explorer: {e}"))
}
