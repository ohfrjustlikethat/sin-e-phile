//! A profile's picture (Phase 16, the author's brief item 11): any picture file, cut to a
//! square automatically, adjustable by the person (drag and zoom), kept small.
//!
//! The page never sees the original. It is shown a preview no larger than
//! [`PREVIEW_MAX`] on its longer side; the person's choice comes back as a [`Crop`] in
//! fractions of the picture, which is cut from the original and kept as a [`SIZE`]-pixel
//! WebP. Any format `image` reads: JPEG, PNG, WebP, GIF (its first frame), BMP, TIFF, ICO.

use image::{imageops::FilterType, DynamicImage, GenericImageView};

use crate::encode::EncodeError;

/// The kept avatar's side, in pixels: sharp at the profile picker's 112 px at 200%.
pub const SIZE: u32 = 256;
/// The preview's longer side, in pixels.
pub const PREVIEW_MAX: u32 = 1024;
/// A picture file larger than this is not a profile picture.
pub const MAX_FILE_BYTES: u64 = 40 * 1024 * 1024;

/// A square, in fractions of the picture: `x` and `side` of its width, `y` of its height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub side: f64,
}

/// What the page shows while the crop is chosen.
#[derive(Debug, Clone)]
pub struct Preview {
    /// Lossy WebP, at most [`PREVIEW_MAX`] on its longer side.
    pub webp: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// The square chosen automatically.
    pub crop: Crop,
}

/// The automatic square: the largest that fits, centred across; down a portrait, set a
/// little high, where a face usually is.
pub fn auto_crop(width: u32, height: u32) -> Crop {
    let (w, h) = (f64::from(width.max(1)), f64::from(height.max(1)));
    let side = w.min(h);
    Crop {
        x: (w - side) / 2.0 / w,
        y: (h - side) * 0.3 / h,
        side: side / w,
    }
}

fn decode(bytes: &[u8]) -> Result<DynamicImage, EncodeError> {
    Ok(image::load_from_memory(bytes)?)
}

fn webp(img: &DynamicImage, quality: f32) -> Result<Vec<u8>, EncodeError> {
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    let encoded = webp::Encoder::from_rgb(&rgb, width, height).encode(quality);
    if encoded.is_empty() {
        return Err(EncodeError::Encode { width, height });
    }
    Ok(encoded.to_vec())
}

/// Decode a picture and make its preview, with the automatic crop.
pub fn preview(bytes: &[u8]) -> Result<Preview, EncodeError> {
    let img = decode(bytes)?;
    let small = if img.width().max(img.height()) > PREVIEW_MAX {
        img.resize(PREVIEW_MAX, PREVIEW_MAX, FilterType::Triangle)
    } else {
        img
    };
    let (width, height) = small.dimensions();
    Ok(Preview {
        webp: webp(&small, 85.0)?,
        width,
        height,
        crop: auto_crop(width, height),
    })
}

/// Cut `crop` from the picture and make the avatar: a [`SIZE`]-pixel square WebP. A crop
/// reaching past an edge is pulled back inside it.
pub fn avatar(bytes: &[u8], crop: Crop) -> Result<Vec<u8>, EncodeError> {
    let img = decode(bytes)?;
    let (w, h) = img.dimensions();
    let largest = w.min(h);
    let side = ((crop.side.clamp(0.0, 1.0) * f64::from(w)).round() as u32).clamp(1, largest);
    let x = ((crop.x.max(0.0) * f64::from(w)).round() as u32).min(w - side);
    let y = ((crop.y.max(0.0) * f64::from(h)).round() as u32).min(h - side);
    let square = img
        .crop_imm(x, y, side, side)
        .resize_exact(SIZE, SIZE, FilterType::Lanczos3);
    webp(&square, 88.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
        let mut buf = image::RgbImage::new(width, height);
        for (x, y, p) in buf.enumerate_pixels_mut() {
            *p = image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(buf)
            .write_to(&mut out, format)
            .expect("encode");
        out.into_inner()
    }

    #[test]
    fn the_automatic_square_is_the_largest_centred_and_high_in_a_portrait() {
        let landscape = auto_crop(400, 200);
        assert_eq!(
            landscape,
            Crop {
                x: 0.25,
                y: 0.0,
                side: 0.5
            }
        );
        let portrait = auto_crop(200, 400);
        assert_eq!(portrait.x, 0.0);
        assert_eq!(portrait.side, 1.0);
        assert!(
            (portrait.y - 0.15).abs() < 1e-9,
            "a little high: {}",
            portrait.y
        );
    }

    #[test]
    fn any_common_format_becomes_a_square_avatar() {
        for format in [
            image::ImageFormat::Png,
            image::ImageFormat::Jpeg,
            image::ImageFormat::Bmp,
            image::ImageFormat::Gif,
            image::ImageFormat::Tiff,
        ] {
            let bytes = picture(300, 500, format);
            let p = preview(&bytes).unwrap_or_else(|e| panic!("{format:?}: {e}"));
            assert_eq!((p.width, p.height), (300, 500));
            let made = avatar(&bytes, p.crop).unwrap();
            let back = image::load_from_memory(&made).unwrap();
            assert_eq!(back.dimensions(), (SIZE, SIZE), "{format:?}");
        }
    }

    #[test]
    fn a_large_picture_is_previewed_small_and_a_crop_past_the_edge_is_pulled_in() {
        let bytes = picture(3000, 2000, image::ImageFormat::Png);
        let p = preview(&bytes).unwrap();
        assert_eq!((p.width, p.height), (1024, 683));
        let wild = Crop {
            x: 0.9,
            y: 0.9,
            side: 0.8,
        };
        let made = avatar(&bytes, wild).unwrap();
        assert_eq!(
            image::load_from_memory(&made).unwrap().dimensions(),
            (SIZE, SIZE)
        );
    }

    #[test]
    fn something_that_is_not_a_picture_is_refused() {
        assert!(preview(b"not a picture").is_err());
    }
}
