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
use windows::Win32::Graphics::Gdi::{ClientToScreen, InvalidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{w, BOOL, Interface, PCWSTR};

mod cutout;
mod mpv;
mod snapshot;
use mpv::*;

use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

/// Shared bits the pause command needs. Spike-grade: a real implementation
/// would not reach for statics.
struct PauseCtx {
    mpv: Mpv,
    video_child: isize,
}
static CTX: Mutex<Option<PauseCtx>> = Mutex::new(None);
static PAUSE_T0: AtomicI64 = AtomicI64::new(0);

fn now_us() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64
}

/// THE measurement (blocker B2, option 1). Pause, capture, downscale, encode,
/// hand to the webview. The clock stops in `overlay_painted`.
#[tauri::command]
fn pause_and_capture(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Emitter;
    let t0 = std::time::Instant::now();
    PAUSE_T0.store(now_us(), Ordering::SeqCst);

    let guard = CTX.lock().unwrap();
    let ctx = guard.as_ref().ok_or("no context")?;

    ctx.mpv.set_property("pause", "yes")?;
    let t_pause = t0.elapsed();

    let frame = unsafe {
        snapshot::screenshot_raw(ctx.mpv.raw(), ctx.mpv.command_node_fn(), ctx.mpv.free_node_fn())?
    };
    let t_capture = t0.elapsed();

    let (rgb, w, h) = snapshot::downscale_to_rgb(&frame, 960, 540);
    let t_scale = t0.elapsed();

    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, 78)
        .encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| e.to_string())?;
    let t_encode = t0.elapsed();

    let uri = format!("data:image/jpeg;base64,{}", b64(&jpeg));
    let t_b64 = t0.elapsed();

    println!(
        "  capture {}x{} {} -> {}x{}  jpeg {} KB",
        frame.width, frame.height, frame.format, w, h, jpeg.len() / 1024
    );
    println!(
        "  set-pause {:.1} | screenshot-raw {:.1} | downscale {:.1} | jpeg {:.1} | base64 {:.1} ms",
        t_pause.as_secs_f64() * 1000.0,
        (t_capture - t_pause).as_secs_f64() * 1000.0,
        (t_scale - t_capture).as_secs_f64() * 1000.0,
        (t_encode - t_scale).as_secs_f64() * 1000.0,
        (t_b64 - t_encode).as_secs_f64() * 1000.0,
    );

    app.emit("pause-frame", uri).map_err(|e| e.to_string())?;
    Ok("ok".into())
}

/// Chrome geometry in client pixels. Kept in one place so the HTML and the
/// region cannot disagree about where the hole is.
fn chrome_rect(w: i32, h: i32) -> RECT {
    let bar_h = 190;
    RECT { left: 40, top: h - bar_h - 30, right: w - 40, bottom: h - 30 }
}

/// TEST 1 + 3. Toggle the cutout, exactly as auto-hide would.
#[tauri::command]
fn set_chrome(app: tauri::AppHandle, visible: bool, redraw: bool) -> Result<u128, String> {
    use tauri::Emitter;
    let guard = CTX.lock().unwrap();
    let ctx = guard.as_ref().ok_or("no context")?;
    let child = HWND(ctx.video_child as *mut _);

    let mut rc = RECT::default();
    unsafe { GetClientRect(child, &mut rc).map_err(|e| e.to_string())?; }
    let (w, h) = (rc.right, rc.bottom);

    let micros = unsafe {
        if visible {
            cutout::apply_cutout(child, w, h, &[chrome_rect(w, h)], redraw)
        } else {
            cutout::clear_cutout(child, redraw)
        }
    };
    app.emit("chrome", visible).map_err(|e| e.to_string())?;
    println!("  SetWindowRgn visible={visible} redraw={redraw}  {:.3} ms", micros as f64 / 1000.0);
    Ok(micros)
}

