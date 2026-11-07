-- 0025 — The films most people know, fast (Phase 14). Home's "Widely loved" rail and
-- onboarding's title cards ask for the most-voted films, overall and decade by decade;
-- without this each query sorted the catalogue in a temporary B-tree (2.1 s overall and
-- 2.3 s for a round of eleven decades on the full catalogue; 0.001 s and 0.067 s with it).
-- The decade query walks this index in vote order and filters by year (see
-- MediaRepository::most_voted_films_between).
CREATE INDEX IF NOT EXISTS idx_media_items_votes
    ON media_items (rating_votes DESC)
    WHERE rating_votes IS NOT NULL AND is_adult = 0;
