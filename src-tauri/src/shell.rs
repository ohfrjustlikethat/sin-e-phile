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
