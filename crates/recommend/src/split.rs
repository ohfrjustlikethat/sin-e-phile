//! Who builds, who tunes, who judges (Phase 17, E1).
//!
//! Every MovieLens user falls in exactly one part, by a hash of their id, so the split is the
//! same on every run and every machine: 80% build the matrix, 10% tune its two numbers, 10%
//! judge it. Within a tuning or judging user, a fifth of their likes is held out, also by
//! hash, and the rest is what the recommender is shown. A user never builds the matrix they
//! are judged on; [`tests`] pins that.

/// The part a user belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Their likes build the matrix.
    Build,
    /// Choose the score's two numbers on these.
    Tune,
    /// The reported numbers (E1, E2) come from these alone.
    Test,
}

/// splitmix64: a fixed, well-mixed hash, so neighbouring ids do not land together.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

pub fn part(user: u32) -> Part {
    match mix(u64::from(user)) % 10 {
        0 => Part::Tune,
        1 => Part::Test,
        _ => Part::Build,
    }
}

/// Whether a tuning or judging user's like of `item` is held out (about one in five).
pub fn held_out(user: u32, item: u32) -> bool {
    mix((u64::from(user) << 32) | u64::from(item)).is_multiple_of(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_are_about_eighty_ten_ten_and_never_shared() {
        let mut counts = [0usize; 3];
        for user in 1..=100_000u32 {
            let p = part(user);
            // Asked again, the same answer: a user cannot be in two parts.
            assert_eq!(p, part(user));
            counts[p as usize] += 1;
        }
        let share = |n: usize| n as f64 / 100_000.0;
        assert!((share(counts[0]) - 0.8).abs() < 0.01, "{counts:?}");
        assert!((share(counts[1]) - 0.1).abs() < 0.01, "{counts:?}");
        assert!((share(counts[2]) - 0.1).abs() < 0.01, "{counts:?}");
    }

    #[test]
    fn about_a_fifth_of_likes_is_held_out() {
        let held = (1..=50_000u32).filter(|i| held_out(7, *i)).count();
        assert!((held as f64 / 50_000.0 - 0.2).abs() < 0.01, "{held}");
    }
}
