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
///
/// **2 removed the title and the alternative titles.** See the comment in [`build`]:
/// they are names rather than descriptions, and they were dominating the embedding to
/// the point that a plot query returned films whose titles shared a word with it.
pub const VERSION: u32 = 2;

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

/// The arrangement of a document — what goes in it and in what order.
///
/// # Why this is a parameter rather than four edits
///
/// D39 asked whether the layout is what costs the ranking, and the honest way to answer
/// is to embed the alternative and look. Doing that by editing [`SHIPPED`] and rebuilding
/// costs three hours per hypothesis; doing it by hand-concatenating strings in the
/// harness costs a minute and answers *a different question*, because a document
/// assembled by `format!("{document} {synopsis}")` contains the first 400 characters of
/// the synopsis **twice** and no rebuild would ever produce it.
///
/// So the variants are built by the real builder, through this. A priced change is then
/// a `Layout` literal, and the thing measured is the thing that would ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// How much of the synopsis survives the cut.
    pub synopsis_chars: usize,
    /// Put the synopsis before the metadata rather than after it.
    ///
    /// CLS pooling weights the head of the sequence heavily, so what leads the document
    /// is not a cosmetic choice.
    pub synopsis_first: bool,
    /// Whether the billed names appear at all.
    pub people: bool,
    /// Drop a Wikipedia-style lead sentence — *"The Return is a 2003 Russian drama film
    /// directed by…"* — which restates the title and the credits the document already
    /// carries, and pushes the plot toward the cut.
    pub strip_lead: bool,
}

/// What the artefact actually holds. [`build`] is this, and changing it is a re-embed.
pub const SHIPPED: Layout = Layout {
    synopsis_chars: SYNOPSIS_CHARS,
    synopsis_first: false,
    people: true,
    strip_lead: false,
};

/// Build the sentence for one item.
///
/// The output is a plain declarative sentence rather than a bag of fields, because the
/// model was trained on prose. `"Stalker. 1979. film. Science Fiction."` and
/// `"Stalker (1979), a science fiction film."` are not equally good inputs to something
/// trained on natural language, and the second is what this produces.
///
/// **[`VERSION`] does not bump for the introduction of [`Layout`]**: this is `build_with`
/// at [`SHIPPED`], and the output is byte-identical to what it was. What proves that
/// rather than asserts it is `eval embed --report`, which re-derives documents from the
/// live catalogue and compares their quantised vectors against the published artefact —
/// 10 of 10 identical, and it has been seen to fail at 2 of 10 when a tokenizer setting
/// moved.
pub fn build(doc: &Document<'_>) -> String {
    build_with(doc, SHIPPED)
}

/// Build the sentence under an arbitrary [`Layout`]. See [`build`] for the shipped one.
pub fn build_with(doc: &Document<'_>, layout: Layout) -> String {
    let mut metadata = String::with_capacity(256);

    // NO TITLE. Version 2 removed it, and the measurement that forced the change is
    // worth keeping: with the title leading the document, "a grieving janitor becomes
    // guardian of his teenage nephew in a Massachusetts fishing town" returned *Fish
    // Hooky* and *Killer Fish* — matching the word "fishing" in a TITLE — while the film
    // whose synopsis describes exactly that plot never appeared.
    //
    // A title is a name, not a description, and it was drowning out the description.
    // Nothing is lost: the exact-title short-circuit and BM25 both index titles already,
    // and they match names better than an embedding ever will. For an enriched item the
    // title is in the text anyway, because a Wikipedia lead opens with it.
    //
    // `SPEC.md` Phase 5 lists the document's ingredients as "synopsis, genres, keywords,
    // director, mood descriptors, and era". The title is not among them; including it
    // was my addition, and removing it is a correction toward the spec.
    let year_prefix = match doc.year {
        Some(year) => format!("({year}) "),
        None => String::new(),
    };
    metadata.push_str(&year_prefix);

    let descriptor = describe_kind(doc.kind);
    if doc.genres.is_empty() {
        metadata.push_str(descriptor);
    } else {
        metadata.push_str(&format!("{} {descriptor}", lowercase_list(doc.genres)));
    }

    if layout.people && !doc.people.is_empty() {
        metadata.push_str(", featuring ");
        metadata.push_str(&join_prose(doc.people));
    }

    metadata.push('.');

    let synopsis = doc
        .synopsis
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let body = if layout.strip_lead { strip_lead(s) } else { s };
            truncate_on_a_boundary(body, layout.synopsis_chars)
        });

    // Each part appears exactly once, in one order or the other. That is the whole
    // reason this is a builder rather than a `format!` in the harness: a variant made by
    // gluing text onto the shipped document contains its synopsis prefix twice, and no
    // rebuild would ever produce it — so measuring one answers a question nobody asked.
    match (synopsis, layout.synopsis_first) {
        (Some(text), false) => format!("{metadata} {text}"),
        (Some(text), true) => format!("{text} {metadata}"),
        (None, _) => metadata,
    }
}

