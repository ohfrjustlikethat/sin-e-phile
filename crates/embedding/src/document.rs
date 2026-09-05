//! Turning a catalogue row into the sentence that gets embedded.
//!
//! This is the part of semantic search that decides what "similar" means. The model is
//! fixed; what it sees is not. A document of `"Stalker"` and a document of
//! `"Stalker (1979), a science fiction drama directed by Andrei Tarkovsky. Three men
//! travel through a forbidden zone…"` embed to entirely different places, and only one
//! of them answers "slow Russian film about a mysterious zone".
//!
//! # Why this is versioned
//!
//! ADR-0014 requires the artefact to record the **document-builder version**, and this
//! is why: changing what goes into the sentence changes every vector in the file. A
//! catalogue embedded under v1 and a query embedded under v2 are being compared in two
//! different spaces, and the failure is silent — results simply get worse. The version
//! is the thing that makes that detectable instead.
//!
//! **Bump [`VERSION`] for any change to [`build`], including one that looks cosmetic.**
//! A different separator is a different string is a different vector.

/// The document-builder version. See the module note: bump on ANY change to `build`.
pub const VERSION: u32 = 1;

/// What the builder is given about one catalogue item.
///
/// Deliberately borrowed and flat rather than a database row: this crate is shared
/// with the application and must not depend on the persistence layer's types, or the
/// format would drag the schema along with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document<'a> {
    pub title: &'a str,
    /// Other names it is known by — romaji, native, the English release title. Included
    /// because "Sen to Chihiro" and "Spirited Away" must land in the same place.
    pub alternative_titles: &'a [&'a str],
    pub year: Option<i64>,
    /// `film`, `anime_series`, and so on.
    pub kind: &'a str,
    pub genres: &'a [&'a str],
    /// Directors and the leading cast, in billing order. A few names, not the whole
    /// crew: a hundred names dilutes the sentence until every film is about its
    /// production, and the model has a token limit besides.
    pub people: &'a [&'a str],
    pub synopsis: Option<&'a str>,
}

/// How much of a synopsis to keep.
///
/// The model truncates at 256 tokens anyway, and a long synopsis crowds out the title
/// and the names — which are the parts a search is most often actually about. Cutting
/// here rather than letting the tokenizer do it means the cut is deterministic and
/// visible, instead of depending on a tokenizer version.
const SYNOPSIS_CHARS: usize = 400;

/// Build the sentence for one item.
///
/// The output is a plain declarative sentence rather than a bag of fields, because the
/// model was trained on prose. `"Stalker. 1979. film. Science Fiction."` and
/// `"Stalker (1979), a science fiction film."` are not equally good inputs to something
/// trained on natural language, and the second is what this produces.
pub fn build(doc: &Document<'_>) -> String {
    let mut out = String::with_capacity(256);

    out.push_str(doc.title.trim());
    if let Some(year) = doc.year {
        out.push_str(&format!(" ({year})"));
    }

    // Alternative titles, de-duplicated against the primary and each other. A film
    // whose romaji and English titles are identical must not say it twice.
    let mut seen: Vec<String> = vec![normalise_for_dedupe(doc.title)];
    let mut alternatives: Vec<&str> = Vec::new();
    for alt in doc.alternative_titles {
        let key = normalise_for_dedupe(alt);
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        alternatives.push(alt.trim());
    }
    if !alternatives.is_empty() {
        out.push_str(", also known as ");
        out.push_str(&join_prose(&alternatives));
    }

    let descriptor = describe_kind(doc.kind);
    if doc.genres.is_empty() {
        out.push_str(&format!(", {descriptor}"));
    } else {
        out.push_str(&format!(", {} {descriptor}", lowercase_list(doc.genres)));
    }

    if !doc.people.is_empty() {
        out.push_str(", featuring ");
        out.push_str(&join_prose(doc.people));
    }

    out.push('.');

    if let Some(synopsis) = doc.synopsis.map(str::trim).filter(|s| !s.is_empty()) {
        out.push(' ');
        out.push_str(&truncate_on_a_boundary(synopsis, SYNOPSIS_CHARS));
    }

    out
}

/// `anime_series` reads as "anime series" to a language model, not as an identifier.
fn describe_kind(kind: &str) -> &'static str {
    match kind {
        "film" => "film",
        "series" => "television series",
        "episode" => "television episode",
        "anime_film" => "anime film",
        "anime_series" => "anime series",
        "live_channel" => "live channel",
        "manga_chapter" => "manga chapter",
        "comic_issue" => "comic issue",
        // An unknown kind must not become part of the sentence: a raw identifier is
        // noise, and a wrong guess is worse than a vague one.
        _ => "title",
    }
}

fn normalise_for_dedupe(text: &str) -> String {
    text.trim().to_lowercase()
}

