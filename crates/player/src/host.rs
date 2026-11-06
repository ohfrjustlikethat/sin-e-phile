//! The native window libmpv draws into — ADR-0020 and ADR-0021, from Spike A.
//!
//! A child window of the app's main window, above the WebView. Windows cannot composite
//! HTML over a native child, so the player's chrome is shown through **holes**: the
//! child's window region is its full rectangle minus each chrome panel's silhouette, so
//! the page beneath shows (and receives clicks) exactly where a panel is.
//!
//! The rules Spike A measured, kept here:
//!
//! - **The hole and the panel are the same shape.** A rounded panel needs a rounded
//!   hole (`CreateRoundRectRgn`); any part of a hole the panel does not paint shows the
//!   desktop through the transparent window.
//! - **No flicker** needs both a NULL class background brush and swallowing
//!   `WM_ERASEBKGND`, plus `SetWindowRgn(redraw = false)`. Without them Windows erases
//!   newly exposed area before mpv repaints it.
//! - On pause the window is **hidden**, and the page shows a still frame instead, so the
//!   pause overlay can dim and blur with ordinary HTML (ADR-0021).

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, HRGN, RGN_DIFF,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowThreadProcessId, RegisterClassW,
    SetWindowPos, ShowWindow, HWND_TOP, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNA, WINDOW_EX_STYLE,
    WM_ERASEBKGND, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
};

/// A chrome panel's silhouette, in physical pixels relative to the host's top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Cutout {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Corner radius; 0 is square. Must match the panel's CSS radius (ADR-0020).
    pub radius: i32,
}

pub struct VideoHost {
    hwnd: HWND,
    width: i32,
    height: i32,
}

// SAFETY: the HWND is only an identifier here. SetWindowPos, SetWindowRgn and ShowWindow
// may be called from any thread: Windows forwards them to the thread that owns the
// window, which `new` guarantees is the UI thread, so they are answered. DestroyWindow
// only works from the owning thread; from any other it fails harmlessly, and the window
// goes when its parent does.
unsafe impl Send for VideoHost {}
unsafe impl Sync for VideoHost {}

const CLASS: windows::core::PCWSTR = w!("SinephileVideoHost");

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_ERASEBKGND {
        // Claim it was erased; mpv paints every pixel itself.
        return LRESULT(1);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

impl VideoHost {
    /// Create the host as a child of `parent` (the app window's HWND as an integer),
    /// hidden until `show`.
    ///
    /// **Must run on the thread that owns `parent`** — the UI thread. A window belongs
    /// to the thread that creates it, and Windows sends a child synchronous messages
    /// whenever its parent moves, paints or is printed. Created on a thread that never
    /// pumps messages (an async command's worker), nothing answers them and the whole
    /// app window hangs, grey. So any other thread gets an error, not a window.
    pub fn new(parent: isize) -> Result<Self, String> {
        unsafe {
            let owner = GetWindowThreadProcessId(HWND(parent as *mut _), None);
            if owner == 0 {
                return Err("the parent is not a window".into());
            }
            if owner != GetCurrentThreadId() {
                return Err(
                    "the video window must be created on the thread that owns the app window"
                        .into(),
                );
            }
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: CLASS,
                // hbrBackground left NULL on purpose (see the module note).
                ..Default::default()
            };
            // Registering twice fails harmlessly with "class already exists".
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                CLASS,
                w!(""),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                Some(HWND(parent as *mut _)),
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| format!("could not create the video window: {e}"))?;
            Ok(Self {
                hwnd,
                width: 1,
                height: 1,
            })
        }
    }

    /// The value libmpv's `wid` option takes.
    pub fn wid(&self) -> i64 {
        self.hwnd.0 as i64
    }

    /// Place the host, in physical pixels within the parent's client area, on top of
    /// the WebView.
    pub fn set_bounds(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.width = width.max(1);
        self.height = height.max(1);
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                x,
                y,
                self.width,
                self.height,
                SWP_NOACTIVATE,
            );
        }
    }

    /// Cut the chrome's silhouettes out of the video, so the page shows through them.
    /// An empty list restores the full rectangle.
    pub fn set_cutouts(&self, cutouts: &[Cutout]) -> Result<(), String> {
        unsafe {
            let region = region(self.width, self.height, cutouts)?;
            // The system owns the region after this call, success or not.
            if SetWindowRgn(self.hwnd, Some(region), false) == 0 {
                return Err("SetWindowRgn failed".into());
            }
        }
        Ok(())
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// The raw handle, for tests and diagnostics.
    pub fn hwnd(&self) -> isize {
        self.hwnd.0 as isize
    }
}

impl Drop for VideoHost {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Where the page wants the video, in CSS pixels within the window's client area, with
/// `scale` the page's `devicePixelRatio`. Cutouts are in the same space as the stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    /// `(x, y, width, height, radius)` of each chrome panel, in CSS pixels.
    pub cutouts: Vec<(f64, f64, f64, f64, f64)>,
}

