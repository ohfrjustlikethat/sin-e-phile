DROP INDEX idx_media_items_votes_changed;
ALTER TABLE media_items DROP COLUMN votes_changed_at;
ALTER TABLE media_items DROP COLUMN previous_votes;
