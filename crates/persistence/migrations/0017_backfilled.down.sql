-- Reverses 0017. The rows themselves stay: they are catalogue entries a user found,
-- and dropping the marker is not a reason to delete them.
DROP INDEX IF EXISTS idx_media_items_backfilled;
ALTER TABLE media_items DROP COLUMN backfilled_from;
