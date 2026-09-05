//! Keyword search: FTS5, BM25, and the exact-title short-circuit.
//!
//! The vector half lives elsewhere. **This half must work with no model, no artefact
//! and no network** — `SPEC.md` §8 makes FTS5-only the Tier 0 fallback and ADR-0014
//! makes it the behaviour when the embedding artefact is absent. It is the floor.
//!
//! # The short-circuit is not an optimisation
//!
//! Exit criterion E2 is a **100%** exact-title top-1 rate. Not "high", not "usually":
//! typing *Solaris* and not getting *Solaris* first is the failure a search engine is
//! not allowed to have, and no amount of good ranking makes it impossible — BM25 will
//! happily rank a documentary *about* Solaris above it if the documentary mentions the
//! word more often.
//!
//! So an exact match on the normalised title **bypasses ranking entirely** and is
//! returned first. Ranking then orders everything after it. That is the only way to
//! make a 100% requirement true rather than likely.

use crate::db::{Db, DbError};

/// One result, and why it is here.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub media_item_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub kind: String,
    /// Lower is better — BM25 returns negative scores, most relevant most negative.
    /// `None` for an exact-title hit, which did not go through ranking at all.
    pub score: Option<f64>,
    pub why: MatchReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchReason {
    /// The query IS this title, normalised. Ranked above everything.
    ExactTitle,
    /// Matched the word index.
    Keyword,
    /// Matched only after typo tolerance.
    Fuzzy,
}

/// Column weights for `bm25()`.
///
/// A title match matters far more than an actor's name appearing somewhere. Without
/// weights, a prolific actor's filmography outranks the film actually being searched
/// for, because their name occurs in thousands of documents and the tokenizer cannot
/// tell that a title is a different kind of thing from a cast list.
const WEIGHT_TITLE: f64 = 10.0;
const WEIGHT_ALTERNATIVE: f64 = 6.0;
const WEIGHT_PEOPLE: f64 = 2.0;
const WEIGHT_KEYWORDS: f64 = 1.0;

pub struct SearchRepository<'a> {
    db: &'a Db,
}

