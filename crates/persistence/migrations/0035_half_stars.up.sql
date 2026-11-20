-- 0035 — Ratings for any title (ADR-0056, the author's answers of 2026-11-20).
--
-- Half stars, 0.5 to 5, as Letterboxd rates: stored as halves, 1 to 10. Every whole-star
-- rating already given is kept as it was (four stars become eight halves).
CREATE TABLE ratings_halves (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    halves        INTEGER NOT NULL,
    rated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (profile_id, media_item_id),
    CHECK (halves BETWEEN 1 AND 10)
);
INSERT INTO ratings_halves (profile_id, media_item_id, halves, rated_at)
    SELECT profile_id, media_item_id, stars * 2, rated_at FROM ratings;
DROP TABLE ratings;
ALTER TABLE ratings_halves RENAME TO ratings;

-- A show is rated season by season; the show itself is the average of its rated seasons.
CREATE TABLE season_ratings (
    profile_id INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    series_id  INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    season     INTEGER NOT NULL,
    halves     INTEGER NOT NULL,
    rated_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (profile_id, series_id, season),
    CHECK (halves BETWEEN 1 AND 10)
);

-- What every reader of ratings reads (the taste model, liked titles, the discovery
-- engine's outcomes): a title's own rating, or, for a show with rated seasons, their
-- average (which replaces any rating of the show as a whole, given before seasons were).
CREATE VIEW title_ratings AS
    SELECT r.profile_id, r.media_item_id, CAST(r.halves AS REAL) AS halves, r.rated_at
    FROM ratings r
    WHERE NOT EXISTS (SELECT 1 FROM season_ratings s
                      WHERE s.profile_id = r.profile_id AND s.series_id = r.media_item_id)
    UNION ALL
    SELECT profile_id, series_id, AVG(halves), MAX(rated_at)
    FROM season_ratings
    GROUP BY profile_id, series_id;

-- Removed from Continue watching: hidden, its place kept, back once it is played again
-- (any position of it, or of one of the show's episodes, saved after it was hidden).
CREATE TABLE continue_hidden (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    hidden_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (profile_id, media_item_id)
);

-- "Haven't seen it" on the rate-a-batch screen: not asked again, and no signal of taste.
CREATE TABLE rate_skipped (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    PRIMARY KEY (profile_id, media_item_id)
);
