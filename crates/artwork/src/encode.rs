//! Decoding what TMDB serves, and re-encoding it smaller.
//!
//! # Why lossy WebP, and why not the `image` crate's own
//!
//! `image` supports WebP **decoding** and **lossless** encoding. Lossless WebP on a
//! photograph is routinely *larger than the JPEG it came from*, so using it here would
//! make the cache worse while appearing to satisfy the requirement. Lossy WebP needs
//! libwebp, which is what the `webp` crate wraps — and this project already requires
//! the C toolchain for FFmpeg and librqbit, so it costs nothing new.
//!
//! The saving is measured on real photographs rather than asserted; see
//! `tests/real_images.rs` and `docs/eval-results.md`.
//!
//! # Quality
//!
//! 80 is the usual default and is what this uses. Posters are viewed at a few hundred
//! pixels wide in a rail; the artefacts that appear below ~70 are visible at that size
//! and the bytes saved above ~85 are not worth the loss.

use image::GenericImageView;

/// WebP quality, 0–100.
pub const QUALITY: f32 = 80.0;

/// Blurhash detail. 4×3 suits a poster's aspect ratio and is the common choice.
pub const BLUR_X: usize = 4;
pub const BLUR_Y: usize = 3;

/// The blurhash is computed from a downscale, not the full image.
///
/// The transform is O(pixels × components), so running it over a 2000-pixel-tall
/// poster costs a hundred times what it costs over a thumbnail and produces the same
/// hash to within rounding — the whole point is that only the lowest frequencies
/// survive. 64px is comfortably above the 4×3 basis it is projected onto.
const BLUR_SOURCE_MAX: u32 = 64;

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("this is not an image we can read: {0}")]
    Decode(#[from] image::ImageError),
    #[error("libwebp refused to encode a {width}x{height} image")]
    Encode { width: u32, height: u32 },
    #[error("blurhash: {0}")]
    Blurhash(#[from] crate::blurhash::BlurhashError),
}

/// An image ready to be stored.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// Lossy WebP.
    pub webp: Vec<u8>,
    /// ~30 characters that render as a blurred version of the image.
    pub blurhash: String,
    pub width: u32,
    pub height: u32,
    /// Size of the bytes that arrived, so the saving can be reported rather than
    /// assumed.
    pub original_bytes: usize,
}

impl Prepared {
    /// How much smaller the stored form is, as a fraction. Negative if WebP lost.
    pub fn saving(&self) -> f64 {
        if self.original_bytes == 0 {
            return 0.0;
        }
        1.0 - (self.webp.len() as f64 / self.original_bytes as f64)
    }
}

/// Decode an image, compute its blurhash, and re-encode it as lossy WebP.
pub fn prepare(bytes: &[u8]) -> Result<Prepared, EncodeError> {
    let decoded = image::load_from_memory(bytes)?;
    let (width, height) = decoded.dimensions();

    // Blurhash from a thumbnail — see BLUR_SOURCE_MAX.
    let thumb = decoded
        .thumbnail(BLUR_SOURCE_MAX, BLUR_SOURCE_MAX)
        .to_rgb8();
    let hash = crate::blurhash::encode(
        thumb.as_raw(),
        thumb.width() as usize,
        thumb.height() as usize,
        BLUR_X,
        BLUR_Y,
    )?;

    // RGB8 rather than RGBA8: TMDB posters are opaque, and carrying an alpha channel
    // through libwebp costs a third more bytes for nothing.
    let rgb = decoded.to_rgb8();
    let encoder = webp::Encoder::from_rgb(rgb.as_raw(), width, height);
    let webp = encoder.encode(QUALITY);

    if webp.is_empty() {
        return Err(EncodeError::Encode { width, height });
    }

    Ok(Prepared {
        webp: webp.to_vec(),
        blurhash: hash,
        width,
        height,
        original_bytes: bytes.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small photographic-ish JPEG, built rather than committed.
    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut buf = image::RgbImage::new(width, height);
        for (x, y, pixel) in buf.enumerate_pixels_mut() {
            *pixel = image::Rgb([
                (x * 255 / width.max(1)) as u8,
                (y * 255 / height.max(1)) as u8,
                ((x + y) % 256) as u8,
            ]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(buf)
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .expect("write jpeg");
        out.into_inner()
    }

    #[test]
    fn a_jpeg_becomes_webp_with_a_blurhash() {
        let source = jpeg(200, 300);
        let prepared = prepare(&source).expect("prepare");

        assert_eq!((prepared.width, prepared.height), (200, 300));
        assert_eq!(prepared.original_bytes, source.len());
        assert_eq!(prepared.blurhash.len(), 28, "4x3 components");
        // RIFF....WEBP is the container's magic.
        assert_eq!(&prepared.webp[0..4], b"RIFF");
        assert_eq!(&prepared.webp[8..12], b"WEBP");
    }

    #[test]
    fn the_blurhash_survives_the_downscale() {
        // The hash is computed from a 64px thumbnail. If that were wrong, a large and
        // a small version of the same picture would disagree.
        let large = prepare(&jpeg(400, 600)).expect("large");
        let small = prepare(&jpeg(200, 300)).expect("small");
        // Not identical — they are different images at different scales — but the
        // dominant colour must be close.
        let a = crate::blurhash::average_colour(&large.blurhash).expect("a");
        let b = crate::blurhash::average_colour(&small.blurhash).expect("b");
        for channel in 0..3 {
            let delta = (a[channel] as i16 - b[channel] as i16).abs();
            assert!(
                delta <= 12,
                "channel {channel} differs by {delta}: {a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn a_tiny_image_still_works() {
        // A 1x1 favicon-sized thing must not divide by zero or fail the blurhash.
        let prepared = prepare(&jpeg(1, 1)).expect("prepare");
        assert_eq!((prepared.width, prepared.height), (1, 1));
        assert!(!prepared.blurhash.is_empty());
    }

    #[test]
    fn bytes_that_are_not_an_image_are_an_error_not_a_panic() {
        // A 404 page, a truncated download, an HTML error body served as an image.
        assert!(matches!(
            prepare(b"<!doctype html><title>404</title>"),
            Err(EncodeError::Decode(_))
        ));
        assert!(prepare(&[]).is_err());
        // A JPEG header with nothing behind it.
        assert!(prepare(&[0xff, 0xd8, 0xff, 0xe0]).is_err());
    }
}
