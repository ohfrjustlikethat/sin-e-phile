//! Region cutouts: punch holes in the video child so the webview shows through.
//!
//! SPIKE CODE. Throwaway (SPEC.md Phase 1, risk R1, blocker B2 option 5).
//!
//! The inversion, from ventic/ventic's X11 backend: rather than compositing UI
//! over the video — which Windows will not do for a child HWND — cut the chrome
//! rectangles OUT of the video window. The page underneath shows through, and
//! input follows the shape because a window's hit-test region follows its
//! bounding region.
//!
//! `SetWindowRgn` is the Windows counterpart of the X11 Shape extension. Child
//! windows are clipped to their parent's region, so a region on the child we own
//! also clips mpv's own render window living inside it.
//!
//! FLICKER is the risk that decides this approach. §11's chrome auto-hides after
//! 2.5 s idle, so the region changes on every show and every hide. Two defences
//! are implemented and measured here:
//!   - the host class has a NULL background brush and swallows WM_ERASEBKGND, so
//!     Windows never paints the newly-exposed area before mpv repaints it;
//!   - `SetWindowRgn` is called with redraw=FALSE, then a targeted invalidate is
//!     issued only where it is actually needed.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, DeleteObject, SetWindowRgn, HRGN, RGN_DIFF,
};

/// Apply a region to `hwnd` covering its full extent minus `holes`.
///
/// Returns the wall-clock cost of the `SetWindowRgn` call itself, in
/// microseconds, which is the number that matters for the auto-hide budget.
///
/// SAFETY: `hwnd` must be a live window owned by the calling thread.
pub unsafe fn apply_cutout(hwnd: HWND, w: i32, h: i32, holes: &[RECT], redraw: bool) -> u128 {
    let full: HRGN = CreateRectRgn(0, 0, w, h);
    for hole in holes {
        let cut = CreateRectRgn(hole.left, hole.top, hole.right, hole.bottom);
        // full = full - cut
        CombineRgn(Some(full), Some(full), Some(cut), RGN_DIFF);
        let _ = DeleteObject(cut.into());
    }
    let t0 = std::time::Instant::now();
    // NOTE: SetWindowRgn takes OWNERSHIP of the region on success. Deleting it
    // afterwards would be a double free, so `full` is deliberately not deleted.
    SetWindowRgn(hwnd, Some(full), redraw);
    t0.elapsed().as_micros()
}

/// Remove any region, restoring the full rectangle.
///
/// SAFETY: as `apply_cutout`.
pub unsafe fn clear_cutout(hwnd: HWND, redraw: bool) -> u128 {
    let t0 = std::time::Instant::now();
    SetWindowRgn(hwnd, None, redraw);
    t0.elapsed().as_micros()
}