/// TEST 1 setup. The page reports the button's CSS-pixel rect; convert to screen
/// coordinates so the harness can click exactly there. CSS px -> physical px via
/// the window scale factor, then client -> screen.
#[tauri::command]
fn report_button(app: tauri::AppHandle, x: f64, y: f64, w: f64, h: f64) -> Result<String, String> {
    use tauri::Manager;
    let win = app.get_webview_window("main").ok_or("no window")?;
    let scale = win.scale_factor().map_err(|e| e.to_string())?;
    let guard = CTX.lock().unwrap();
    let ctx = guard.as_ref().ok_or("no context")?;
    let top = unsafe { GetParent(HWND(ctx.video_child as *mut _)).map_err(|e| e.to_string())? };
    let mut pt = windows::Win32::Foundation::POINT {
        x: ((x + w / 2.0) * scale) as i32,
        y: ((y + h / 2.0) * scale) as i32,
    };
    unsafe { let _ = ClientToScreen(top, &mut pt); }
    let out = format!("BUTTON_SCREEN_XY {} {}", pt.x, pt.y);
    println!("{out}");
    Ok(out)
}

/// TEST 1. The page reports that a click landed on the button inside the hole.
#[tauri::command]
fn cutout_click(x: f64, y: f64) {
    println!("TEST 1  HIT-TEST: click REACHED the webview through the hole at ({x:.0}, {y:.0})");
}

/// TEST 2. Resize the parent mid-playback, then re-apply the region and confirm
/// the hole tracks the new size.
#[tauri::command]
fn resize_and_reshape(app: tauri::AppHandle, w: i32, h: i32) -> Result<String, String> {
    let guard = CTX.lock().unwrap();
    let ctx = guard.as_ref().ok_or("no context")?;
    let child = HWND(ctx.video_child as *mut _);
    let top = unsafe { GetParent(child).map_err(|e| e.to_string())? };

    unsafe {
        SetWindowPos(top, None, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER)
            .map_err(|e| e.to_string())?;
        let mut rc = RECT::default();
        let _ = GetClientRect(top, &mut rc);
        // Track the parent: resize the video child, then re-cut at the new size.
        let _ = SetWindowPos(child, None, 0, 0, rc.right, rc.bottom, SWP_NOZORDER);
        let hole = chrome_rect(rc.right, rc.bottom);
        cutout::apply_cutout(child, rc.right, rc.bottom, &[hole], false);
        let _ = InvalidateRect(Some(child), None, false);
        drop(guard);
        let _ = app;
        Ok(format!("parent {}x{} -> child {}x{}, hole {},{} {}x{}",
            w, h, rc.right, rc.bottom,
            hole.left, hole.top, hole.right - hole.left, hole.bottom - hole.top))
    }
}

/// The webview has painted the still frame. Stop the clock, then hide the video
/// child — hiding only now means there is never a frame of blank window.
#[tauri::command]
fn overlay_painted() {
    let elapsed_ms = (now_us() - PAUSE_T0.load(Ordering::SeqCst)) as f64 / 1000.0;
    if let Some(ctx) = CTX.lock().unwrap().as_ref() {
        unsafe {
            let _ = ShowWindow(HWND(ctx.video_child as *mut _), SW_HIDE);
        }
    }
    let verdict = if elapsed_ms < 200.0 { "WITHIN" } else { "OVER" };
    println!("PAUSE -> OVERLAY VISIBLE   {elapsed_ms:>7.1} ms   [{verdict} the 200 ms budget]");
}

