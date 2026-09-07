//! Hybrid search: the keyword half and the vector half, fused by reciprocal rank.
//!
//! # Three tiers, and the order is the guarantee
//!
//! 1. **Exact title** — bypasses ranking entirely and is returned first. E2 asks for a
//!    100% top-1 rate, and no amount of good ranking makes that *certain*.
//! 2. **Fused** — BM25 and vector results combined by reciprocal rank.
//! 3. **Fuzzy** — typo tolerance, only when the tiers above found almost nothing,
//!    because it costs a trigram scan over millions of rows.
//!
//! # Why reciprocal rank fusion and not a weighted score
//!
//! BM25 returns negative scores whose scale depends on the corpus and the query's word
//! frequencies. Cosine distance returns 0 to 2. They are not comparable, and making
//! them comparable means normalising two distributions that both move with the query —
//! a tuning problem with no stable answer.
//!
//! RRF ignores the scores and uses only the **positions**. A document at rank 1 in
//! either list scores `1/(k+1)`; being tenth in both beats being first in neither.
//! Nothing needs calibrating, and a query where one half is useless degrades to the
//! other half rather than to noise.
//!
//! # Degrading without the vector half
//!
//! [`Engine`] holds the semantic half as an `Option`. Absent — no artefact downloaded,
//! Tier 0 declining the download, a machine with no model — search runs on FTS5 alone,
//! which `SPEC.md` §8 and ADR-0014 both require to be *diminished but genuinely useful*,
//! never broken or empty. That path is the floor, and it is the same code path.

pub mod query;

use sinephile_persistence::repositories::{Hit, MatchReason, SearchRepository};
use sinephile_persistence::{Db, DbError};
use sinephile_vector_index::VectorIndex;

pub use sinephile_embedder::{EmbedError, Embedder};

/// Reciprocal rank fusion's smoothing constant.
///
/// The literature's default is 60 (Cormack et al., 2009). **It is a default here too,
/// and it has not been measured** — there is nothing yet to measure it against. E2's
/// fixture is exact titles, which never reach fusion at all, so sweeping `k` against it
/// would produce identical numbers and false confidence. The sweep belongs with the
/// semantic fixture in subtask 5.6.
///
/// Recorded as an open question rather than a settled value, because the last library
/// default taken on trust in this phase (usearch's `ef` of 64) measured wrong for this
/// corpus.
///
/// What it does: a large `k` flattens the difference between ranks, so agreement between
/// the two halves matters more than either one's confidence. A small `k` lets a single
/// first place dominate.
pub const RRF_K: f64 = 60.0;

/// How deep each half's candidate list goes before fusion.
///
/// Fusion needs more than a page: a document ranked eleventh by BM25 and second by the
/// vector half should surface, and it cannot if BM25 only ever offered ten. Costs one
/// larger `LIMIT` on a query that is already indexed.
const CANDIDATES: i64 = 50;

