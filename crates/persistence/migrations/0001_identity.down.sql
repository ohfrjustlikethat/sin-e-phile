-- Reverse of 0001. Children first: dropping media_items while external_ids still
-- references it fails outright with foreign_keys ON, which is the correct
-- behaviour and the reason this order is not incidental.
DROP INDEX IF EXISTS idx_titles_unique;
DROP INDEX IF EXISTS idx_titles_text;
DROP INDEX IF EXISTS idx_titles_item;
DROP TABLE IF EXISTS titles;

DROP INDEX IF EXISTS idx_external_ids_lookup;
DROP TABLE IF EXISTS external_ids;

DROP INDEX IF EXISTS idx_media_items_rating;
DROP INDEX IF EXISTS idx_media_items_title;
DROP INDEX IF EXISTS idx_media_items_sort;
DROP INDEX IF EXISTS idx_media_items_kind_year;
DROP TABLE IF EXISTS media_items;
