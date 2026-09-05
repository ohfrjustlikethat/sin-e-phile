-- Reverses 0013. SQLite cannot drop a column before 3.35 and can after, but the index
-- must go first either way.
DROP INDEX IF EXISTS idx_search_indexed_trigram;
ALTER TABLE search_indexed DROP COLUMN trigram;