impl Placement {
    /// The host's bounds `(x, y, width, height)` in physical pixels, and the holes
    /// relative to the host's own top-left.
    pub fn physical(&self) -> ((i32, i32, i32, i32), Vec<Cutout>) {
        let px = |v: f64| (v * self.scale).round() as i32;
        let bounds = (px(self.x), px(self.y), px(self.width), px(self.height));
        let holes = self
            .cutouts
            .iter()
            .map(|&(x, y, width, height, radius)| Cutout {
                x: px(x - self.x),
                y: px(y - self.y),
                width: px(width),
                height: px(height),
                radius: px(radius),
            })
            .collect();
        (bounds, holes)
    }
}

impl VideoHost {
    /// Size and place the host and cut its holes, from the page's request.
    pub fn place(&mut self, p: &Placement) -> Result<(), String> {
        let ((x, y, w, h), holes) = p.physical();
        self.set_bounds(x, y, w, h);
        self.set_cutouts(&holes)
    }
}

/// The window's region: its rectangle minus each cutout. Pure geometry over GDI, kept
/// separate so a test can check it without a window.
pub(crate) unsafe fn region(width: i32, height: i32, cutouts: &[Cutout]) -> Result<HRGN, String> {
    let full = CreateRectRgn(0, 0, width, height);
    for c in cutouts {
        let hole = if c.radius > 0 {
            // CreateRoundRectRgn takes the ellipse's width and height, so 2 × radius.
            CreateRoundRectRgn(
                c.x,
                c.y,
                c.x + c.width + 1,
                c.y + c.height + 1,
                c.radius * 2,
                c.radius * 2,
            )
        } else {
            CreateRectRgn(c.x, c.y, c.x + c.width, c.y + c.height)
        };
        CombineRgn(Some(full), Some(full), Some(hole), RGN_DIFF);
        let _ = DeleteObject(hole.into());
    }
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::PtInRegion;
    use windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW;

    #[test]
    fn a_cutout_removes_exactly_its_shape() {
        unsafe {
            let r = region(
                800,
                450,
                &[Cutout {
                    x: 100,
                    y: 350,
                    width: 600,
                    height: 60,
                    radius: 0,
                }],
            )
            .unwrap();
            assert!(PtInRegion(r, 50, 50).as_bool(), "video outside the panel");
            assert!(!PtInRegion(r, 400, 380).as_bool(), "the panel is a hole");
            assert!(
                PtInRegion(r, 99, 380).as_bool(),
                "one pixel left of the panel is video"
            );
            assert!(
                !PtInRegion(r, 100, 380).as_bool(),
                "the panel's first pixel is the hole"
            );
            let _ = DeleteObject(r.into());
        }
    }

    /// A top-level window owned by the calling thread, standing in for the app window.
    fn parent_window() -> HWND {
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                200,
                200,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        }
    }

    #[test]
    fn a_placement_becomes_physical_pixels_relative_to_the_host() {
        // 150 % scaling: the stage starts 10 CSS px in; a bottom bar 20 px from the stage edge.
        let p = Placement {
            x: 10.0,
            y: 0.0,
            width: 800.0,
            height: 450.0,
            scale: 1.5,
            cutouts: vec![(30.0, 400.0, 760.0, 40.0, 2.0)],
        };
        let (bounds, holes) = p.physical();
        assert_eq!(bounds, (15, 0, 1200, 675));
        assert_eq!(
            holes,
            vec![Cutout {
                x: 30,
                y: 600,
                width: 1140,
                height: 60,
                radius: 3
            }]
        );
    }

    #[test]
    fn the_host_is_created_only_on_the_parents_thread() {
        // Created from a worker thread, the host hung the whole app window (Phase 8).
        let parent = parent_window();
        let raw = parent.0 as isize;
        let elsewhere = std::thread::spawn(move || VideoHost::new(raw).err())
            .join()
            .unwrap();
        assert!(
            elsewhere.is_some_and(|e| e.contains("thread that owns")),
            "a worker thread must be refused"
        );
        let here = VideoHost::new(raw).expect("the owning thread may create it");
        drop(here);
        assert!(VideoHost::new(0).is_err(), "no parent, no window");
        unsafe {
            let _ = DestroyWindow(parent);
        }
    }

    #[test]
    fn a_rounded_panel_leaves_its_corners_as_video() {
        // ADR-0020: a square hole under a rounded panel shows the desktop at the corners.
        unsafe {
            let r = region(
                800,
                450,
                &[Cutout {
                    x: 100,
                    y: 300,
                    width: 600,
                    height: 100,
                    radius: 12,
                }],
            )
            .unwrap();
            assert!(
                PtInRegion(r, 101, 301).as_bool(),
                "the very corner is outside the rounded hole"
            );
            assert!(
                !PtInRegion(r, 112, 312).as_bool(),
                "inside the curve is the hole"
            );
            assert!(
                !PtInRegion(r, 400, 301).as_bool(),
                "the straight top edge is the hole"
            );
            let _ = DeleteObject(r.into());
        }
    }
}
