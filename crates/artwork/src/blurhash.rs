//! Blurhash: a whole poster in about thirty characters.
//!
//! A placeholder that is a *blurred version of the actual image* rather than a grey
//! rectangle. `SPEC.md` §9.4 is emphatic that an absent poster must never look like a
//! failure, and a rail of grey boxes filling in one by one is exactly that.
//!
//! # How it works
//!
//! The image is projected onto a small basis of two-dimensional cosine functions —
//! the same idea as the DCT at the heart of JPEG, but kept to a handful of components
//! instead of sixty-four per block. Each component is one number per colour channel
//! saying "how much of this wave is in the picture". Four by three components is
//! enough to reconstruct something recognisably the right shape and colour, and packs
//! into ~30 characters of base-83.
//!
//! Written out here rather than pulled from a crate. It is about a hundred lines, the
//! algorithm is the interesting part of this subtask, and a dependency would hide it.
//!
//! # The sRGB detail that is easy to get wrong
//!
//! Averaging sRGB values directly is wrong: sRGB is stored gamma-encoded, so the
//! numeric midpoint of black and white is not the perceptual midpoint. Every pixel is
//! linearised before the transform and re-encoded afterwards. Skipping that step
//! produces placeholders that are noticeably too dark, which is the classic bug in
//! reimplementations of this algorithm.

/// Base-83 alphabet, in the order the format defines.
const ALPHABET: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlurhashError {
    #[error("components must be 1..=9 in each direction, got {x}x{y}")]
    Components { x: usize, y: usize },
    #[error("the image is empty")]
    Empty,
    #[error("expected {expected} bytes of RGB, got {actual}")]
    Size { expected: usize, actual: usize },
}

/// sRGB byte to linear light.
fn srgb_to_linear(value: u8) -> f32 {
    let v = value as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light back to an sRGB byte.
fn linear_to_srgb(value: f32) -> u8 {
    let v = value.clamp(0.0, 1.0);
    let encoded = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5) as u8
}

fn push_base83(out: &mut String, value: u32, length: usize) {
    for i in 1..=length {
        let digit = (value / 83u32.pow((length - i) as u32)) % 83;
        out.push(ALPHABET[digit as usize] as char);
    }
}

/// Compress a linear value into the format's 9-bit-ish quantisation.
fn quantise_ac(value: f32, maximum: f32) -> u32 {
    let signed = (value / maximum).abs().powf(0.5) * value.signum();
    (((signed * 9.0) + 9.5).floor() as i32).clamp(0, 18) as u32
}

/// Encode RGB8 pixels as a blurhash.
///
/// `components_x` and `components_y` control detail; 4×3 is the usual choice for a
/// poster and is what this project uses.
pub fn encode(
    rgb: &[u8],
    width: usize,
    height: usize,
    components_x: usize,
    components_y: usize,
) -> Result<String, BlurhashError> {
    if !(1..=9).contains(&components_x) || !(1..=9).contains(&components_y) {
        return Err(BlurhashError::Components {
            x: components_x,
            y: components_y,
        });
    }
    if width == 0 || height == 0 {
        return Err(BlurhashError::Empty);
    }
    let expected = width * height * 3;
    if rgb.len() != expected {
        return Err(BlurhashError::Size {
            expected,
            actual: rgb.len(),
        });
    }

    // One [r, g, b] factor per basis function.
    let mut factors: Vec<[f32; 3]> = Vec::with_capacity(components_x * components_y);
    for y in 0..components_y {
        for x in 0..components_x {
            // The DC term (0,0) is a plain average; every AC term is scaled by 2.
            let normalisation = if x == 0 && y == 0 { 1.0 } else { 2.0 };
            let mut factor = [0.0f32; 3];

            for py in 0..height {
                for px in 0..width {
                    let basis = normalisation
                        * (std::f32::consts::PI * x as f32 * px as f32 / width as f32).cos()
                        * (std::f32::consts::PI * y as f32 * py as f32 / height as f32).cos();
                    let i = (py * width + px) * 3;
                    factor[0] += basis * srgb_to_linear(rgb[i]);
                    factor[1] += basis * srgb_to_linear(rgb[i + 1]);
                    factor[2] += basis * srgb_to_linear(rgb[i + 2]);
                }
            }

            let scale = 1.0 / (width * height) as f32;
            factors.push([factor[0] * scale, factor[1] * scale, factor[2] * scale]);
        }
    }

    let (dc, ac) = factors.split_first().expect("at least the DC term");

    let mut hash = String::new();
    let size_flag = ((components_x - 1) + (components_y - 1) * 9) as u32;
    push_base83(&mut hash, size_flag, 1);

    // The maximum AC value, quantised, so the rest can be stored relative to it.
    let actual_max = ac
        .iter()
        .flat_map(|f| f.iter())
        .fold(0.0f32, |m, v| m.max(v.abs()));
    let (quantised_max, maximum) = if ac.is_empty() {
        (0, 1.0)
    } else {
        let q = (((actual_max * 166.0 - 0.5).floor() as i32).clamp(0, 82)) as u32;
        (q, (q + 1) as f32 / 166.0)
    };
    push_base83(&mut hash, quantised_max, 1);

    // DC: three 8-bit sRGB channels packed into one 24-bit value.
    let dc_value = ((linear_to_srgb(dc[0]) as u32) << 16)
        | ((linear_to_srgb(dc[1]) as u32) << 8)
        | linear_to_srgb(dc[2]) as u32;
    push_base83(&mut hash, dc_value, 4);

    for factor in ac {
        let value = quantise_ac(factor[0], maximum) * 19 * 19
            + quantise_ac(factor[1], maximum) * 19
            + quantise_ac(factor[2], maximum);
        push_base83(&mut hash, value, 2);
    }

    Ok(hash)
}

