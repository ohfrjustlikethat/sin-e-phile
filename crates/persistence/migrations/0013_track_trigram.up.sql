-- 0013 — Track which items are in the trigram index, so a re-run is not a rebuild.
--
-- `search_indexed` recorded that an item was in the MAIN index and nothing more, so a
-- second `ingest search-index` re-indexed all 2,702,737 items — idempotent, and 150
-- seconds plus 49 MB of churn for no change. A contentless FTS5 table cannot be
-- scanned to ask what is in it (`SELECT rowid FROM search_trigram` is not allowed
-- without a MATCH, by design), so the answer has to be recorded beside it.
--
-- A column rather than a second table: the two indexes cover overlapping sets of the
-- same items, and one row per item saying which indexes hold it is the smaller and
-- more obvious thing.
ALTER TABLE search_indexed ADD COLUMN trigram INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_search_indexed_trigram ON search_indexed (trigram) WHERE trigram = 0;