impl<'a> SearchRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Index one item. `rowid` is the media item's id — see migration 0011.
    pub async fn index_item(
        &self,
        media_item_id: i64,
        title: &str,
        alternative_titles: &str,
        people: &str,
        keywords: &str,
    ) -> Result<(), DbError> {
        let mut tx = self.db.pool().begin().await?;

        // A contentless FTS5 table cannot UPDATE in place, so re-indexing is a delete
        // and an insert. This is a plain DELETE only because the table declares
        // `contentless_delete=1` (migration 0011) — without it, deleting means
        // re-supplying the original text through the `'delete'` command, and doing that
        // for a row which was never inserted CORRUPTS THE INDEX rather than failing.
        // The first version did exactly that, swallowed the error with `.ok()`, and
        // every test that touched search reported "database disk image is malformed".
        sqlx::query("DELETE FROM search_index WHERE rowid = ?")
            .bind(media_item_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT INTO search_index(rowid, title, alternative_titles, people, keywords)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(media_item_id)
        .bind(title)
        .bind(alternative_titles)
        .bind(people)
        .bind(keywords)
        .execute(&mut *tx)
        .await?;

        sqlx::query("INSERT INTO search_indexed (media_item_id, generation) VALUES (?, 1) ON CONFLICT (media_item_id) DO NOTHING")
            .bind(media_item_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Add a title to the trigram index, for typo tolerance.
    pub async fn index_trigram(&self, media_item_id: i64, title: &str) -> Result<(), DbError> {
        sqlx::query("INSERT INTO search_trigram(rowid, title) VALUES (?, ?)")
            .bind(media_item_id)
            .bind(title)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// How many items are indexed. Distinguishes "not indexed yet" from "no match".
    pub async fn indexed_count(&self) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM search_indexed")
            .fetch_one(self.db.pool())
            .await?)
    }

    /// Items whose exact normalised title is the query.
    ///
    /// Uses `titles.normalised` from migration 0009, which already holds the folded
    /// form for every title variant — so this finds *Sen to Chihiro no Kamikakushi*
    /// when the user types it, not only the display title.
    pub async fn exact_title(&self, query: &str, limit: i64) -> Result<Vec<Hit>, DbError> {
        let normalised = normalise_query(query);
        if normalised.is_empty() {
            return Ok(Vec::new());
        }

        let rows: Vec<(i64, String, Option<i64>, String)> = sqlx::query_as(
            "SELECT DISTINCT m.id, m.primary_title, m.release_year, m.kind
               FROM titles t
               JOIN media_items m ON m.id = t.media_item_id
              WHERE t.normalised = ?
              ORDER BY m.rating_votes DESC NULLS LAST
              LIMIT ?",
        )
        .bind(&normalised)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(media_item_id, title, year, kind)| Hit {
                media_item_id,
                title,
                year,
                kind,
                score: None,
                why: MatchReason::ExactTitle,
            })
            .collect())
    }

    /// Keyword search, BM25-ranked.
    pub async fn keyword(&self, query: &str, limit: i64) -> Result<Vec<Hit>, DbError> {
        let Some(match_expression) = fts_query(query) else {
            return Ok(Vec::new());
        };

        let rows: Vec<(i64, String, Option<i64>, String, f64)> = sqlx::query_as(
            "SELECT m.id, m.primary_title, m.release_year, m.kind,
                    bm25(search_index, ?, ?, ?, ?) AS score
               FROM search_index
               JOIN media_items m ON m.id = search_index.rowid
              WHERE search_index MATCH ?
              ORDER BY score
              LIMIT ?",
        )
        .bind(WEIGHT_TITLE)
        .bind(WEIGHT_ALTERNATIVE)
        .bind(WEIGHT_PEOPLE)
        .bind(WEIGHT_KEYWORDS)
        .bind(&match_expression)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(media_item_id, title, year, kind, score)| Hit {
                media_item_id,
                title,
                year,
                kind,
                score: Some(score),
                why: MatchReason::Keyword,
            })
            .collect())
    }

    /// Typo-tolerant title matching, by trigram OVERLAP.
    ///
    /// # SQLite's trigram tokenizer does substring search, not typo tolerance
    ///
    /// It is easy to assume otherwise — "trigram" sounds like fuzzy matching, and
    /// `SPEC.md` calls this deliverable "trigram fuzzy matching for typos". But a
    /// phrase query against a trigram index asks *"does this document contain this
    /// substring"*, and `Solaris` does not contain `solariss`. Searching for the typo
    /// as a phrase returns nothing at all, which is what the first version did.
    ///
    /// Typo tolerance comes from the trigrams **individually**: `solariss` yields
    /// `sol ola lar ari ris iss`, and `Solaris` contains five of those six. So the
    /// query is the trigrams OR-ed together, and BM25 ranks by how many a document
    /// shares — which is trigram-overlap similarity, computed by the index rather than
    /// by us.
    ///
    /// **A last resort, and priced like one.** OR-ing six trigrams casts a wide net;
    /// it is a way of finding candidates, not a ranking to trust. Running it before the
    /// word index would bury exact word matches under near-misses.
    ///
    /// Below four characters it is refused: almost everything is within one trigram of
    /// everything, and the results are noise wearing a confident expression.
    pub async fn fuzzy(&self, query: &str, limit: i64) -> Result<Vec<Hit>, DbError> {
        let cleaned = normalise_query(query);
        if cleaned.chars().count() < 4 {
            return Ok(Vec::new());
        }

        let Some(phrase) = trigram_query(&cleaned) else {
            return Ok(Vec::new());
        };
        let rows: Vec<(i64, String, Option<i64>, String, f64)> = sqlx::query_as(
            "SELECT m.id, m.primary_title, m.release_year, m.kind, bm25(search_trigram)
               FROM search_trigram
               JOIN media_items m ON m.id = search_trigram.rowid
              WHERE search_trigram MATCH ?
              ORDER BY bm25(search_trigram)
              LIMIT ?",
        )
        .bind(&phrase)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(media_item_id, title, year, kind, score)| Hit {
                media_item_id,
                title,
                year,
                kind,
                score: Some(score),
                why: MatchReason::Fuzzy,
            })
            .collect())
    }

    /// Search: exact titles first, then keyword hits, de-duplicated.
    ///
    /// The order is the guarantee. An exact match is never displaced by a better-scoring
    /// keyword hit, because it never entered the ranking.
    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<Hit>, DbError> {
        let mut hits = self.exact_title(query, limit).await?;
        let mut seen: Vec<i64> = hits.iter().map(|h| h.media_item_id).collect();

        if hits.len() as i64 >= limit {
            hits.truncate(limit as usize);
            return Ok(hits);
        }

        for hit in self.keyword(query, limit).await? {
            if seen.contains(&hit.media_item_id) {
                continue;
            }
            seen.push(hit.media_item_id);
            hits.push(hit);
            if hits.len() as i64 >= limit {
                return Ok(hits);
            }
        }

        // Fuzzy only fills what is left. A typo-tolerant match is worth showing when
        // the word index found little; it is never worth showing INSTEAD of a word
        // match, because "nearly spelled like this" is weaker evidence than "contains
        // this word" and presenting them as equals makes good queries worse.
        if (hits.len() as i64) < limit {
            for hit in self.fuzzy(query, limit).await? {
                if seen.contains(&hit.media_item_id) {
                    continue;
                }
                seen.push(hit.media_item_id);
                hits.push(hit);
                if hits.len() as i64 >= limit {
                    break;
                }
            }
        }
        Ok(hits)
    }
}