/// Run the fuzzy tier only when everything above found fewer than this.
const FUZZY_THRESHOLD: usize = 2;

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Embed(#[from] EmbedError),
    #[error(transparent)]
    Index(#[from] sinephile_vector_index::VectorIndexError),
}

/// The vector half. Both halves or neither: an index with no embedder cannot be
/// queried, and an embedder with no index has nothing to search.
pub struct Semantic {
    pub index: VectorIndex,
    pub embedder: Embedder,
}

pub struct Engine {
    semantic: Option<Semantic>,
}

impl Engine {
    /// Keyword-only. The Tier 0 floor, and what runs before an artefact is downloaded.
    pub fn keyword_only() -> Self {
        Self { semantic: None }
    }

    pub fn hybrid(semantic: Semantic) -> Self {
        Self {
            semantic: Some(semantic),
        }
    }

    /// Whether the vector half is available — 5.8's UI needs to say so honestly rather
    /// than let a user wonder why "films about grief" returns titles containing "grief".
    pub fn is_hybrid(&self) -> bool {
        self.semantic.is_some()
    }

    pub async fn search(
        &mut self,
        db: &Db,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Hit>, SearchError> {
        let search = SearchRepository::new(db);

        // Filters out first: a decade or a runtime bound is a CONSTRAINT, not something
        // to embed. What is left is the part that carries meaning (`query::parse`).
        let parsed = query::parse(query);
        let text = parsed.text.as_str();

        // TIER 1. Never displaced, never ranked.
        //
        // The ORIGINAL query, not the residual: "blade runner 2049" must reach the
        // exact-title tier whole, and a filter that stripped a year from a title would
        // be the one failure E2 is not allowed to have.
        let mut results = search.exact_title(query, limit).await?;
        let mut seen: Vec<i64> = results.iter().map(|h| h.media_item_id).collect();
        if results.len() as i64 >= limit {
            results.truncate(limit as usize);
            return Ok(results);
        }

        let (under, over) = match parsed.runtime {
            Some(query::Runtime::Under(m)) => (Some(m), None),
            Some(query::Runtime::Over(m)) => (None, Some(m)),
            None => (None, None),
        };

        // A QUERY THAT IS NOTHING BUT FILTERS is answered from the filters. "directed by
        // Akira Kurosawa" leaves no text, and searching for an empty string returns
        // nothing — which is what it did until someone typed it.
        if text.is_empty() {
            for hit in search
                .by_filters(parsed.years, under, over, parsed.director.as_deref(), limit)
                .await?
            {
                if seen.contains(&hit.media_item_id) {
                    continue;
                }
                seen.push(hit.media_item_id);
                results.push(hit);
                if results.len() as i64 >= limit {
                    break;
                }
            }
            return Ok(results);
        }

        // TIER 2. Both halves, as deep as CANDIDATES, then fused by position.
        let keyword = search.keyword(text, CANDIDATES).await?;
        let semantic = match self.semantic.as_mut() {
            Some(semantic) => {
                // embed_QUERY: this is the side that carries the instruction prefix.
                let vector = semantic.embedder.embed_query(text)?;
                // Quantised to match the artefact's own representation. Cosine ignores
                // the per-vector scale the quantiser applies, so this is exact rather
                // than approximate — see `crates/vector-index`.
                let quantised = sinephile_embedding::quantise::quantise(&vector);
                let neighbours = semantic
                    .index
                    .search(&quantised.values, CANDIDATES as usize)?;
                let ids: Vec<i64> = neighbours.iter().map(|n| n.media_item_id).collect();

                // The index knows ids; the catalogue knows what they are. One batch
                // lookup, then put them back in the index's order — `describe` does not
                // preserve it, and here the order IS the rank.
                let described = search.describe(&ids, MatchReason::Semantic).await?;
                ids.iter()
                    .filter_map(|id| described.iter().find(|h| h.media_item_id == *id).cloned())
                    .collect()
            }
            None => Vec::new(),
        };

        let mut fused = fuse(&keyword, &semantic, RRF_K);

        // Applied AFTER fusion and BEFORE the page is filled, so the filter removes
        // candidates rather than results: dropping them afterwards would leave a short
        // page where a full one was available.
        //
        // EVERY tier that can put something on the page goes through this, fuzzy
        // included. The first version filtered only the fused tier, so "comedies under
        // 90 minutes" happily returned three-hour films the moment the fuzzy tier ran —
        // a filter with a hole in it is worse than none, because the user believes it.
        if parsed.has_filters() {
            let candidates: Vec<i64> = fused.iter().map(|h| h.media_item_id).collect();
            let admitted = search
                .admitted(
                    &candidates,
                    parsed.years,
                    under,
                    over,
                    parsed.director.as_deref(),
                )
                .await?;
            fused.retain(|h| admitted.contains(&h.media_item_id));
        }

        for hit in fused {
            if seen.contains(&hit.media_item_id) {
                continue;
            }
            seen.push(hit.media_item_id);
            results.push(hit);
            if results.len() as i64 >= limit {
                return Ok(results);
            }
        }

        // TOP UP FROM THE FILTERS, when they are present and the page is not full.
        //
        // `admitted` can only narrow what the retrieval tiers already found, and they
        // find by TEXT. "films directed by Alfred Hitchcock from the 1950s" leaves the
        // residual "films" — so generic that Hitchcock's fifties never appear among the
        // fifty candidates, and the filter had nothing correct to keep. The page was
        // empty, which is the worst possible answer to a query whose answer is famous.
        //
        // So the filter also RETRIEVES. Text-ranked matches keep their places at the top;
        // these fill what is left, by votes.
        if parsed.has_filters() && (results.len() as i64) < limit {
            for hit in search
                .by_filters(parsed.years, under, over, parsed.director.as_deref(), limit)
                .await?
            {
                if seen.contains(&hit.media_item_id) {
                    continue;
                }
                seen.push(hit.media_item_id);
                results.push(hit);
                if results.len() as i64 >= limit {
                    return Ok(results);
                }
            }
        }

        // TIER 3. Only when there is almost nothing, because it is expensive.
        if results.len() < FUZZY_THRESHOLD {
            let mut fuzzy = search.fuzzy(text, limit).await?;
            if parsed.has_filters() {
                let candidates: Vec<i64> = fuzzy.iter().map(|h| h.media_item_id).collect();
                let admitted = search
                    .admitted(
                        &candidates,
                        parsed.years,
                        under,
                        over,
                        parsed.director.as_deref(),
                    )
                    .await?;
                fuzzy.retain(|h| admitted.contains(&h.media_item_id));
            }
            for hit in fuzzy {
                if seen.contains(&hit.media_item_id) {
                    continue;
                }
                seen.push(hit.media_item_id);
                results.push(hit);
                if results.len() as i64 >= limit {
                    break;
                }
            }
        }

        Ok(results)
    }
}

/// Reciprocal rank fusion of two ranked lists.
///
/// `score(d) = Σ 1/(k + rank(d))` over the lists that contain `d`, ranks counted from 1.
/// A document in both lists is credited twice, which is the entire mechanism: agreement
/// between two independent notions of relevance outranks confidence in either one.
///
/// Ties break on catalogue id so a rerun agrees with itself — floating-point sums of the
/// same reciprocals in a different order are not bitwise equal, and a search that
/// reorders its own results between identical keystrokes looks broken.
pub fn fuse(keyword: &[Hit], semantic: &[Hit], k: f64) -> Vec<Hit> {
    let mut fused: Vec<(Hit, f64)> = Vec::new();

    for list in [keyword, semantic] {
        for (position, hit) in list.iter().enumerate() {
            let contribution = 1.0 / (k + (position + 1) as f64);
            match fused
                .iter_mut()
                .find(|(h, _)| h.media_item_id == hit.media_item_id)
            {
                Some((existing, score)) => {
                    *score += contribution;
                    // Found by both halves — the strongest signal available, and what
                    // 5.8's "why this matched" hint should say.
                    existing.why = MatchReason::Both;
                }
                None => fused.push((hit.clone(), contribution)),
            }
        }
    }

    fused.sort_by(|(a, sa), (b, sb)| {
        sb.partial_cmp(sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.media_item_id.cmp(&b.media_item_id))
    });

    fused
        .into_iter()
        .map(|(mut hit, score)| {
            // Negated so that "lower is better" holds across every tier, as it already
            // does for BM25. Two conventions in one result list is a bug waiting.
            hit.score = Some(-score);
            hit
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: i64, why: MatchReason) -> Hit {
        Hit {
            media_item_id: id,
            title: format!("Item {id}"),
            year: Some(2000),
            kind: "film".into(),
            score: None,
            why,
        }
    }

    fn keyword(ids: &[i64]) -> Vec<Hit> {
        ids.iter()
            .map(|id| hit(*id, MatchReason::Keyword))
            .collect()
    }

    fn semantic(ids: &[i64]) -> Vec<Hit> {
        ids.iter()
            .map(|id| hit(*id, MatchReason::Semantic))
            .collect()
    }

    fn order(hits: &[Hit]) -> Vec<i64> {
        hits.iter().map(|h| h.media_item_id).collect()
    }

    #[test]
    fn agreement_beats_a_single_first_place() {
        // 2 is second in both lists; 1 and 9 are first in one and absent from the other.
        // RRF's whole point: 1/61 + 1/62 > 1/61.
        let fused = fuse(&keyword(&[1, 2, 3]), &semantic(&[9, 2, 8]), RRF_K);
        assert_eq!(fused[0].media_item_id, 2, "{:?}", order(&fused));
        assert_eq!(fused[0].why, MatchReason::Both);
    }

    #[test]
    fn one_useless_half_degrades_to_the_other_rather_than_to_noise() {
        // The vector half returns nothing relevant — or, on Tier 0, nothing at all. The
        // keyword order must survive intact.
        let fused = fuse(&keyword(&[4, 5, 6]), &[], RRF_K);
        assert_eq!(order(&fused), vec![4, 5, 6]);
        assert!(fused.iter().all(|h| h.why == MatchReason::Keyword));
    }

    #[test]
    fn a_semantic_only_hit_still_surfaces() {
        // "films about grief" shares no words with most of what should answer it. If
        // fusion could not surface a keyword-invisible document, the vector half would
        // be decoration.
        let fused = fuse(&keyword(&[1]), &semantic(&[7]), RRF_K);
        assert!(order(&fused).contains(&7), "{:?}", order(&fused));
    }

    #[test]
    fn the_same_input_produces_the_same_order() {
        // Ties are real: two documents each first in one list have identical scores, and
        // an unstable sort would reorder them between identical keystrokes.
        let once = fuse(&keyword(&[3, 1]), &semantic(&[1, 3]), RRF_K);
        let twice = fuse(&keyword(&[3, 1]), &semantic(&[1, 3]), RRF_K);
        assert_eq!(order(&once), order(&twice));
        assert_eq!(order(&once), vec![1, 3], "ties break on id, ascending");
    }

    #[test]
    fn k_decides_how_much_a_first_place_is_worth() {
        // The constant is not decoration, and this is the case that isolates it:
        // document 1 is FIRST in the keyword list and absent from the other, while
        // document 3 is THIRD in both.
        //
        //   k=0   1 scores 1/1 = 1.000,  3 scores 1/3 + 1/3 = 0.667  -> 1 wins
        //   k=60  1 scores 1/61 = 0.016, 3 scores 2/63     = 0.032   -> 3 wins
        //
        // So k is the dial between "trust a confident half" and "trust agreement", and
        // that is what the sweep in 5.6 will be choosing.
        let sharp = fuse(&keyword(&[1, 8, 3]), &semantic(&[9, 7, 3]), 0.0);
        assert_eq!(sharp[0].media_item_id, 1, "k=0: one first place dominates");

        let flat = fuse(&keyword(&[1, 8, 3]), &semantic(&[9, 7, 3]), 60.0);
        assert_eq!(flat[0].media_item_id, 3, "k=60: appearing twice wins");
    }

    #[test]
    fn every_score_is_lower_is_better() {
        // BM25 is negative-is-better and the fused score must not silently invert the
        // convention halfway down a result list.
        let fused = fuse(&keyword(&[1, 2]), &semantic(&[2]), RRF_K);
        let scores: Vec<f64> = fused.iter().filter_map(|h| h.score).collect();
        assert_eq!(scores.len(), fused.len());
        assert!(scores.windows(2).all(|w| w[0] <= w[1]), "{scores:?}");
    }
}
