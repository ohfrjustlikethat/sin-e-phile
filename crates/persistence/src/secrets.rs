//! Wrapping a user's own API key before it touches the disk.
//!
//! # What this protects against, and what it does not
//!
//! The key is the **user's own** TMDB key, on their own machine. The realistic risk is
//! not a targeted attacker; it is that `./data/` gets copied. This application is
//! portable by default (ADR-0008), so its data directory is exactly the kind of folder
//! that ends up on a USB stick, in a cloud-synced folder, or attached to a bug report.
//! Migration 0008 already worried about this in as many words — *"the cache is copied
//! with the app folder, and a key in it would travel too"* — and the same argument
//! applies with more force to the key itself.
//!
//! So: **DPAPI**, `CryptProtectData` with `CRYPTPROTECT_UI_FORBIDDEN`. The ciphertext
//! is bound to the Windows user account, so a copied folder yields bytes that will not
//! unwrap anywhere else.
//!
//! It is **not** protection against something already running as that user. Nothing
//! stored on a machine is, and claiming otherwise would be worse than not doing it.
//!
//! # Why failing to unwrap is not an error worth propagating
//!
//! A folder moved to another machine, another Windows account, or restored from a
//! backup will fail to unwrap — correctly. The app already has a designed, supported
//! state for "no key": the §9.4 typographic treatment, which ADR-0027 requires to be
//! *"genuinely beautiful rather than a fallback"*. So an unwrap failure degrades to
//! exactly that and asks the user to re-enter their key. It is a graceful path, not an
//! error path, and treating it as a failure would turn a copied folder into a crash.

use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// A description stored alongside the ciphertext by Windows. Visible to anything that
/// can already unwrap it, so it says what the blob is and nothing sensitive.
const DESCRIPTION: &str = "sin-e-phile profile secret";

/// Wrap a secret for storage.
///
/// Returns `None` if Windows refuses, in which case the caller must not fall back to
/// storing plaintext — the whole point is that this file may travel.
pub fn wrap(plaintext: &str) -> Option<Vec<u8>> {
    let mut input = plaintext.as_bytes().to_vec();
    let mut description: Vec<u16> = DESCRIPTION
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    // SAFETY: both blobs point at live allocations for the duration of the call, and
    // `out_blob.pbData` is written by Windows with memory it owns, which is copied out
    // and then freed with `LocalFree` exactly once below.
    let ok = unsafe {
        CryptProtectData(
            &in_blob,
            description.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            // No interactive prompt. This can run during a background job, where a
            // dialog nobody is present to answer would hang the job rather than fail.
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };

    if ok == 0 || out_blob.pbData.is_null() {
        return None;
    }

    // SAFETY: Windows guarantees `pbData` is valid for `cbData` bytes on success.
    let wrapped =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
    unsafe { LocalFree(out_blob.pbData as HLOCAL) };
    Some(wrapped)
}

/// Unwrap a stored secret.
///
/// `None` means "this machine or this Windows account cannot read it" — a copied data
/// folder, a restored backup, a different user. That is a supported state, not a fault.
pub fn unwrap(wrapped: &[u8]) -> Option<String> {
    if wrapped.is_empty() {
        return None;
    }
    let mut input = wrapped.to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    // SAFETY: as above.
    let ok = unsafe {
        CryptUnprotectData(
            &in_blob,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };

    if ok == 0 || out_blob.pbData.is_null() {
        return None;
    }

    // SAFETY: valid for `cbData` bytes on success.
    let plaintext =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec() };
    unsafe { LocalFree(out_blob.pbData as HLOCAL) };
    String::from_utf8(plaintext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_survives_a_round_trip() {
        let key = "abcdef0123456789abcdef0123456789";
        let wrapped = wrap(key).expect("DPAPI is available on Windows");
        assert_eq!(unwrap(&wrapped).as_deref(), Some(key));
    }

    #[test]
    fn the_wrapped_form_does_not_contain_the_secret() {
        // The entire reason this exists: `data/` gets copied, and a grep of the file
        // must not find the key.
        let key = "abcdef0123456789abcdef0123456789";
        let wrapped = wrap(key).expect("wrap");
        assert!(
            !wrapped.windows(key.len()).any(|w| w == key.as_bytes()),
            "the plaintext key is present in the wrapped bytes"
        );
    }

    #[test]
    fn corrupt_bytes_unwrap_to_nothing_rather_than_panicking() {
        // What a copied folder from another machine looks like from here.
        assert_eq!(unwrap(&[]), None);
        assert_eq!(unwrap(&[0u8; 32]), None);

        let mut wrapped = wrap("a-real-key").expect("wrap");
        let last = wrapped.len() - 1;
        wrapped[last] ^= 0xff;
        assert_eq!(unwrap(&wrapped), None, "a tampered blob must not unwrap");
    }

    #[test]
    fn an_empty_secret_round_trips_rather_than_being_special_cased() {
        // Storing an empty key is a caller error, not this module's business — but it
        // must not silently become "no key stored", which is a different state.
        let wrapped = wrap("").expect("wrap");
        assert_eq!(unwrap(&wrapped).as_deref(), Some(""));
    }
}