#[tauri::command]
fn resume(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;
    if let Some(ctx) = CTX.lock().unwrap().as_ref() {
        unsafe {
            let _ = ShowWindow(HWND(ctx.video_child as *mut _), SW_SHOW);
        }
        ctx.mpv.set_property("pause", "no")?;
    }
    app.emit("resume", ()).map_err(|e| e.to_string())?;
    Ok(())
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// The mpv host child needs no behaviour of its own; mpv paints it.
unsafe extern "system" fn video_host_proc(
    hwnd: HWND, msg: u32, wp: windows::Win32::Foundation::WPARAM, lp: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // Test 3 defence. When the region shrinks, Windows would otherwise erase the
    // newly-exposed area with the class brush before mpv repaints — which is
    // exactly the flash that would make auto-hiding chrome strobe. Claiming the
    // erase (return 1) and shipping a NULL class brush means nothing paints
    // there but mpv.
    if msg == WM_ERASEBKGND {
        return windows::Win32::Foundation::LRESULT(1);
    }
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
        .invoke_handler(tauri::generate_handler![pause_and_capture, overlay_painted, resume, set_chrome, cutout_click, resize_and_reshape, report_button])
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
                    // NULL background brush: see video_host_proc. Default would be
                    // COLOR_WINDOW and would flash white on every region change.
                    hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(std::ptr::null_mut()),
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

                // The pause command needs the handle too. Spike-grade sharing:
                // a second Mpv over the same DLL would be a second mpv instance,
                // so instead the event pump gets a raw pointer and CTX owns it.
                let raw_handle = m.raw() as usize;
                let cmd_node = m.command_node_fn();
                let free_node = m.free_node_fn();
                *CTX.lock().unwrap() = Some(PauseCtx {
                    mpv: m,
                    video_child: video_child.0 as isize,
                });
                let _ = (raw_handle, cmd_node, free_node);

                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(2500));

                    println!("=== TEST 4: seam — cutout applied, hold for capture ===");
                    let _ = set_chrome(handle.clone(), true, false);
                    std::thread::sleep(Duration::from_millis(2500));

                    println!("=== TEST 2: resize with region applied ===");
                    match resize_and_reshape(handle.clone(), 1100, 720) {
                        Ok(s) => println!("  {s}"),
                        Err(e) => println!("  resize failed: {e}"),
                    }
                    std::thread::sleep(Duration::from_millis(1800));
                    match resize_and_reshape(handle.clone(), 1400, 880) {
                        Ok(s) => println!("  {s}"),
                        Err(e) => println!("  resize failed: {e}"),
                    }
                    std::thread::sleep(Duration::from_millis(1800));

                    println!("=== TEST 3: flicker — 12 toggles at the auto-hide cadence ===");
                    let mut costs = Vec::new();
                    for i in 0..12 {
                        let visible = i % 2 == 0;
                        if let Ok(us) = set_chrome(handle.clone(), visible, false) {
                            costs.push(us);
                        }
                        std::thread::sleep(Duration::from_millis(420));
                    }
                    if !costs.is_empty() {
                        costs.sort_unstable();
                        println!("  SetWindowRgn cost: min {:.3} / median {:.3} / max {:.3} ms",
                            costs[0] as f64 / 1000.0,
                            costs[costs.len() / 2] as f64 / 1000.0,
                            costs[costs.len() - 1] as f64 / 1000.0);
                    }

                    println!("=== TEST 1: waiting for a click in the cutout ===");
                    let _ = set_chrome(handle.clone(), true, false);
                    std::thread::sleep(Duration::from_millis(6000));
                    println!("--- tests complete ---");
                });

                // Pump mpv events on a worker so the Tauri event loop stays free.
                std::thread::spawn(move || {
                    let mut reported = false;
                    loop {
                        let guard = CTX.lock().unwrap();
                        let Some(ctx) = guard.as_ref() else { break };
                        let (id, text) = ctx.mpv.wait_event(0.05);
                        if let Some(t) = text {
                            println!("  mpv {t}");
                        }
                        if id == MPV_EVENT_PLAYBACK_RESTART && !reported {
                            reported = true;
                            println!(
                                "FIRST FRAME           {:>7.1} ms  (into a Tauri child HWND)",
                                t0.elapsed().as_secs_f64() * 1000.0
                            );
                            println!("  hwdec-current       {:?}",
                                ctx.mpv.get_property("hwdec-current"));
                        }
                        let done = id == MPV_EVENT_SHUTDOWN;
                        drop(guard);
                        if done { break; }
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
