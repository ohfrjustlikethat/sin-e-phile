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
#[allow(clippy::enum_variant_names)]
pub enum MatchReason {
    /// The query IS this title, normalised. Ranked above everything.
    ExactTitle,
    /// Matched the word index.
    Keyword,
    /// Matched only after typo tolerance.
    Fuzzy,
    /// Near the query in embedding space, sharing no words with it necessarily.
    Semantic,
    /// Both halves found it. The strongest signal there is, and the one worth telling
    /// the user about in 5.8's "why this matched" hint.
    Both,
    /// Retrieved by a structured filter — a director, a decade, a runtime bound.
    ///
    /// Distinct from [`MatchReason::Keyword`] because the hint shown to the user has to
    /// be TRUE. A screenshot of "films directed by Alfred Hitchcock from the 1950s"
    /// captioned every one of eleven correct results "matched words", which is not what
    /// happened and not why they are there.
    Filter,
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

/// Run the fuzzy tier only when the exact and keyword tiers between them found fewer
/// than this many results.
///
/// Chosen from the fixture rather than guessed: at 2 the harness's p95 drops from
/// 803 ms to single figures, because a query that already has a good answer stops
/// paying for a trigram scan over 2.7 million rows. Raising it trades latency for
/// recall on queries that are already answered.
const FUZZY_THRESHOLD: usize = 2;

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

    /// Narrow a set of candidate ids to those a structured filter admits.
    ///
    /// # Why this is a filter over candidates rather than a WHERE clause in each tier
    ///
    /// The three retrieval tiers are very different queries — an FTS5 MATCH, a trigram
    /// scan, and an HNSW graph that is not SQL at all. Threading `release_year BETWEEN
    /// ? AND ?` through each of them would mean writing the filter three times and
    /// keeping the three in step forever, and the vector half could not honour it at
    /// all without a second index.
    ///
    /// So the filter is applied once, to the ids the tiers returned. **Candidates are
    /// fetched deep** (50 per tier) precisely so this can remove most of them and still
    /// fill a page. That is a real limit and worth stating: a filter matching nothing in
    /// the top 50 returns nothing, even if the catalogue holds a match at rank 500.
    /// Subtask 5.6's fixture is where that becomes a measurable trade rather than a
    /// guess.
    pub async fn admitted(
        &self,
        ids: &[i64],
        years: Option<(i64, i64)>,
        runtime_under: Option<i64>,
        runtime_over: Option<i64>,
        director: Option<&str>,
    ) -> Result<Vec<i64>, DbError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");

