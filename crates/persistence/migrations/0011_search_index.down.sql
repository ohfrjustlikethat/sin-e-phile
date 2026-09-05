-- Reverses 0011. The FTS indexes are derived data — everything in them is rebuildable
-- from titles, credits and genres — so dropping them loses nothing that cannot be
-- regenerated, which is why there is no backup step here.
DROP INDEX IF EXISTS idx_search_indexed_generation;
DROP TABLE IF EXISTS search_indexed;
DROP TABLE IF EXISTS search_trigram;
DROP TABLE IF EXISTS search_index;
