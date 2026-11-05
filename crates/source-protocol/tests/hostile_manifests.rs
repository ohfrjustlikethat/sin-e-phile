//! Phase 6 E3: "A malformed or hostile addon manifest is rejected with a clear error
//! and cannot crash or hang the app."
//!
//! Three kinds of evidence, because "rejected" alone would pass a validator that
//! rejects everything:
//!
//! 1. A corpus on disk (`fixtures/addon-manifests/`): every valid file parses, and
//!    every hostile file is refused FOR ITS OWN REASON — the expected error is named
//!    per file, so a check that fires on the wrong rule fails here.
//! 2. Inputs too large to keep as files: oversize, a hundred thousand brackets,
//!    invalid UTF-8, NUL bytes. Each must be refused fast.
//! 3. Ten thousand random mutations of a good manifest, none of which may panic, and
//!    any that survive must still satisfy every rule.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use source_protocol::manifest::{MAX_DEPTH, MAX_MANIFEST_BYTES};
use source_protocol::{Manifest, ManifestError};
use url::Url;

fn corpus(dir: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/addon-manifests")
        .join(dir)
}

fn origin() -> Url {
    Url::parse("https://addon.example/manifest.json").unwrap()
}

fn parse(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    Manifest::parse(bytes, &origin())
}

/// Which rule each hostile file must trip — the field or kind of error, not just "an
/// error". Keep this in step with `fixtures/addon-manifests/hostile/`; the test below
/// fails on any file missing from it.
const EXPECTED: &[(&str, &str)] = &[
    ("carries-code.json", "Malformed"),
    ("protocol-2.json", "UnsupportedProtocol"),
    ("protocol-as-string.json", "Malformed"),
    ("endpoint-on-another-host.json", "endpoints.find"),
    ("endpoint-on-another-port.json", "endpoints.resolve"),
    ("endpoint-plain-http.json", "Url"),
    ("endpoint-file-scheme.json", "Url"),
    ("endpoint-with-password.json", "Url"),
    ("homepage-javascript.json", "Url"),
    ("name-bidi-override.json", "name"),
    ("name-zero-width.json", "name"),
    ("name-control-character.json", "name"),
    ("name-blank.json", "name"),
    ("name-too-long.json", "name"),
    ("description-too-long.json", "description"),
    ("id-uppercase.json", "id"),
    ("id-path-traversal.json", "id"),
    ("media-unknown-only.json", "media"),
    ("media-empty.json", "media"),
    ("transports-too-many.json", "transports"),
    ("missing-endpoints.json", "Malformed"),
    ("endpoints-extra-field.json", "Malformed"),
    ("not-json.json", "Malformed"),
    ("json-array.json", "Malformed"),
    ("truncated.json", "Malformed"),
];

/// The variant name, or for a field error the field — what `EXPECTED` names.
fn rule(err: &ManifestError) -> String {
    match err {
        ManifestError::Field { field, .. } => field.to_string(),
        other => format!("{other:?}")
            .split(['(', ' ', '{'])
            .next()
            .unwrap()
            .to_string(),
    }
}

#[test]
fn every_valid_manifest_in_the_corpus_parses() {
    let mut n = 0;
    for entry in std::fs::read_dir(corpus("valid")).unwrap() {
        let path = entry.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        if let Err(e) = parse(&bytes) {
            panic!("{} was refused: {e}", path.display());
        }
        n += 1;
    }
    assert!(n >= 3, "the valid corpus has gone missing");
}

#[test]
fn every_hostile_manifest_is_refused_for_its_own_reason() {
    let mut seen = 0;
    for entry in std::fs::read_dir(corpus("hostile")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let expected = EXPECTED
            .iter()
            .find(|(f, _)| *f == name)
            .unwrap_or_else(|| panic!("{name} has no expected rule in EXPECTED"))
            .1;
        let err = parse(&std::fs::read(&path).unwrap())
            .err()
            .unwrap_or_else(|| panic!("{name} was ACCEPTED"));
        assert_eq!(
            rule(&err),
            expected,
            "{name}: refused, but by the wrong rule: {err}"
        );
        // "A clear error": a sentence, not a dump, and never the attacker's whole input.
        let message = err.to_string();
        assert!(
            (10..=300).contains(&message.len()),
            "{name}: message is {} chars: {message}",
            message.len()
        );
        seen += 1;
    }
    assert_eq!(
        seen,
        EXPECTED.len(),
        "a hostile fixture listed in EXPECTED is missing"
    );
}

