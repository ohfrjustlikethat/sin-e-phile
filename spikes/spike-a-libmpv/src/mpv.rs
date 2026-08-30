//! Minimal libmpv FFI, loaded dynamically.
//!
//! SPIKE CODE. Throwaway (SPEC.md Phase 1). Not the Phase 8 design.
//!
//! Why dynamic loading rather than linking:
//!   The mpv dev build ships `libmpv.dll.a`, a MinGW import library. MSVC cannot
//!   link that, so a real build would need `dumpbin /exports` -> .def -> `lib
//!   /def:` to synthesise `mpv.lib`. For a spike that is noise, and dynamic
//!   loading also answers part of R1's question directly: how much toolchain
//!   does this actually require? Answer so far: none beyond the DLL.
//!
//! Only the ~10 entry points the spike needs are declared.

#![allow(non_camel_case_types)]

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

pub enum mpv_handle {}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct mpv_event {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

// From client.h. Only the ones the spike reacts to.
pub const MPV_EVENT_NONE: c_int = 0;
pub const MPV_EVENT_SHUTDOWN: c_int = 1;
pub const MPV_EVENT_LOG_MESSAGE: c_int = 2;
pub const MPV_EVENT_FILE_LOADED: c_int = 8;
pub const MPV_EVENT_VIDEO_RECONFIG: c_int = 17;
pub const MPV_EVENT_PLAYBACK_RESTART: c_int = 21;

#[repr(C)]
pub struct mpv_event_log_message {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    pub log_level: c_int,
}

type FnCreate = unsafe extern "C" fn() -> *mut mpv_handle;
type FnInitialize = unsafe extern "C" fn(*mut mpv_handle) -> c_int;
type FnTerminateDestroy = unsafe extern "C" fn(*mut mpv_handle);
type FnSetOptionString = unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int;
type FnSetPropertyString = unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int;
type FnGetPropertyString = unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> *mut c_char;
type FnCommand = unsafe extern "C" fn(*mut mpv_handle, *mut *const c_char) -> c_int;
type FnWaitEvent = unsafe extern "C" fn(*mut mpv_handle, f64) -> *mut mpv_event;
type FnErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type FnRequestLogMessages = unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> c_int;
type FnFree = unsafe extern "C" fn(*mut c_void);

pub struct Mpv {
    handle: *mut mpv_handle,
    // Declared last so it drops last: unloading the DLL before destroying the
    // handle would call freed code.
    _lib: Library,
    initialize: RawFn<FnInitialize>,
    terminate_destroy: RawFn<FnTerminateDestroy>,
    set_option_string: RawFn<FnSetOptionString>,
    set_property_string: RawFn<FnSetPropertyString>,
    get_property_string: RawFn<FnGetPropertyString>,
    command: RawFn<FnCommand>,
    wait_event: RawFn<FnWaitEvent>,
    error_string: RawFn<FnErrorString>,
    request_log_messages: RawFn<FnRequestLogMessages>,
    free: RawFn<FnFree>,
}

/// A resolved symbol, detached from the `Library` borrow.
///
/// `Symbol<'lib, T>` borrows the `Library`, which cannot be stored beside it in
/// the same struct. `into_raw` drops the lifetime; keeping `_lib` alive for the
/// struct's whole life is what makes that sound.
struct RawFn<T>(T);

unsafe fn resolve<T: Copy>(lib: &Library, name: &[u8]) -> Result<RawFn<T>, String> {
    let sym: Symbol<T> = lib
        .get(name)
        .map_err(|e| format!("symbol {}: {e}", String::from_utf8_lossy(name)))?;
    Ok(RawFn(*sym))
}

impl Mpv {
    pub fn load(dll: &Path) -> Result<Self, String> {
        unsafe {
            let lib = Library::new(dll).map_err(|e| format!("load {}: {e}", dll.display()))?;

            let create: RawFn<FnCreate> = resolve(&lib, b"mpv_create\0")?;
            let initialize = resolve(&lib, b"mpv_initialize\0")?;
            let terminate_destroy = resolve(&lib, b"mpv_terminate_destroy\0")?;
            let set_option_string = resolve(&lib, b"mpv_set_option_string\0")?;
            let set_property_string = resolve(&lib, b"mpv_set_property_string\0")?;
            let get_property_string = resolve(&lib, b"mpv_get_property_string\0")?;
            let command = resolve(&lib, b"mpv_command\0")?;
            let wait_event = resolve(&lib, b"mpv_wait_event\0")?;
            let error_string = resolve(&lib, b"mpv_error_string\0")?;
            let request_log_messages = resolve(&lib, b"mpv_request_log_messages\0")?;
            let free = resolve(&lib, b"mpv_free\0")?;

            let handle = (create.0)();
            if handle.is_null() {
                return Err("mpv_create returned null".into());
            }

            Ok(Self {
                handle,
                _lib: lib,
                initialize,
                terminate_destroy,
                set_option_string,
                set_property_string,
                get_property_string,
                command,
                wait_event,
                error_string,
                request_log_messages,
                free,
            })
        }
    }

