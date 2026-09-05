-- 0011 — The keyword half of search (Phase 5, subtask 5.1).
--
-- FTS5 with BM25. The vector half is a separate artefact and a separate index; this is
-- the one that must work with no model, no artefact and no network, because SPEC.md §8
-- makes FTS5-only the Tier 0 fallback and ADR-0014 makes it the behaviour when the
-- embedding artefact is absent. It is the floor the rest of search stands on.
--
-- VERIFIED BEFORE BEING BUILT ON, 2026-09-06: the SQLite bundled with sqlx 0.8.6 is
-- 3.46.0 and supports fts5, the trigram tokenizer, unicode61 with diacritic folding,
-- and bm25(). Two assumptions went unchecked earlier in this project and both were
-- wrong, so this one was probed first.

-- ── The main index ───────────────────────────────────────────────────────────
--
-- CONTENTLESS (`content=''`). FTS5 can shadow an external table, but every column it
-- indexes here is assembled from four different tables — titles, credits, people,
-- genres — so there is no single table to shadow. Contentless means the text is stored
-- once, in the index, rather than duplicated beside a copy we would then have to keep
-- in step.
--
-- `rowid` IS `media_items.id`. That is the whole join: a match returns rowids, and the
-- caller reads those items directly. Storing the id as a column too would index it as
-- searchable text, so that "1997" matched every film whose internal id happened to be
-- 1997.
--
-- `remove_diacritics 2` folds accents, so "Amelie" finds "Amélie" and "Nausicaa" finds
-- "Nausicaä". Version 2 rather than 1 because 1 leaves characters outside Latin-1
-- alone, which would miss exactly the titles this catalogue is full of.
CREATE VIRTUAL TABLE search_index USING fts5(
    -- The primary display title, weighted highest by the caller's bm25() weights.
    title,
    -- Every other name it is known by: romaji, native, English, regional.
    alternative_titles,
    -- Directors first, then billed cast. Core tier only — nothing else has credits.
    people,
    -- Genres, and later keywords.
    keywords,
    tokenize = 'unicode61 remove_diacritics 2',
    -- CONTENTLESS_DELETE=1, and it is load-bearing. Without it, deleting from a
    -- contentless table means re-supplying the exact original text through the
    -- `'delete'` command — and issuing that for a row which was never inserted does
    -- not error, it CORRUPTS THE INDEX (SQLITE_CORRUPT_VTAB, "database disk image is
    -- malformed"). The first version of the indexer did precisely that on every first
    -- insert. With this, `DELETE FROM search_index WHERE rowid = ?` is ordinary SQL and
    -- deleting a row that is not there is a no-op. Requires SQLite 3.43+; we have 3.46.
    content = '',
    contentless_delete = 1
);

-- ── Typo tolerance ───────────────────────────────────────────────────────────
--
-- A SEPARATE TABLE, and separate deliberately. The trigram tokenizer indexes every
-- three-character window of every string, so it is far larger per row than a word
-- tokenizer and answers a different question: "is this nearly right?" rather than "does
-- this contain these words".
--
-- Keeping it apart means its cost can be measured on its own and it can be dropped,
-- reduced to the core tier, or made optional without touching the main index. R4's
-- headroom is 452 MB and this is the single largest unknown in Phase 5 — so it is built
-- second, measured, and only then trusted.
--
-- Only `title`: trigram-indexing every alternative title and every actor's name would
-- multiply the largest index in the database by the number of names in it.
CREATE VIRTUAL TABLE search_trigram USING fts5(
    title,
    tokenize = 'trigram',
    content = '',
    contentless_delete = 1
);

-- Which media items are currently in the index, so a rebuild can be incremental and a
-- caller can tell "not indexed yet" from "indexed and does not match" — the difference
-- between a catalogue still building and a film that genuinely is not there.
--
-- A plain table rather than a query against the FTS index, because a contentless FTS5
-- table cannot be scanned: `SELECT rowid FROM search_index` is not allowed without a
-- MATCH, by design.
CREATE TABLE search_indexed (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    -- Which pass wrote it, so widening the indexed set later is a targeted top-up
    -- rather than a full rebuild.
    generation    INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_search_indexed_generation ON search_indexed (generation);
