DROP TABLE rate_skipped;
DROP TABLE continue_hidden;
DROP VIEW title_ratings;
DROP TABLE season_ratings;
CREATE TABLE ratings_stars (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    stars         INTEGER NOT NULL,
    rated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (profile_id, media_item_id),
    CHECK (stars BETWEEN 1 AND 5)
);
INSERT INTO ratings_stars (profile_id, media_item_id, stars, rated_at)
    SELECT profile_id, media_item_id, MAX(1, (halves + 1) / 2), rated_at FROM ratings;
DROP TABLE ratings;
ALTER TABLE ratings_stars RENAME TO ratings;
