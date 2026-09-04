//! WebP re-encoding, measured on real photographs.
//!
//! Synthetic gradients compress nothing like film stills, so a saving measured on one
//! says nothing about the other. `dist/stills/` holds 24 frames from public-domain
//! films — *Battleship Potemkin*, *Detour*, *Beat the Devil* — which are exactly the
//! kind of image this cache will hold.
//!
//! The test asserts a floor rather than a number: the point is that lossy WebP is
//! genuinely smaller than the JPEG it replaces, which is the entire justification for
//! taking a C dependency instead of using `image`'s lossless encoder. The measured
//! figure goes in `docs/eval-results.md`.

use sinephile_artwork::prepare;

fn stills() -> Vec<(String, Vec<u8>)> {
    // From the crate directory up to the repository root.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repository root")
        .join("dist/stills");

    let Ok(dir) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            out.push((
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                bytes,
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn lossy_webp_is_smaller_than_the_jpeg_it_replaces() {
    let stills = stills();
    if stills.is_empty() {
        // The stills are build output rather than committed fixtures, so a clean
        // checkout may not have them. Skipping loudly beats a silent pass.
        eprintln!("no stills in dist/stills — skipping the WebP measurement");
        return;
    }

    let mut original_total = 0usize;
    let mut webp_total = 0usize;
    let mut worst: Option<(String, f64)> = None;

    for (name, bytes) in &stills {
        let prepared = prepare(bytes).expect("a real JPEG must decode");
        original_total += bytes.len();
        webp_total += prepared.webp.len();

        let saving = prepared.saving();
        if worst.as_ref().is_none_or(|(_, w)| saving < *w) {
            worst = Some((name.clone(), saving));
        }

        assert_eq!(
            prepared.blurhash.len(),
            28,
            "{name} produced a malformed blurhash"
        );
    }

    let overall = 1.0 - (webp_total as f64 / original_total as f64);
    let (worst_name, worst_saving) = worst.expect("at least one still");

    eprintln!(
        "{} stills: {:.0} KB -> {:.0} KB, {:.1}% saved overall; worst was {worst_name} at {:.1}%",
        stills.len(),
        original_total as f64 / 1024.0,
        webp_total as f64 / 1024.0,
        overall * 100.0,
        worst_saving * 100.0
    );

    assert!(
        overall > 0.10,
        "lossy WebP saved only {:.1}% overall — if that is all it is worth, the C \
         dependency is not, and `image`'s pure-Rust decoder-plus-store would do",
        overall * 100.0
    );
}

#[test]
fn a_real_still_produces_a_plausible_average_colour() {
    // A blurhash whose average colour is black or white on every image means the
    // linear-light conversion or the DC term is wrong, and every placeholder in the
    // app would be subtly awful in a way no unit test with flat colours would catch.
    let stills = stills();
    if stills.is_empty() {
        eprintln!("no stills in dist/stills — skipping");
        return;
    }

    let mut all_extreme = true;
    for (name, bytes) in stills.iter().take(6) {
        let prepared = prepare(bytes).expect("decode");
        let colour = sinephile_artwork::blurhash::average_colour(&prepared.blurhash)
            .unwrap_or_else(|| panic!("{name} produced an unreadable blurhash"));
        let luma = colour.iter().map(|c| *c as u32).sum::<u32>() / 3;
        if (8..248).contains(&luma) {
            all_extreme = false;
        }
    }
    assert!(
        !all_extreme,
        "every still averaged to near-black or near-white — the DC term is wrong"
    );
}