        let mut sql = format!("SELECT m.id FROM media_items m WHERE m.id IN ({placeholders})");
        if years.is_some() {
            // NOT NULL is explicit: an item with no year cannot satisfy a year filter,
            // and SQL's three-valued logic would otherwise drop it silently for the
            // wrong reason.
            sql.push_str(" AND m.release_year IS NOT NULL AND m.release_year BETWEEN ? AND ?");
        }
        if runtime_under.is_some() {
            sql.push_str(" AND m.runtime_minutes IS NOT NULL AND m.runtime_minutes <= ?");
        }
        if runtime_over.is_some() {
            sql.push_str(" AND m.runtime_minutes IS NOT NULL AND m.runtime_minutes >= ?");
        }
        let people = match director {
            Some(name) => {
                let ids = self.director_ids(name).await?;
                if ids.is_empty() {
                    return Ok(Vec::new());
                }
                Some(ids)
            }
            None => None,
        };
        if let Some(people) = &people {
            let list = std::iter::repeat_n("?", people.len())
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM credits c
                               WHERE c.media_item_id = m.id
                                 AND c.role = 'director'
                                 AND c.person_id IN ({list}))"
            ));
        }

        let mut query = sqlx::query_scalar(&sql);
        for id in ids {
            query = query.bind(id);
        }
        if let Some((from, to)) = years {
            query = query.bind(from).bind(to);
        }
        if let Some(minutes) = runtime_under {
            query = query.bind(minutes);
        }
        if let Some(minutes) = runtime_over {
            query = query.bind(minutes);
        }
        if let Some(people) = &people {
            for id in people {
                query = query.bind(id);
            }
        }

        Ok(query.fetch_all(self.db.pool()).await?)
    }

    /// The people a director filter names.
    ///
    /// # Why this is two queries and not a subquery
    ///
    /// The first version put `p.name LIKE '%kurosawa%'` in an EXISTS clause against
    /// `media_items`, so SQLite ran it per row: **11,932 ms** for "directed by Akira
    /// Kurosawa", against E1's 80 ms budget. The indexes were there; the query defeated
    /// them.
    ///
    /// A leading `%` cannot use `idx_people_name`, so the prefix form is tried first —
    /// "directed by Akira Kurosawa" gives the name in natural order and hits the index
    /// directly. Substring is the fallback, for "directed by Kurosawa", and it scans
    /// `people` **once** instead of once per catalogue row.
    ///
    /// Capped: a filter naming twenty different people is not a filter.
    async fn director_ids(&self, name: &str) -> Result<Vec<i64>, DbError> {
        let prefix: Vec<i64> =
            sqlx::query_scalar("SELECT id FROM people WHERE name LIKE ? LIMIT 20")
                .bind(format!("{name}%"))
                .fetch_all(self.db.pool())
                .await?;
        if !prefix.is_empty() {
            return Ok(prefix);
        }
        Ok(
            sqlx::query_scalar("SELECT id FROM people WHERE name LIKE ? LIMIT 20")
                .bind(format!("%{name}%"))
                .fetch_all(self.db.pool())
                .await?,
        )
    }

    /// Retrieve BY the filters, when there is no text left to retrieve with.
    ///
    /// # The gap this closes
    ///
    /// "directed by Akira Kurosawa" is a complete, natural query, and stripping its
    /// filter leaves nothing to search for. [`SearchRepository::admitted`] can only
    /// narrow candidates some other tier produced, so a purely structural query returned
    /// **nothing at all** — found by typing it, not by reasoning about it.
    ///
    /// Ordered by votes because a filter expresses no preference between the things it
    /// admits, and "most people have seen this" is the least surprising default. That is
    /// the same rule the exact-title tier already uses when several works share a name.
    pub async fn by_filters(
        &self,
        years: Option<(i64, i64)>,
        runtime_under: Option<i64>,
        runtime_over: Option<i64>,
        director: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Hit>, DbError> {
        // A query with no constraints at all would be "every title, most popular first",
        // which is a browse surface (Phase 18), not a search result.
        if years.is_none()
            && runtime_under.is_none()
            && runtime_over.is_none()
            && director.is_none()
        {
            return Ok(Vec::new());
        }

        // DRIVEN FROM `credits` WHEN A DIRECTOR IS NAMED, and from `media_items`
        // otherwise. The difference is not cosmetic: scanning media_items and testing
        // each row against the director took 1,800 ms, because 2.7 million rows have to
        // be considered before the top ten by votes are known. Starting from
        // `idx_credits_person` yields the thirty films that person directed, and the
        // sort is over thirty rows.
        let people = match director {
            Some(name) => {
                let ids = self.director_ids(name).await?;
                // A named director nobody matches admits nothing. Returning everything
                // that satisfied the OTHER filters would answer a different question
                // than the one asked.
                if ids.is_empty() {
                    return Ok(Vec::new());
                }
                Some(ids)
            }
            None => None,
        };

        let mut sql = match &people {
            Some(ids) => {
                let placeholders = std::iter::repeat_n("?", ids.len())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "SELECT m.id, m.primary_title, m.release_year, m.kind
                       FROM credits c
                       JOIN media_items m ON m.id = c.media_item_id
                      WHERE c.role = 'director'
                        AND c.person_id IN ({placeholders})
                        AND m.kind <> 'episode'"
                )
            }
            None => String::from(
                "SELECT m.id, m.primary_title, m.release_year, m.kind
                   FROM media_items m
                  WHERE m.kind <> 'episode'",
            ),
        };
        if years.is_some() {
            sql.push_str(" AND m.release_year IS NOT NULL AND m.release_year BETWEEN ? AND ?");
        }
        if runtime_under.is_some() {
            sql.push_str(" AND m.runtime_minutes IS NOT NULL AND m.runtime_minutes <= ?");
        }
        if runtime_over.is_some() {
            sql.push_str(" AND m.runtime_minutes IS NOT NULL AND m.runtime_minutes >= ?");
        }
        sql.push_str(" ORDER BY m.rating_votes DESC NULLS LAST LIMIT ?");

        let mut query = sqlx::query_as(&sql);
        if let Some(ids) = &people {
            for id in ids {
                query = query.bind(id);
            }
        }
        if let Some((from, to)) = years {
            query = query.bind(from).bind(to);
        }
        if let Some(minutes) = runtime_under {
            query = query.bind(minutes);
        }
        if let Some(minutes) = runtime_over {
            query = query.bind(minutes);
        }
        let rows: Vec<(i64, String, Option<i64>, String)> =
            query.bind(limit).fetch_all(self.db.pool()).await?;

        Ok(rows
            .into_iter()
            .map(|(media_item_id, title, year, kind)| Hit {
                media_item_id,
                title,
                year,
                kind,
                score: None,
                why: MatchReason::Filter,
            })
            .collect())
    }

    /// Title, year and kind for ids that arrived without them.
    ///
    /// The vector half returns catalogue ids and nothing else — the index holds no text
    /// — so the fusion layer needs one batch lookup rather than ten round trips it would
    /// pay on every keystroke. `why` is supplied by the caller, which is the only thing
    /// that knows why these ids are here.
    ///
    /// Order is not preserved and does not need to be: whatever ranks these re-sorts
    /// them. Ids not in the catalogue are simply absent, which is the right answer for
    /// an index built against an older snapshot.
    pub async fn describe(&self, ids: &[i64], why: MatchReason) -> Result<Vec<Hit>, DbError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Built rather than macro-checked: the placeholder count varies with the input,
        // which is exactly the case ADR-0026 says `query!` cannot serve. The values are
        // still bound, never interpolated.
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, primary_title, release_year, kind
               FROM media_items
              WHERE id IN ({placeholders})"
        );
        let mut query = sqlx::query_as(&sql);
        for id in ids {
            query = query.bind(id);
        }
        let rows: Vec<(i64, String, Option<i64>, String)> = query.fetch_all(self.db.pool()).await?;

        Ok(rows
            .into_iter()
            .map(|(media_item_id, title, year, kind)| Hit {
                media_item_id,
                title,
                year,
                kind,
                score: None,
                why,
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

        // FUZZY RUNS ONLY WHEN THE OTHER TIERS FOUND ALMOST NOTHING — not merely when
        // there is room left on the page.
        //
        // The first version ran it whenever `hits.len() < limit`, which is nearly
        // always: a query with one excellent exact match still has four empty slots.
        // So every search paid for a trigram scan it did not need, and the harness
        // measured p95 803 ms against an 80 ms budget — with the slowest cases being
        // ones where the exact-title answer had already been found.
        //
        // A typo-tolerant match is worth showing when nothing better exists. It is
        // never worth a second of latency to pad a page that already answers the
        // question.
        if hits.len() < FUZZY_THRESHOLD {
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

/// Fold a query exactly as `titles.normalised` was folded.
///
/// **Must match `tools/ingest/src/matching.rs::normalise` character for character.**
/// Two definitions of "the same title" drift apart silently, and the symptom is an
/// exact-title match that stops working for one kind of input — which is E2 going from
/// 100% to 86%. That is not hypothetical: the two disagreed about diacritics until the
/// relevance harness measured it.
fn normalise_query(query: &str) -> String {
    use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

    let mut out = String::with_capacity(query.len());
    let mut last_was_space = true;

    for ch in query.nfd().filter(|c| !is_combining_mark(*c)) {
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