/// Drop a Wikipedia-style lead sentence, keeping the rest.
///
/// *"The Return is a 2003 Russian drama film directed by Andrey Zvyagintsev."* restates
/// the title the document deliberately omits and the credits it already carries, and
/// under CLS pooling it is weighted heavily while the plot behind it falls past the cut.
///
/// Conservative on purpose: it fires only on a lead that both reads as a definition and
/// names a form, because a synopsis whose real first sentence happens to contain "is a"
/// would otherwise lose its opening.
fn strip_lead(synopsis: &str) -> &str {
    let Some(end) = synopsis.find(". ") else {
        return synopsis;
    };
    let lead = &synopsis[..end];
    let looks_like_boilerplate = lead.contains(" is a ") || lead.contains(" is an ");
    let names_a_form = ["film", "series", "documentary", "short", "anime", "drama"]
        .iter()
        .any(|form| lead.contains(form));

    if looks_like_boilerplate && names_a_form {
        synopsis[end + 2..].trim()
    } else {
        synopsis
    }
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
        // No title and no alternative titles — see `build`. What survives describes the
        // work rather than naming it, which is the only thing an embedding is good at.
        assert_eq!(
            build(&doc),
            "(1979) science fiction drama film, \
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
        assert!(build(&unknown).contains("title"));
        assert!(!build(&unknown).contains("something_new"));
    }

    #[test]
    fn no_name_of_the_work_reaches_the_document() {
        // Version 2's whole point. Titles are names, not descriptions, and while they
        // were in the document a plot query returned films that merely shared a word
        // with a title. The de-duplication this test used to check is gone with them.
        let doc = Document {
            title: "Akira",
            alternative_titles: &["Akira", "AKIRA", "アキラ"],
            year: Some(1988),
            kind: "anime_film",
            ..Default::default()
        };
        let built = build(&doc);
        assert!(!built.contains("Akira"), "{built}");
        assert!(!built.contains("アキラ"), "{built}");
        assert!(!built.contains("also known as"), "{built}");
        assert_eq!(built, "(1988) anime film.");
    }

    #[test]
    fn a_missing_field_leaves_no_gap_in_the_sentence() {
        // Every field is optional in the catalogue, and an item with nothing but a kind
        // must still produce something a model can read rather than an empty string.
        let bare = Document {
            title: "Untitled",
            kind: "film",
            ..Default::default()
        };
        assert_eq!(build(&bare), "film.");

        let no_year = Document {
            title: "Nosferatu",
            kind: "film",
            genres: &["Horror"],
            ..Default::default()
        };
        assert_eq!(build(&no_year), "horror film.");
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

    /// The shipped layout IS `build`. If this ever fails, the artefact and the query
    /// path have drifted and every vector in the file is stale.
    #[test]
    fn build_is_build_with_at_the_shipped_layout() {
        let doc = Document {
            title: "Stalker",
            alternative_titles: &["Сталкер"],
            year: Some(1979),
            kind: "film",
            genres: &["Science Fiction", "Drama"],
            people: &["Andrei Tarkovsky"],
            synopsis: Some("A guide leads two men through the Zone."),
        };
        assert_eq!(build(&doc), build_with(&doc, SHIPPED));
    }

    /// The property the harness's old hand-rolled variants did not have.
    ///
    /// `format!("{document} {synopsis}")` put the first 400 characters of the synopsis in
    /// the string twice, so what got embedded was not a document any rebuild could
    /// produce. A variant is only worth measuring if it is what would ship.
    #[test]
    fn a_reordered_layout_does_not_duplicate_anything() {
        let doc = Document {
            title: "T",
            year: Some(2003),
            kind: "film",
            genres: &["Drama"],
            people: &["A Director"],
            synopsis: Some("Two brothers travel with a father they do not know."),
            ..Default::default()
        };
        let first = build_with(
            &doc,
            Layout {
                synopsis_first: true,
                ..SHIPPED
            },
        );

        assert!(first.starts_with("Two brothers"), "{first}");
        assert!(first.ends_with("featuring A Director."), "{first}");
        assert_eq!(first.matches("Two brothers").count(), 1, "{first}");
        assert_eq!(first.matches("A Director").count(), 1, "{first}");

        // Same ingredients as the shipped order, rearranged — nothing added, nothing lost.
        let shipped = build(&doc);
        let mut a: Vec<&str> = shipped.split_whitespace().collect();
        let mut b: Vec<&str> = first.split_whitespace().collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
    }

    #[test]
    fn a_longer_budget_keeps_more_of_the_synopsis_and_no_more() {
        let synopsis = format!("{}end", "word ".repeat(300));
        let doc = Document {
            title: "T",
            kind: "film",
            synopsis: Some(&synopsis),
            ..Default::default()
        };
        let short = build(&doc);
        let long = build_with(
            &doc,
            Layout {
                synopsis_chars: 1_000,
                ..SHIPPED
            },
        );
        assert!(long.chars().count() > short.chars().count());
        assert!(long.chars().count() <= 1_000 + 32);
    }

    #[test]
    fn people_can_be_left_out_entirely() {
        let doc = Document {
            title: "T",
            kind: "film",
            people: &["Casey Affleck", "Michelle Williams"],
            synopsis: Some("A janitor returns home."),
            ..Default::default()
        };
        let without = build_with(
            &doc,
            Layout {
                people: false,
                ..SHIPPED
            },
        );
        assert!(!without.contains("Casey Affleck"), "{without}");
        assert!(!without.contains("featuring"), "{without}");
        assert!(without.contains("A janitor returns home."), "{without}");
    }

    #[test]
    fn a_lead_sentence_goes_only_when_it_is_boilerplate() {
        // Boilerplate: restates the title and the credits the document already carries.
        let doc = Document {
            title: "T",
            kind: "film",
            synopsis: Some(
                "The Return is a 2003 Russian drama film directed by Andrey Zvyagintsev. \
                 Two brothers travel with a father they do not know.",
            ),
            ..Default::default()
        };
        let stripped = build_with(
            &doc,
            Layout {
                strip_lead: true,
                ..SHIPPED
            },
        );
        assert!(!stripped.contains("Zvyagintsev"), "{stripped}");
        assert!(stripped.contains("Two brothers travel"), "{stripped}");

        // Not boilerplate: a real opening that merely contains "is a". Dropping this
        // would cost the synopsis its first sentence for nothing.
        let plot = Document {
            synopsis: Some("Marriage is a battlefield. He leaves at dawn."),
            ..doc.clone()
        };
        let kept = build_with(
            &plot,
            Layout {
                strip_lead: true,
                ..SHIPPED
            },
        );
        assert!(kept.contains("Marriage is a battlefield."), "{kept}");
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