fn lowercase_list(items: &[&str]) -> String {
    items
        .iter()
        .map(|g| g.trim().to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

/// "a, b and c" — prose, because the model reads prose.
fn join_prose(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => one.trim().to_string(),
        [rest @ .., last] => format!(
            "{} and {}",
            rest.iter().map(|s| s.trim()).collect::<Vec<_>>().join(", "),
            last.trim()
        ),
    }
}

/// Cut at a character boundary, and prefer a word boundary near it.
///
/// Slicing a `&str` by byte index panics mid-character, and synopses contain accented
/// names and non-Latin scripts constantly.
fn truncate_on_a_boundary(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    match cut.rfind(' ') {
        // Only back up to a space if it is reasonably near the end, or a synopsis with
        // one very long word would lose most of itself.
        Some(space) if space > max_chars * 3 / 4 => {
            format!("{}…", &cut[..space])
        }
        _ => format!("{cut}…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_document_reads_as_a_sentence() {
        let doc = Document {
            title: "Stalker",
            alternative_titles: &["Сталкер"],
            year: Some(1979),
            kind: "film",
            genres: &["Science Fiction", "Drama"],
            people: &["Andrei Tarkovsky", "Alexander Kaidanovsky"],
            synopsis: Some("A guide leads two men through the Zone."),
        };
        assert_eq!(
            build(&doc),
            "Stalker (1979), also known as Сталкер, science fiction drama film, \
             featuring Andrei Tarkovsky and Alexander Kaidanovsky. \
             A guide leads two men through the Zone."
        );
    }

    #[test]
    fn the_kind_is_described_rather_than_named() {
        // "anime_series" is an identifier. The model reads prose.
        let base = Document {
            title: "X",
            kind: "anime_series",
            ..Default::default()
        };
        assert!(build(&base).contains("anime series"));
        assert!(!build(&base).contains("anime_series"));

        // An unknown kind falls back to something vague rather than leaking the raw
        // value into the sentence.
        let unknown = Document {
            kind: "something_new",
            ..base.clone()
        };
        assert!(build(&unknown).contains(", title"));
        assert!(!build(&unknown).contains("something_new"));
    }

    #[test]
    fn a_duplicate_alternative_title_is_not_said_twice() {
        // Extremely common: AniList's romaji and English forms are identical for a
        // great many titles, and repeating a name skews its own embedding.
        let doc = Document {
            title: "Akira",
            alternative_titles: &["Akira", "AKIRA", " akira "],
            year: Some(1988),
            kind: "anime_film",
            ..Default::default()
        };
        let built = build(&doc);
        assert_eq!(built.matches("Akira").count(), 1, "{built}");
        assert!(!built.contains("also known as"));
    }

    #[test]
    fn a_missing_field_leaves_no_gap_in_the_sentence() {
        // Every field except the title is optional in the catalogue, and an item with
        // only a title must still produce something a model can read.
        let bare = Document {
            title: "Untitled",
            kind: "film",
            ..Default::default()
        };
        assert_eq!(build(&bare), "Untitled, film.");

        let no_year = Document {
            title: "Nosferatu",
            kind: "film",
            genres: &["Horror"],
            ..Default::default()
        };
        assert_eq!(build(&no_year), "Nosferatu, horror film.");
    }

    #[test]
    fn a_long_synopsis_is_cut_at_a_character_boundary() {
        // Slicing a &str by byte index panics mid-character, and synopses are full of
        // accents and non-Latin scripts.
        let synopsis = "é".repeat(1_000);
        let doc = Document {
            title: "T",
            kind: "film",
            synopsis: Some(&synopsis),
            ..Default::default()
        };
        let built = build(&doc);
        assert!(built.ends_with('…'));
        assert!(built.chars().count() < 500);
    }

    #[test]
    fn a_synopsis_is_cut_at_a_word_where_one_is_near() {
        let synopsis = format!("{} and then something else entirely", "word ".repeat(120));
        let doc = Document {
            title: "T",
            kind: "film",
            synopsis: Some(&synopsis),
            ..Default::default()
        };
        let built = build(&doc);
        assert!(built.ends_with('…'));
        assert!(
            !built.contains("wor…"),
            "cut mid-word rather than at the nearby space: {built}"
        );
    }

    #[test]
    fn the_output_is_deterministic() {
        // The whole artefact's determinism rests on this: same input, same bytes.
        let doc = Document {
            title: "Seven Samurai",
            alternative_titles: &["Shichinin no samurai", "七人の侍"],
            year: Some(1954),
            kind: "film",
            genres: &["Action", "Drama"],
            people: &["Akira Kurosawa", "Toshiro Mifune"],
            synopsis: Some("A village hires seven warriors."),
        };
        let once = build(&doc);
        for _ in 0..50 {
            assert_eq!(build(&doc), once);
        }
    }

    #[test]
    fn a_list_of_one_does_not_say_and() {
        let doc = Document {
            title: "T",
            kind: "film",
            people: &["Only Person"],
            ..Default::default()
        };
        assert!(build(&doc).contains("featuring Only Person."));
    }
}
