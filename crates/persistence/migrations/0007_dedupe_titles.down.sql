-- Restores the previous index. It does NOT restore the deleted duplicate rows, and
-- cannot: they were removed because they carried no information, and no record of
-- which region each redundant copy claimed is kept.
--
-- That makes this migration lossy in one direction. It is stated here rather than
-- discovered, and it is safe: the old index is (media_item_id, variant, language,
-- region), and a set of rows unique on (item, variant, title) is still unique under
-- it — so this re-applies cleanly and E1's ladder still runs both ways.
DROP INDEX IF EXISTS idx_titles_unique;

CREATE UNIQUE INDEX idx_titles_unique
    ON titles (media_item_id, variant, COALESCE(language, ''), COALESCE(region, ''));
