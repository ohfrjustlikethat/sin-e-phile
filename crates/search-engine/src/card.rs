//! What a result row says besides its title: a byline and, for the lead result, a few
//! lines of synopsis.
//!
//! Here rather than in `src-tauri` because the shaping is logic, and logic there can't
//! be tested (ADR-0022). The command only converts the result into its DTO.

use std::collections::HashMap;

use sinephile_embedding::document::strip_lead;
use sinephile_persistence::repositories::SearchRepository;
use sinephile_persistence::{Db, DbError};

/// Long enough for two lines at the lead row's width, short enough that the lead
/// stays one entry on a list rather than becoming an article.
pub const EXCERPT_CHARS: usize = 220;

/// The details of one result, ready to render.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Card {
    pub made_by: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub excerpt: Option<String>,
}

/// Cards for these ids, keyed by id. An id the catalogue doesn't hold has no card.
pub async fn cards(db: &Db, ids: &[i64]) -> Result<HashMap<i64, Card>, DbError> {
    let details = SearchRepository::new(db).details(ids).await?;
    Ok(details
        .into_iter()
        .map(|d| {
            let card = Card {
                made_by: d.made_by,
                runtime_minutes: d.runtime_minutes,
                excerpt: d
                    .synopsis
                    .as_deref()
                    .and_then(|s| excerpt(s, EXCERPT_CHARS)),
            };
            (d.media_item_id, card)
        })
        .collect())
}

/// Phrases that mark a sentence as being about the production, not the story.
///
/// Many stored synopses are an encyclopedia lead, whose later sentences are release
/// history: *Rear Window*'s second sentence is "Originally released by Paramount
/// Pictures, the film stars James Stewart…". Under a byline that already says who
/// made it and when, that reads as filler. Matched in lower case.
const PRODUCTION_NOTES: &[&str] = &[
    "released by",
    "was released",
    "the film stars",
    "the series stars",
    "starring ",
    "premiered",
    "was screened",
    "box office",
    "grossed",
    "nominated for",
    "academy award",
    "was produced",
    "is based on",
    "was based on",
];

/// The synopsis without its encyclopedia lead or its production notes, cut at a word
/// boundary.
///
/// The lead ("*Rear Window* is a 1954 American mystery thriller film directed by…")
/// only repeats the byline above it, so it's dropped the same way the embedded
/// document drops it, and so is any sentence about the production. `None` when nothing
/// is left: no excerpt is better than trivia in the place a reader expects the story.
pub fn excerpt(synopsis: &str, max_chars: usize) -> Option<String> {
    let story = strip_lead(synopsis.trim())
        .split_inclusive(". ")
        .filter(|sentence| {
            let lower = sentence.to_lowercase();
            !PRODUCTION_NOTES.iter().any(|note| lower.contains(note))
        })
        .collect::<String>();
    let text = story.trim();
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= max_chars {
        return Some(text.to_string());
    }
    // Cut on a char boundary, then back to the last space, so neither a multi-byte
    // character nor a word is split.
    let cut: String = text.chars().take(max_chars).collect();
    let cut = match cut.rfind(' ') {
        Some(space) => &cut[..space],
        None => cut.as_str(),
    };
    let cut = cut.trim_end_matches([',', ';', ':', ' ']);
    Some(format!("{cut}…"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_synopsis_is_returned_whole() {
        assert_eq!(excerpt("A man waits.", 50).as_deref(), Some("A man waits."));
    }

    #[test]
    fn the_encyclopedia_lead_is_dropped() {
        let synopsis = "Rear Window is a 1954 American mystery thriller film. A photographer \
                        watches his neighbours.";
        assert_eq!(
            excerpt(synopsis, 200).as_deref(),
            Some("A photographer watches his neighbours.")
        );
    }

    #[test]
    fn a_long_synopsis_is_cut_at_a_word_with_an_ellipsis() {
        let cut = excerpt("one two three four five", 12).expect("excerpt");
        assert_eq!(cut, "one two…", "never 'one two thr…'");
    }

    #[test]
    fn a_cut_never_splits_a_multi_byte_character() {
        // 'é' is two bytes; a byte-indexed cut at 3 would panic mid-character.
        let cut = excerpt("Amélie rêve à Montmartre", 5).expect("excerpt");
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn trailing_punctuation_does_not_dangle_before_the_ellipsis() {
        assert_eq!(excerpt("alpha, beta gamma", 8).as_deref(), Some("alpha…"));
    }

    #[test]
    fn production_notes_are_dropped_and_the_story_kept() {
        let synopsis = "Originally released by Paramount Pictures, the film stars James \
                        Stewart. A photographer watches his neighbours. It premiered at Venice.";
        assert_eq!(
            excerpt(synopsis, 200).as_deref(),
            Some("A photographer watches his neighbours.")
        );
    }

    #[test]
    fn a_synopsis_that_is_only_production_notes_has_no_excerpt() {
        let synopsis = "Rear Window is a 1954 American mystery thriller film. Originally \
                        released by Paramount Pictures, the film stars James Stewart.";
        assert_eq!(excerpt(synopsis, 200), None);
    }

    #[test]
    fn an_empty_synopsis_has_no_excerpt() {
        assert_eq!(excerpt("   ", 50), None);
    }
}
