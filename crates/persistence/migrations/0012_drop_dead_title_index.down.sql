-- Restores the index 0012 dropped. Recreating it costs disk and buys nothing — SQLite
-- prefers idx_titles_unique for every query it could serve — but a down migration must
-- return the schema to what it was, not to what would be better.
CREATE INDEX IF NOT EXISTS idx_titles_item ON titles (media_item_id, variant);