/// The average colour of a blurhash, without decoding it.
///
/// The DC term is the whole of it, which is why a blurhash doubles as a "dominant
/// colour" for a background wash at no extra cost.
pub fn average_colour(hash: &str) -> Option<[u8; 3]> {
    // Indexed as BYTES, never sliced as a string. `&hash[2..6]` panics outright if a
    // multi-byte character straddles either boundary, and this is handed values that
    // came out of a database — so one corrupt row would take the process with it.
    let bytes = hash.as_bytes();
    if bytes.len() < 6 {
        return None;
    }
    let mut value: u32 = 0;
    for c in &bytes[2..6] {
        let digit = ALPHABET.iter().position(|a| a == c)?;
        value = value.checked_mul(83)?.checked_add(digit as u32)?;
    }
    Some([
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: usize, height: usize, colour: [u8; 3]) -> Vec<u8> {
        colour
            .iter()
            .copied()
            .cycle()
            .take(width * height * 3)
            .collect()
    }

    #[test]
    fn a_solid_colour_round_trips_through_the_dc_term() {
        // With no variation there is nothing for the AC terms to carry, so the average
        // colour must come back exactly.
        for colour in [[255u8, 0, 0], [0, 128, 64], [255, 255, 255], [0, 0, 0]] {
            let hash = encode(&solid(8, 8, colour), 8, 8, 4, 3).expect("encode");
            assert_eq!(
                average_colour(&hash),
                Some(colour),
                "solid {colour:?} did not survive"
            );
        }
    }

    #[test]
    fn the_average_is_computed_in_linear_light_not_in_srgb() {
        // THE classic bug in reimplementations. Half black, half white: averaging the
        // stored sRGB bytes gives 128, but the correct answer in linear light is ~188.
        // A placeholder built the wrong way is visibly too dark.
        let mut rgb = Vec::new();
        for y in 0..8 {
            for _ in 0..8 {
                let v = if y < 4 { 0u8 } else { 255 };
                rgb.extend_from_slice(&[v, v, v]);
            }
        }
        let hash = encode(&rgb, 8, 8, 4, 3).expect("encode");
        let average = average_colour(&hash).expect("dc");
        assert!(
            average[0] > 175 && average[0] < 200,
            "expected ~188 in linear light, got {average:?} — 128 means sRGB was averaged directly"
        );
    }

    #[test]
    fn the_hash_is_the_length_the_format_specifies() {
        // 1 size flag + 1 max + 4 DC + 2 per AC term.
        let hash = encode(&solid(4, 4, [10, 20, 30]), 4, 4, 4, 3).expect("encode");
        assert_eq!(hash.len(), 1 + 1 + 4 + 2 * (4 * 3 - 1));
        assert_eq!(hash.len(), 28);

        let smaller = encode(&solid(4, 4, [10, 20, 30]), 4, 4, 1, 1).expect("encode");
        assert_eq!(smaller.len(), 6, "one component is just the DC term");
    }

    #[test]
    fn every_character_is_in_the_base83_alphabet() {
        let mut rgb = Vec::new();
        for y in 0..16 {
            for x in 0..16 {
                rgb.extend_from_slice(&[(x * 16) as u8, (y * 16) as u8, 128]);
            }
        }
        let hash = encode(&rgb, 16, 16, 4, 3).expect("encode");
        for c in hash.bytes() {
            assert!(
                ALPHABET.contains(&c),
                "{:?} is not a base-83 digit",
                c as char
            );
        }
    }

    #[test]
    fn a_gradient_and_a_solid_do_not_produce_the_same_hash() {
        // If they did, the AC terms would be doing nothing and this would be an
        // expensive way to store one colour.
        let solid_hash = encode(&solid(16, 16, [128, 128, 128]), 16, 16, 4, 3).expect("solid");
        let mut gradient = Vec::new();
        for _ in 0..16 {
            for x in 0..16 {
                let v = (x * 16) as u8;
                gradient.extend_from_slice(&[v, v, v]);
            }
        }
        let gradient_hash = encode(&gradient, 16, 16, 4, 3).expect("gradient");
        assert_ne!(solid_hash, gradient_hash);
    }

    #[test]
    fn bad_input_is_rejected_rather_than_producing_a_wrong_hash() {
        assert!(matches!(
            encode(&[0; 3], 1, 1, 0, 3),
            Err(BlurhashError::Components { .. })
        ));
        assert!(matches!(
            encode(&[0; 3], 1, 1, 10, 3),
            Err(BlurhashError::Components { .. })
        ));
        assert!(matches!(encode(&[], 0, 0, 4, 3), Err(BlurhashError::Empty)));
        assert!(matches!(
            encode(&[0; 5], 2, 2, 4, 3),
            Err(BlurhashError::Size {
                expected: 12,
                actual: 5
            })
        ));
    }

    #[test]
    fn a_malformed_hash_yields_no_colour_rather_than_panicking() {
        assert_eq!(average_colour(""), None);
        assert_eq!(average_colour("abc"), None);
        // A character outside the alphabet, INSIDE the four digits actually read.
        assert_eq!(average_colour("LE\u{00e9}V6nkc"), None);
        // And a multi-byte character straddling the slice boundary. Indexing a `&str`
        // here rather than its bytes panics outright, and these values come out of a
        // database — one corrupt row would take the process with it.
        assert_eq!(average_colour("L\u{00e9}HV6nkc"), None);
        assert_eq!(average_colour("LEHV\u{00e9}nkc"), None);
    }
}