/// Fold a query the same way `titles.normalised` was folded.
///
/// Must match `tools/ingest/src/matching.rs::normalise` exactly. Two definitions of
/// "the same title" drift apart silently, and the symptom is an exact-title match that
/// stops working for one kind of punctuation — which is E2 going from 100% to 99%.
fn normalise_query(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    let mut last_was_space = true;
    for ch in query.chars() {
        let ch = ch.to_lowercase().next().unwrap_or(ch);
        if ch.is_alphanumeric() {
            out.push(ch);
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim_end().to_string()
}

/// The query's trigrams, OR-ed, so BM25 ranks by how many a title shares.
///
/// Character-based rather than byte-based: a Japanese title is three characters in
/// nine bytes, and slicing by byte would both split a character and produce trigrams
/// that match nothing.
fn trigram_query(cleaned: &str) -> Option<String> {
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() < 3 {
        return None;
    }
    let mut grams: Vec<String> = Vec::new();
    for window in chars.windows(3) {
        let gram: String = window.iter().collect();
        // A trigram of only spaces matches everything and ranks nothing.
        if gram.trim().is_empty() {
            continue;
        }
        let quoted = format!("\"{}\"", gram.replace('"', ""));
        if !grams.contains(&quoted) {
            grams.push(quoted);
        }
    }
    if grams.is_empty() {
        return None;
    }
    Some(grams.join(" OR "))
}

/// Turn user text into an FTS5 MATCH expression.
///
/// **Every token is quoted.** FTS5's query language treats `"`, `*`, `:`, `-`, `^`,
/// `(`, `)` and the bare words `AND`/`OR`/`NOT` as syntax, so a user typing
/// `Face/Off` or `Mission: Impossible` or `AND THEN THERE WERE NONE` gets a syntax
/// error rather than results. Quoting turns all of it into literal text — which is
/// what someone typing a film title meant.
///
/// Returns `None` when nothing usable survives, so the caller returns no results
/// rather than issuing a malformed query.
fn fts_query(query: &str) -> Option<String> {
    let tokens: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.to_lowercase()))
        .collect();
    if tokens.is_empty() {
        return None;
    }
    Some(tokens.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typo_shares_most_of_its_trigrams_with_the_real_title() {
        // The whole mechanism, stated as a test: "solariss" and "solaris" differ by one
        // character and share five trigrams out of six, which is what lets BM25 rank
        // the real film above unrelated ones.
        let typo = trigram_query("solariss").expect("trigrams");
        let real = trigram_query("solaris").expect("trigrams");
        let shared = real.split(" OR ").filter(|g| typo.contains(*g)).count();
        assert_eq!(shared, 5, "typo {typo} vs real {real}");
        assert!(
            typo.contains("\"iss\""),
            "the typo's own trigram is present too"
        );
    }

    #[test]
    fn trigrams_are_taken_by_character_not_by_byte() {
        // A Japanese title is three characters in nine bytes; slicing by byte would
        // split a character and produce trigrams that match nothing.
        let grams = trigram_query("君の名は").expect("trigrams");
        assert!(grams.contains("君の名"), "{grams}");
        assert!(grams.contains("の名は"), "{grams}");
    }

    #[test]
    fn a_query_too_short_to_have_a_trigram_yields_nothing() {
        assert_eq!(trigram_query("ab"), None);
        assert_eq!(trigram_query(""), None);
        assert_eq!(trigram_query("   "), None);
    }

    #[test]
    fn fts_syntax_in_a_title_is_treated_as_text() {
        // Every one of these is a real film title, and every one is FTS5 syntax.
        assert_eq!(fts_query("Face/Off").as_deref(), Some("\"face\" \"off\""));
        assert_eq!(
            fts_query("Mission: Impossible").as_deref(),
            Some("\"mission\" \"impossible\"")
        );
        // Bare AND/OR/NOT are operators to FTS5 unless quoted.
        assert_eq!(
            fts_query("And Then There Were None").as_deref(),
            Some("\"and\" \"then\" \"there\" \"were\" \"none\"")
        );
        // A lone asterisk or quote must not escape.
        assert_eq!(fts_query("*").as_deref(), None);
        assert_eq!(fts_query("\"\"\"").as_deref(), None);
        assert_eq!(fts_query("").as_deref(), None);
    }

    #[test]
    fn the_query_is_folded_exactly_as_stored_titles_are() {
        // If these two definitions drift, exact-title matching silently stops working
        // for one kind of punctuation — E2 going from 100% to 99%.
        assert_eq!(
            normalise_query("Fullmetal Alchemist: Brotherhood"),
            "fullmetal alchemist brotherhood"
        );
        assert_eq!(normalise_query("  Kimi no Na wa.  "), "kimi no na wa");
        assert_eq!(normalise_query("WALL·E"), "wall e");
        assert_eq!(normalise_query("8½"), "8½");
        assert_eq!(normalise_query("!!!"), "");
    }
}
