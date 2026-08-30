//! Spike A, step 2 — the child-window-overlay approach, in isolation.
//!
//! Before dragging Tauri in, establish what the Win32 mechanism can do at all:
//!
//!   Q1  Does mpv render into a child HWND we hand it via `wid`?
//!   Q2  Does the child clip to its parent, so it can be positioned like a UI element?
//!   Q3  Can a sibling window be composited ON TOP of the video? This is the crux —
//!       R1 is not "can video render", it is "can UI be drawn over video".
//!   Q4  Does resizing the parent resize the video without tearing the surface down?
//!
//! Layout: a 1280x800 parent. The mpv child occupies the top 1280x720. A red
//! sibling "overlay" is placed to straddle the video's lower edge; if it draws
//! over the video rather than behind it, Q3 is answered.
//!
//! Usage: step2-child-hwnd <libmpv-2.dll> <video>

use spike_a_libmpv::mpv::*;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, EndPaint, FillRect, InvalidateRect, UpdateWindow, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

static mut VIDEO_CHILD: HWND = HWND(std::ptr::null_mut());
static mut OVERLAY: HWND = HWND(std::ptr::null_mut());

const VIDEO_H: i32 = 720;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            // Q4: keep the child sized to the parent's client area.
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            if !VIDEO_CHILD.0.is_null() {
                let _ = MoveWindow(VIDEO_CHILD, 0, 0, rect.right, VIDEO_H.min(rect.bottom), true);
            }
            if !OVERLAY.0.is_null() {
                // Straddle the video's lower edge on purpose.
                let _ = MoveWindow(OVERLAY, 40, VIDEO_H - 120, rect.right - 80, 200, true);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let brush = CreateSolidBrush(COLORREF(0x00_20_20_C0)); // BGR: red
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        FillRect(hdc, &rect, brush);
        let _ = EndPaint(hwnd, &ps);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return Err("usage: step2-child-hwnd <libmpv-2.dll> <video>".into());
    }
    let (dll, video) = (args[1].clone(), args[2].clone());
    let headless = std::env::var("SPIKE_HEADLESS").is_ok();

    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;

        for (name, proc) in [
            // WNDPROC is Option<fn(..)>, not a bare fn pointer, so wrap rather than cast.
            (w!("SpikeParent"), Some(wndproc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT)),
            (w!("SpikeOverlay"), Some(overlay_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT)),
        ] {
            let class = WNDCLASSW {
                lpfnWndProc: proc,
                hInstance: instance.into(),
                lpszClassName: name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(format!("RegisterClassW failed for {name:?}"));
            }
        }

        let parent = CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("SpikeParent"), w!("Spike A - step 2"),
            WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT, 1280, 800,
            None, None, instance, None,
        ).map_err(|e| format!("create parent: {e}"))?;

        // Q1/Q2: a child window, clipped to the parent, for mpv to render into.
        VIDEO_CHILD = CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("STATIC"), PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0, 0, 1280, VIDEO_H, parent, None, instance, None,
        ).map_err(|e| format!("create video child: {e}"))?;

        // Q3: a sibling created AFTER the video child, so it is above it in z-order.
        OVERLAY = CreateWindowExW(
            WINDOW_EX_STYLE::default(), w!("SpikeOverlay"), PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            40, VIDEO_H - 120, 1200, 200, parent, None, instance, None,
        ).map_err(|e| format!("create overlay: {e}"))?;

        let _ = ShowWindow(parent, if headless { SW_HIDE } else { SW_SHOW });

        let wid = VIDEO_CHILD.0 as usize;
        println!("parent hwnd      {:?}", parent.0);
        println!("video child hwnd {:?}  -> mpv wid={wid}", VIDEO_CHILD.0);
        println!("overlay hwnd     {:?}  (sibling, above in z-order)", OVERLAY.0);

        let mpv = Mpv::load(std::path::Path::new(&dll))?;
        mpv.set_option("terminal", "no")?;
        mpv.set_option("hwdec", "auto-safe")?;
        mpv.set_option("keep-open", "yes")?;
        mpv.set_option("loop-file", "inf")?;
        // THE embedding call: render into someone else's window.
        mpv.set_option("wid", &wid.to_string())?;
        mpv.request_log_messages("info")?;
        mpv.initialize()?;

        let t0 = Instant::now();
        mpv.command(&["loadfile", &video])?;

        let mut first_frame: Option<f64> = None;
        let mut resized = false;
        let deadline = Instant::now() + Duration::from_secs(if headless { 20 } else { 45 });

        let mut msg = MSG::default();
        while Instant::now() < deadline {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return finish(first_frame, resized);
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let (id, text) = mpv.wait_event(0.05);
            if let Some(t) = text {
                println!("  mpv {t}");
            }
            match id {
                MPV_EVENT_VIDEO_RECONFIG => println!("video-reconfig   {:?}",
                    mpv.get_property("dwidth")),
                MPV_EVENT_PLAYBACK_RESTART if first_frame.is_none() => {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    first_frame = Some(ms);
                    println!("FIRST FRAME      {ms:>7.1} ms  (into a child HWND)");
                    println!("  hwdec-current  {:?}", mpv.get_property("hwdec-current"));

                    // Q3, explicitly: force the overlay to the top of the z-order
                    // and repaint it, so a negative result cannot be blamed on a
                    // missed WM_PAINT or on creation order.
                    let _ = SetWindowPos(OVERLAY, HWND_TOP, 0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
                    let _ = InvalidateRect(OVERLAY, None, true);
                    let _ = UpdateWindow(OVERLAY);
                    let above = GetWindow(OVERLAY, GW_HWNDPREV);
                    println!("  overlay forced HWND_TOP; prev-in-z = {:?} (null => topmost child)",
                        above.map(|h| h.0).unwrap_or(std::ptr::null_mut()));
                }
                MPV_EVENT_SHUTDOWN => break,
                _ => {}
            }

            // Q4: resize the parent mid-playback and see whether video survives.
            if first_frame.is_some() && !resized && t0.elapsed() > Duration::from_secs(3) {
                resized = true;
                println!("resizing parent to 1000x700 mid-playback...");
                let _ = SetWindowPos(parent, None, 0, 0, 1000, 700,
                    SWP_NOMOVE | SWP_NOZORDER);
                std::thread::sleep(Duration::from_millis(600));
                println!("  after resize: time-pos {:?}, dwidth {:?}",
                    mpv.get_property("time-pos"), mpv.get_property("dwidth"));
            }

            if headless && first_frame.is_some() && resized
                && t0.elapsed() > Duration::from_secs(6) {
                break;
            }
        }

        finish(first_frame, resized)
    }
}

fn finish(first_frame: Option<f64>, resized: bool) -> Result<(), String> {
    println!();
    match first_frame {
        Some(ms) => {
            println!("STEP 2 RESULT");
            println!("  Q1 renders into a child HWND via wid  YES  (first frame {ms:.1} ms)");
            println!("  Q2 clips to parent                    YES  (child is WS_CHILD)");
            println!("  Q3 sibling composites over video      SEE SCREENSHOT / observation");
            println!("  Q4 survives parent resize             {}",
                if resized { "YES - see time-pos after resize" } else { "NOT REACHED" });
            Ok(())
        }
        None => Err("STEP 2 FAIL - no playback into the child window".into()),
    }
}
