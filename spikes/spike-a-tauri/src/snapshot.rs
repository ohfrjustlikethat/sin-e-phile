//! `screenshot-raw` via mpv's node API, downscaled and JPEG-encoded.
//!
//! SPIKE CODE. Throwaway (SPEC.md Phase 1, risk R1, blocker B2 option 1).
//!
//! Why `screenshot-raw window` rather than `screenshot-to-file`:
//!   - No disk round trip, which the §11 200 ms budget cannot spare.
//!   - `window` captures at WINDOW resolution, not source resolution. A 4K source
//!     in a 1280x800 window yields a 1280x800 capture, so the cost of this path
//!     does not scale with source resolution — which is the whole reason it can
//!     hold a latency budget.
//!
//! The frame is then boxed down to ~960x540 and JPEG'd, because the overlay dims
//! and blurs it anyway (SPEC.md §11), so full resolution buys nothing.

use std::ffi::{c_char, c_int, c_void, CStr, CString};

// mpv_format, from client.h. Only what the screenshot-raw result uses.
const MPV_FORMAT_STRING: c_int = 1;
const MPV_FORMAT_INT64: c_int = 4;
const MPV_FORMAT_NODE_ARRAY: c_int = 7;
const MPV_FORMAT_NODE_MAP: c_int = 8;
const MPV_FORMAT_BYTE_ARRAY: c_int = 9;

#[repr(C)]
pub struct MpvNode {
    pub u: MpvNodeUnion,
    pub format: c_int,
}

#[repr(C)]
pub union MpvNodeUnion {
    pub string: *mut c_char,
    pub flag: c_int,
    pub int64: i64,
    pub double_: f64,
    pub list: *mut MpvNodeList,
    pub ba: *mut MpvByteArray,
}

#[repr(C)]
pub struct MpvNodeList {
    pub num: c_int,
    pub values: *mut MpvNode,
    pub keys: *mut *mut c_char,
}

#[repr(C)]
pub struct MpvByteArray {
    pub data: *mut c_void,
    pub size: usize,
}

pub type FnCommandNode =
    unsafe extern "C" fn(*mut crate::mpv::mpv_handle, *mut MpvNode, *mut MpvNode) -> c_int;
pub type FnFreeNodeContents = unsafe extern "C" fn(*mut MpvNode);

pub struct RawFrame {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub format: String,
    pub data: Vec<u8>,
}

/// Call `screenshot-raw window` and copy the result out of mpv's memory.
///
/// SAFETY: caller guarantees `handle` is live and that only this thread drives it.
pub unsafe fn screenshot_raw(
    handle: *mut crate::mpv::mpv_handle,
    command_node: FnCommandNode,
    free_node: FnFreeNodeContents,
) -> Result<RawFrame, String> {
    // Build the argument array: ["screenshot-raw", "window"].
    let a0 = CString::new("screenshot-raw").unwrap();
    let a1 = CString::new("window").unwrap();
    let mut argv = [
        MpvNode { u: MpvNodeUnion { string: a0.as_ptr() as *mut c_char }, format: MPV_FORMAT_STRING },
        MpvNode { u: MpvNodeUnion { string: a1.as_ptr() as *mut c_char }, format: MPV_FORMAT_STRING },
    ];
    let mut list = MpvNodeList { num: 2, values: argv.as_mut_ptr(), keys: std::ptr::null_mut() };
    let mut args = MpvNode {
        u: MpvNodeUnion { list: &mut list },
        format: MPV_FORMAT_NODE_ARRAY,
    };
    let mut result = MpvNode { u: MpvNodeUnion { int64: 0 }, format: 0 };

    let rc = command_node(handle, &mut args, &mut result);
    if rc < 0 {
        return Err(format!("screenshot-raw failed: {rc}"));
    }
    if result.format != MPV_FORMAT_NODE_MAP {
        free_node(&mut result);
        return Err(format!("expected a node map, got format {}", result.format));
    }

    let (mut w, mut h, mut stride) = (0usize, 0usize, 0usize);
    let mut fmt = String::new();
    let mut data: Vec<u8> = Vec::new();

    let map = &*result.u.list;
    for i in 0..map.num as isize {
        let key = CStr::from_ptr(*map.keys.offset(i)).to_string_lossy().into_owned();
        let node = &*map.values.offset(i);
        match (key.as_str(), node.format) {
            ("w", MPV_FORMAT_INT64) => w = node.u.int64 as usize,
            ("h", MPV_FORMAT_INT64) => h = node.u.int64 as usize,
            ("stride", MPV_FORMAT_INT64) => stride = node.u.int64 as usize,
            ("format", MPV_FORMAT_STRING) => {
                fmt = CStr::from_ptr(node.u.string).to_string_lossy().into_owned()
            }
            ("data", MPV_FORMAT_BYTE_ARRAY) => {
                let ba = &*node.u.ba;
                data = std::slice::from_raw_parts(ba.data as *const u8, ba.size).to_vec();
            }
            _ => {}
        }
    }

    free_node(&mut result); // mpv owns the buffers; hand them back promptly

    if w == 0 || h == 0 || data.is_empty() {
        return Err(format!("incomplete screenshot: {w}x{h}, {} bytes", data.len()));
    }
    Ok(RawFrame { width: w, height: h, stride, format: fmt, data })
}

/// Box-filter downscale to fit within `max_w` x `max_h`, converting to RGB.
///
/// mpv hands back `bgr0` (or `rgb0`): 4 bytes per pixel with an ignored 4th.
/// Box-averaging rather than nearest-neighbour because the result gets blurred
/// anyway and averaging is barely more expensive at these sizes.
pub fn downscale_to_rgb(frame: &RawFrame, max_w: usize, max_h: usize) -> (Vec<u8>, usize, usize) {
    let scale = ((frame.width as f64 / max_w as f64).max(frame.height as f64 / max_h as f64)).max(1.0);
    let out_w = ((frame.width as f64 / scale).round() as usize).max(1);
    let out_h = ((frame.height as f64 / scale).round() as usize).max(1);

    let bgr = frame.format.starts_with("bgr");
    let mut out = vec![0u8; out_w * out_h * 3];

    let box_w = (frame.width as f64 / out_w as f64).max(1.0) as usize;
    let box_h = (frame.height as f64 / out_h as f64).max(1.0) as usize;

    for oy in 0..out_h {
        let sy0 = oy * frame.height / out_h;
        for ox in 0..out_w {
            let sx0 = ox * frame.width / out_w;
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for dy in 0..box_h {
                let sy = (sy0 + dy).min(frame.height - 1);
                let row = sy * frame.stride;
                for dx in 0..box_w {
                    let sx = (sx0 + dx).min(frame.width - 1);
                    let p = row + sx * 4;
                    if p + 2 >= frame.data.len() {
                        continue;
                    }
                    let (c0, c1, c2) = (frame.data[p], frame.data[p + 1], frame.data[p + 2]);
                    if bgr {
                        b += c0 as u32; g += c1 as u32; r += c2 as u32;
                    } else {
                        r += c0 as u32; g += c1 as u32; b += c2 as u32;
                    }
                    n += 1;
                }
            }
            if n > 0 {
                let o = (oy * out_w + ox) * 3;
                out[o] = (r / n) as u8;
                out[o + 1] = (g / n) as u8;
                out[o + 2] = (b / n) as u8;
            }
        }
    }
    (out, out_w, out_h)
}
