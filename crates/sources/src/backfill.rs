//! Search-triggered backfill — ADR-0030 layer 3, `docs/specs/catalogue-freshness.md`.
//!
//! "If a user searches for something the catalogue does not contain, and the source
//! resolver finds a candidate for it, create the catalogue entry from what the resolver
//! learned." This is what stops an out-of-date catalogue being a dead end.
//!
//! The rules, each because the spec's constraints demand it:
//!
//! - **Called after the empty result is shown**, never inside search: the UI asks for it
//!   once a search has settled on nothing, so it cannot slow a search down.
//! - **A year is required.** A title alone, from one source, is too weak a guess to put
//!   in the catalogue; with a year it is what an IMDb row minimally is.
//! - **Never a second copy.** If the catalogue already has the title in that year —
//!   a search can miss for other reasons — nothing is written.
//! - **One row per call, marked** with the backend that reported it (migration 0017),
//!   so a bulk refresh can reconcile it and Phase 12 can merge it with a real IMDb id.
//!
//! What this does NOT do yet, and Phase 12 will: read a title and year out of a filename
//! for candidates that carry neither, and merge a backfilled row into an IMDb match.

use sinephile_catalogue::matching::normalise;
use sinephile_persistence::repositories::BackfillRepository;
use sinephile_persistence::{Db, DbError};
use source_protocol::{MediaQuery, SourceCandidate};

use crate::registry;
use crate::resolver::Resolver;

/// Shorter than this is a search still being typed, not a film being looked for.
pub const MIN_QUERY_CHARS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backfilled {
    pub id: i64,
    pub title: String,
    pub year: u16,
    pub from: String,
}

pub async fn backfill(
    db: &Db,
    resolver: &Resolver,
    query: &str,
) -> Result<Option<Backfilled>, DbError> {
    let query = query.trim();
    if query.chars().count() < MIN_QUERY_CHARS {
        return Ok(None);
    }
    let found = resolver.find_all(&MediaQuery::film(query, None)).await;
    registry::record(db, &found.outcomes).await?;

    let Some((title, year, from)) = choose(query, &found.candidates) else {
        return Ok(None);
    };
    let repo = BackfillRepository::new(db);
    let year_i = Some(i64::from(year));
    if repo.existing(&title, year_i).await?.is_some()
        || repo.existing(query, year_i).await?.is_some()
    {
        return Ok(None);
    }
    let id = repo.create_film(&title, year_i, &from).await?;
    tracing::info!(id, %title, year, %from, "backfilled a catalogue entry");
    Ok(Some(Backfilled {
        id,
        title,
        year,
        from,
    }))
}

/// The candidate to learn from: one with a year whose title contains every word of the
/// query. Among those, an exact title match beats a longer one — "Nosferatu" over
/// "Nosferatu, eine Symphonie des Grauens (A Symphony of Horror)" — and otherwise the
/// first, which is the resolver's order.
fn choose(query: &str, candidates: &[SourceCandidate]) -> Option<(String, u16, String)> {
    let wanted = normalise(query);
    let words: Vec<&str> = wanted.split(' ').collect();
    let mut fitting = candidates.iter().filter_map(|c| {
        let year = c.year?;
        let have = normalise(&c.title);
        let have_words: Vec<&str> = have.split(' ').collect();
        words
            .iter()
            .all(|w| have_words.contains(w))
            .then_some((c, year, have == wanted))
    });
    let first = fitting.next()?;
    let best = std::iter::once(first)
        .chain(fitting)
        .find(|(_, _, exact)| *exact)
        .unwrap_or(first);
    let (c, year, _) = best;
    Some((
        c.title.trim().to_string(),
        year,
        c.provenance.backend.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_protocol::{Provenance, Transport};

    fn cand(title: &str, year: Option<u16>) -> SourceCandidate {
        SourceCandidate {
            key: title.into(),
            title: title.into(),
            year,
            transport: Transport::Http,
            quality: Default::default(),
            languages: vec![],
            subtitles: vec![],
            codec: Default::default(),
            size_bytes: None,
            health: Default::default(),
            locator: None,
            provenance: Provenance {
                backend: "b".into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn an_exact_title_beats_a_longer_one_and_a_year_is_required() {
        let c = [
            cand("Nosferatu", None),
            cand("Nosferatu, eine Symphonie des Grauens", Some(1922)),
            cand("NOSFERATU", Some(1922)),
        ];
        assert_eq!(
            choose("nosferatu", &c),
            Some(("NOSFERATU".into(), 1922, "b".into()))
        );
        assert_eq!(
            choose("nosferatu", &c[..2]),
            Some((
                "Nosferatu, eine Symphonie des Grauens".into(),
                1922,
                "b".into()
            ))
        );
        assert_eq!(choose("nosferatu", &c[..1]), None, "no year, no row");
        assert_eq!(choose("metropolis", &c), None, "not what was asked for");
    }
}