    fn check(&self, code: c_int, what: &str) -> Result<(), String> {
        if code >= 0 {
            return Ok(());
        }
        let msg = unsafe { CStr::from_ptr((self.error_string.0)(code)) }
            .to_string_lossy()
            .into_owned();
        Err(format!("{what}: {msg} ({code})"))
    }

    pub fn set_option(&self, key: &str, value: &str) -> Result<(), String> {
        let (k, v) = (CString::new(key).unwrap(), CString::new(value).unwrap());
        let rc = unsafe { (self.set_option_string.0)(self.handle, k.as_ptr(), v.as_ptr()) };
        self.check(rc, &format!("set_option {key}={value}"))
    }

    pub fn set_property(&self, key: &str, value: &str) -> Result<(), String> {
        let (k, v) = (CString::new(key).unwrap(), CString::new(value).unwrap());
        let rc = unsafe { (self.set_property_string.0)(self.handle, k.as_ptr(), v.as_ptr()) };
        self.check(rc, &format!("set_property {key}={value}"))
    }

    pub fn get_property(&self, key: &str) -> Option<String> {
        let k = CString::new(key).unwrap();
        unsafe {
            let raw = (self.get_property_string.0)(self.handle, k.as_ptr());
            if raw.is_null() {
                return None;
            }
            let out = CStr::from_ptr(raw).to_string_lossy().into_owned();
            (self.free.0)(raw as *mut c_void); // mpv owns it; we must hand it back
            Some(out)
        }
    }

    pub fn request_log_messages(&self, level: &str) -> Result<(), String> {
        let l = CString::new(level).unwrap();
        let rc = unsafe { (self.request_log_messages.0)(self.handle, l.as_ptr()) };
        self.check(rc, "request_log_messages")
    }

    pub fn initialize(&self) -> Result<(), String> {
        let rc = unsafe { (self.initialize.0)(self.handle) };
        self.check(rc, "mpv_initialize")
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null()); // mpv_command takes a NULL-terminated array
        let rc = unsafe { (self.command.0)(self.handle, ptrs.as_mut_ptr()) };
        self.check(rc, &format!("command {args:?}"))
    }

    /// Block up to `timeout` seconds for the next event.
    pub fn wait_event(&self, timeout: f64) -> (c_int, Option<String>) {
        unsafe {
            let ev = (self.wait_event.0)(self.handle, timeout);
            if ev.is_null() {
                return (MPV_EVENT_NONE, None);
            }
            let id = (*ev).event_id;
            let text = if id == MPV_EVENT_LOG_MESSAGE && !(*ev).data.is_null() {
                let m = (*ev).data as *const mpv_event_log_message;
                Some(format!(
                    "[{}] {}",
                    CStr::from_ptr((*m).prefix).to_string_lossy(),
                    CStr::from_ptr((*m).text).to_string_lossy().trim_end()
                ))
            } else {
                None
            };
            (id, text)
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.terminate_destroy.0)(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}
