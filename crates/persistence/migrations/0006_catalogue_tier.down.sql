DROP INDEX IF EXISTS idx_media_items_core;
-- DROP COLUMN needs SQLite 3.35+. sqlx bundles a newer one, and the migration test
-- rolls the whole ladder down, so an older SQLite would fail there rather than in
-- front of a user.
ALTER TABLE media_items DROP COLUMN in_core;
