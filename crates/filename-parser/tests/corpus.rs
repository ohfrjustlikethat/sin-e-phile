//! Every field of every hand-labelled name in `fixtures/filenames/`, against what a careful
//! human reads in it (Phase 12 E6: the parser is independently useful only if it is
//! measurably right). `names.psv` is what it was tuned on; `heldout.psv` was written after
//! the tuning and is never tuned on, so its number is the honest one.
//!
//! `cargo test -p filename-parser --test corpus -- --nocapture` prints each miss.

use filename_parser::parse;

/// One labelled row: the name, then what it says.
struct Row {
    line: usize,
    name: String,
    fields: Vec<String>,
}

fn rows(file: &str) -> Vec<Row> {
    let path = format!(
        "{}/../../fixtures/filenames/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect(file);
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|(i, l)| {
            let parts: Vec<String> = l.split('|').map(|f| f.trim().to_string()).collect();
            assert_eq!(parts.len(), 15, "{file} line {}: 15 fields", i + 1);
            Row {
                line: i + 1,
                name: parts[0].clone(),
                fields: parts,
            }
        })
        .collect()
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

fn list(v: &[u32]) -> String {
    v.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// The parsed value of each labelled field, as the corpus writes it.
fn read(name: &str) -> [String; 9] {
    let p = parse(name);
    [
        p.title.clone(),
        opt(p.year),
        opt(p.season),
        list(&p.episodes),
        list(&p.absolute_episodes),
        opt(p.resolution),
        opt(p.source.map(|s| s.as_str())),
        opt(p.codec.map(|c| c.as_str())),
        p.group.clone().unwrap_or_default(),
    ]
}

const FIELDS: [&str; 9] = [
    "title",
    "year",
    "season",
    "episodes",
    "absolute",
    "resolution",
    "source",
    "codec",
    "group",
];

/// Per-field accuracy and the share of names right on every field; each miss printed.
fn measure(rows: &[Row]) -> ([f64; 9], f64) {
    let mut right = [0usize; 9];
    let mut whole = 0;
    for row in rows {
        let got = read(&row.name);
        let mut all = true;
        for (i, field) in FIELDS.iter().enumerate() {
            let want = &row.fields[i + 1];
            // Titles compare without case: "the grand budapest hotel" is read as written.
            let ok = if i == 0 {
                got[i].to_lowercase() == want.to_lowercase()
            } else {
                &got[i] == want
            };
            if ok {
                right[i] += 1;
            } else {
                all = false;
                println!(
                    "line {:>3} {field:<10} want {want:?} got {:?}   {}",
                    row.line, got[i], row.name
                );
            }
        }
        if all {
            whole += 1;
        }
    }
    let n = rows.len() as f64;
    println!("\n{} names", rows.len());
    let rates = right.map(|r| r as f64 / n);
    for (i, field) in FIELDS.iter().enumerate() {
        println!("  {field:<10} {:>6.2}%", 100.0 * rates[i]);
    }
    println!("  every field {:>5.2}%", 100.0 * whole as f64 / n);
    (rates, whole as f64 / n)
}

// The thresholds the parser is held to. Raising one is progress; lowering one needs a
// reason in docs/eval-results.md.

#[test]
fn the_corpus_reads_as_a_human_reads_it() {
    let rows = rows("names.psv");
    assert!(
        rows.len() >= 500,
        "the corpus is 500+ names (SPEC.md Phase 12): {}",
        rows.len()
    );
    let (fields, whole) = measure(&rows);
    for (i, field) in FIELDS.iter().enumerate() {
        assert!(
            fields[i] >= 0.97,
            "{field}: {:.2}% < 97%",
            100.0 * fields[i]
        );
    }
    assert!(
        whole >= 0.95,
        "every field right on {:.2}% < 95%",
        100.0 * whole
    );
}

#[test]
fn names_it_was_never_tuned_on_read_as_well() {
    let rows = rows("heldout.psv");
    assert!(rows.len() >= 100, "{} held-out names", rows.len());
    let (_, whole) = measure(&rows);
    assert!(
        whole >= 0.90,
        "held out: every field right on {:.2}% < 90%",
        100.0 * whole
    );
}
