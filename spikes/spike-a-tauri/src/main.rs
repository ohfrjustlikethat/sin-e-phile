//! Spike A, step 3 — THE question R1 actually asks.
//!
//! Can a native libmpv video surface sit inside a Tauri v2 window with HTML UI
//! composited over it?
//!
//! Approach under test: **child window under a transparent webview.**
//!   1. Tauri creates a top-level window; WebView2 lives in a child HWND of it.
//!   2. We create our own child HWND for mpv, as a sibling of the webview.
//!   3. We push the mpv child BELOW the webview in z-order.
//!   4. The window and the webview are transparent, and the page paints no
//!      background — so the video shows through wherever the HTML does not paint.
//!
//! If the badge, the dashed box and the bottom chrome all render over moving
//! video, R1's primary approach works and Phase 8 is unblocked.
//!
//! Usage: spike-a-tauri <libmpv-2.dll> <video>

use std::time::{Duration, Instant};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{w, BOOL, Interface, PCWSTR};

mod mpv;
use mpv::*;

/// The mpv host child needs no behaviour of its own; mpv paints it.
unsafe extern "system" fn video_host_proc(
    hwnd: HWND, msg: u32, wp: windows::Win32::Foundation::WPARAM, lp: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// Find the WebView2 host child window inside the Tauri top-level window.
/// WebView2 hosts under a `Chrome_WidgetWin_*` class.
unsafe extern "system" fn find_webview(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut class = [0u16; 128];
    let n = GetClassNameW(hwnd, &mut class);
    let name = String::from_utf16_lossy(&class[..n as usize]);
    if name.starts_with("Chrome_WidgetWin") {
        *(lparam.0 as *mut HWND) = hwnd;
        return BOOL(0); // stop enumerating
    }
    BOOL(1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: spike-a-tauri <libmpv-2.dll> <video>");
        std::process::exit(2);
    }
    let (dll, video) = (args[1].clone(), args[2].clone());

    tauri::Builder::default()
        .setup(move |app| {
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Spike A - transparent WebView2 over libmpv")
                .inner_size(1280.0, 800.0)
                .transparent(true)   // required for the webview to composite
                .decorations(true)
                .build()?;

            // ATTEMPT 2. `transparent(true)` alone was not enough: the webview
            // painted opaque black over the video (attempt 1). WebView2 has its own
            // background, separate from the window's, and it must be set to a fully
            // transparent colour via ICoreWebView2Controller2::put_DefaultBackgroundColor.
            let bg_set = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            {
                let flag = bg_set.clone();
                win.with_webview(move |wv| unsafe {
                    use webview2_com::Microsoft::Web::WebView2::Win32::{
                        ICoreWebView2Controller2, COREWEBVIEW2_COLOR,
                    };
                    let controller = wv.controller();
                    if let Ok(c2) = controller.cast::<ICoreWebView2Controller2>() {
                        let transparent = COREWEBVIEW2_COLOR { A: 0, R: 0, G: 0, B: 0 };
                        match c2.SetDefaultBackgroundColor(transparent) {
                            Ok(()) => {
                                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                                println!("webview DefaultBackgroundColor -> transparent (A=0)");
                            }
                            Err(e) => println!("SetDefaultBackgroundColor failed: {e}"),
                        }
                    } else {
                        println!("ICoreWebView2Controller2 not available");
                    }
                })?;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
            println!("transparent background applied: {}",
                bg_set.load(std::sync::atomic::Ordering::SeqCst));

            let top = HWND(win.hwnd()?.0 as *mut _);
            println!("tauri top-level hwnd  {:?}", top.0);

            unsafe {
                // Locate the webview child so we can position mpv relative to it.
                let mut webview = HWND(std::ptr::null_mut());
                // windows 0.61 takes Option<HWND> for the parent.
                let _ = EnumChildWindows(
                    Some(top),
                    Some(find_webview),
                    LPARAM(&mut webview as *mut HWND as isize),
                );
                println!("webview child hwnd    {:?}", webview.0);
                if webview.0.is_null() {
                    println!("WARNING: no Chrome_WidgetWin child found");
                }

                let instance = GetModuleHandleW(None)?;
                let class = WNDCLASSW {
                    lpfnWndProc: Some(video_host_proc),
                    hInstance: instance.into(),
                    lpszClassName: w!("SpikeVideoHost"),
                    ..Default::default()
                };
                RegisterClassW(&class);

                let mut rect = RECT::default();
                let _ = GetClientRect(top, &mut rect);

                let video_child = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("SpikeVideoHost"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                    0, 0, rect.right, rect.bottom,
                    Some(top), None, Some(instance.into()), None,
                )?;
                println!("mpv child hwnd        {:?}", video_child.0);

                // ATTEMPT 3. Attempts 1 and 2 put the video BELOW the webview and made
                // the webview transparent; the video never appeared, because a
                // transparent webview composites against what is behind the WINDOW,
                // not against sibling child HWNDs.
                //
                // So invert it: put the video ON TOP, clipped to a rect that leaves
                // room around it. If video appears in that rect while HTML renders
                // around it, the mechanism works but only for UI BESIDE video, not
                // UI OVER video - which is the distinction that decides R1.
                let mode = std::env::var("SPIKE_ZORDER").unwrap_or_else(|_| "top".into());
                let inset = 120;
                match mode.as_str() {
                    "bottom" => {
                        let _ = SetWindowPos(video_child, Some(HWND_BOTTOM), 0, 0, 0, 0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
                        println!("z-order: video BELOW webview (attempts 1-2)");
                    }
                    _ => {
                        let _ = SetWindowPos(
                            video_child, Some(HWND_TOP),
                            inset, inset,
                            (rect.right - inset * 2).max(320),
                            (rect.bottom - inset * 2).max(240),
                            SWP_SHOWWINDOW,
                        );
                        println!("z-order: video ABOVE webview, inset {inset}px");
                    }
                }

                let m = Mpv::load(std::path::Path::new(&dll))
                    .map_err(|e| tauri::Error::Anyhow(anyhow_from(e)))?;
                m.set_option("terminal", "no").ok();
                m.set_option("hwdec", "auto-safe").ok();
                m.set_option("keep-open", "yes").ok();
                m.set_option("loop-file", "inf").ok();
                m.set_option("wid", &(video_child.0 as usize).to_string()).ok();
                m.request_log_messages("warn").ok();
                m.initialize().map_err(|e| tauri::Error::Anyhow(anyhow_from(e)))?;

                let t0 = Instant::now();
                m.command(&["loadfile", &video]).ok();

                // Pump mpv events on a worker so the Tauri event loop stays free.
                std::thread::spawn(move || {
                    let mut reported = false;
                    loop {
                        let (id, text) = m.wait_event(0.25);
                        if let Some(t) = text {
                            println!("  mpv {t}");
                        }
                        if id == MPV_EVENT_PLAYBACK_RESTART && !reported {
                            reported = true;
                            println!(
                                "FIRST FRAME           {:>7.1} ms  (into a Tauri child HWND)",
                                t0.elapsed().as_secs_f64() * 1000.0
                            );
                            println!("  hwdec-current       {:?}", m.get_property("hwdec-current"));
                        }
                        if id == MPV_EVENT_SHUTDOWN {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri run failed");
}

fn anyhow_from(e: String) -> anyhow::Error {
    anyhow::anyhow!(e)
}
