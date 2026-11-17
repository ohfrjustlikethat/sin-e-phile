-- 0027 — This year's releases, fast (Phase 16). Home's "New releases" asks for the
-- most-voted titles of this year; walking idx_media_items_votes in vote order and
-- filtering by year read most of the catalogue (98 ms on the real one, and a profile
-- switch went from 30 ms to 118 ms). By year first, it reads only that year's titles.
CREATE INDEX IF NOT EXISTS idx_media_items_year_votes
    ON media_items (release_year, rating_votes DESC)
    WHERE rating_votes IS NOT NULL AND is_adult = 0;
