//! The pause overlay's still frame, as the page receives it (ADR-0021).

/// A JPEG as a `data:` URI the page can put straight into an `<img>`.
pub fn data_uri(jpeg: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", base64(jpeg))
}

/// A complete JPEG ends with the EOI marker. mpv writes screenshots asynchronously, so a
/// file that exists may still be half-written.
pub fn is_complete_jpeg(bytes: &[u8]) -> bool {
    bytes.len() > 4 && bytes[..2] == [0xFF, 0xD8] && bytes[bytes.len() - 2..] == [0xFF, 0xD9]
}

/// Standard base64 with padding. A dozen lines rather than a dependency.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        // RFC 4648 §10.
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(raw.as_bytes()), enc, "{raw}");
        }
    }

    #[test]
    fn a_half_written_jpeg_is_not_complete() {
        assert!(is_complete_jpeg(&[0xFF, 0xD8, 1, 2, 0xFF, 0xD9]));
        assert!(!is_complete_jpeg(&[0xFF, 0xD8, 1, 2, 3]));
        assert!(!is_complete_jpeg(&[]));
    }
}
