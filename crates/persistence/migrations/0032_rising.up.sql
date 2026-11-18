-- 0032 — Rising now (ADR-0054 step 6). Each ratings refresh used to overwrite a title's vote
-- count, so the app knew how widely a title had been seen but not what people are watching
-- now. A refresh that changes the count keeps the one before and when it changed; the
-- difference is what a title gained since IMDb's last update. ADD COLUMN rewrites nothing.
ALTER TABLE media_items ADD COLUMN previous_votes INTEGER;
ALTER TABLE media_items ADD COLUMN votes_changed_at TEXT;

-- What rose lately, without reading the catalogue: only titles a refresh has changed are in it.
CREATE INDEX idx_media_items_votes_changed ON media_items (votes_changed_at)
    WHERE previous_votes IS NOT NULL;