/// Refused, and quickly: the budget is far above what a check should take and far
/// below anything a person would call a hang.
fn refused_fast(bytes: &[u8]) -> ManifestError {
    let start = Instant::now();
    let err = parse(bytes).expect_err("accepted");
    let took = start.elapsed();
    assert!(took < Duration::from_millis(250), "took {took:?}");
    err
}

#[test]
fn oversize_input_is_refused_before_it_is_read() {
    let mut big = br#"{"protocol":1,"description":""#.to_vec();
    big.resize(MAX_MANIFEST_BYTES + 1, b'a');
    assert!(matches!(refused_fast(&big), ManifestError::TooLarge(_)));
    // Fifty megabytes costs no more than one over the line.
    let huge = vec![b' '; 50 * 1024 * 1024];
    assert!(matches!(refused_fast(&huge), ManifestError::TooLarge(_)));
}

#[test]
fn deep_nesting_cannot_reach_the_recursive_parser() {
    for open in *b"[{" {
        let deep = vec![open; MAX_MANIFEST_BYTES];
        assert!(matches!(refused_fast(&deep), ManifestError::TooDeep));
    }
    let just_over: String = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
    assert!(matches!(
        refused_fast(just_over.as_bytes()),
        ManifestError::TooDeep
    ));
}

#[test]
fn bytes_that_are_not_text_are_refused() {
    assert!(matches!(
        refused_fast(&[0xff, 0xfe, 0x7b, 0x7d]),
        ManifestError::NotText
    ));
    assert!(matches!(refused_fast(b""), ManifestError::Malformed(_)));
    assert!(matches!(
        refused_fast(b"\0\0\0\0"),
        ManifestError::Malformed(_)
    ));
}

#[test]
fn an_error_never_quotes_more_than_a_little_of_the_input() {
    // serde names an unknown field in its message; the field name is attacker-chosen.
    let key = "k".repeat(30_000);
    let json = format!(r#"{{"{key}":1}}"#);
    let err = refused_fast(json.as_bytes());
    assert!(
        err.to_string().len() <= 300,
        "{} chars",
        err.to_string().len()
    );
}

/// xorshift64: deterministic, so a failure reproduces, and no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn ten_thousand_mutations_never_panic_and_survivors_still_obey_every_rule() {
    let good = std::fs::read(corpus("valid").join("everything.json")).unwrap();
    // Bytes that matter to JSON and to the checks, plus the bidi override's UTF-8.
    const INTERESTING: &[u8] = b"{}[]\":,\\ 0azAZ/.:@\xe2\x80\xae\xff";
    let mut rng = Rng(0x5eed_f00d_cafe_d00d);
    let (mut accepted, start) = (0, Instant::now());
    for _ in 0..10_000 {
        let mut m = good.clone();
        for _ in 0..1 + rng.below(4) {
            let at = rng.below(m.len());
            match rng.below(3) {
                0 => m[at] = INTERESTING[rng.below(INTERESTING.len())],
                1 => {
                    m.remove(at);
                }
                _ => m.insert(at, INTERESTING[rng.below(INTERESTING.len())]),
            }
        }
        if let Ok(manifest) = parse(&m) {
            accepted += 1;
            // A survivor is a manifest the app would install, so re-check the rules
            // that matter most rather than trusting the path that let it through.
            for ep in [&manifest.endpoints.find, &manifest.endpoints.resolve] {
                let url = Url::parse(ep).unwrap();
                assert_eq!(url.scheme(), "https", "{ep}");
                assert_eq!(url.host_str(), Some("addon.example"), "{ep}");
            }
            assert!(!manifest.name.contains('\u{202e}'));
        }
    }
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
    // Most single-byte edits to JSON break it; some land in a description and survive.
    // Neither all nor none is plausible, and either would mean the harness is broken.
    assert!(accepted > 0 && accepted < 10_000, "accepted {accepted}");
}
