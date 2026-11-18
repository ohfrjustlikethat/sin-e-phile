-- 0031 — The neighbour graph by source (ADR-0054 step 3). MovieLens is a snapshot GroupLens
-- updates rarely, so two sources that change every day stand beside it: AniList's community
-- recommendations for anime and TMDB's for films and series (with a profile's own key). Each
-- row says where it came from, so a MovieLens rebuild replaces only MovieLens' rows and a
-- live source's rows for one title are replaced when that title is asked again.
CREATE TABLE item_neighbours_by_source (
    media_item_id INTEGER NOT NULL REFERENCES media_items (id) ON DELETE CASCADE,
    source        TEXT    NOT NULL CHECK (source IN ('movielens', 'anilist', 'tmdb')),
    neighbour_id  INTEGER NOT NULL REFERENCES media_items (id) ON DELETE CASCADE,
    score         REAL    NOT NULL,
    PRIMARY KEY (media_item_id, source, neighbour_id)
) WITHOUT ROWID;

INSERT INTO item_neighbours_by_source (media_item_id, source, neighbour_id, score)
    SELECT media_item_id, 'movielens', neighbour_id, score FROM item_neighbours;

DROP TABLE item_neighbours;
ALTER TABLE item_neighbours_by_source RENAME TO item_neighbours;

-- When each title was last asked of a live source, answered or not: a week later it is
-- asked again (ADR-0054), and not before.
CREATE TABLE neighbours_asked (
    media_item_id INTEGER NOT NULL REFERENCES media_items (id) ON DELETE CASCADE,
    source        TEXT    NOT NULL CHECK (source IN ('anilist', 'tmdb')),
    asked_at      TEXT    NOT NULL,
    PRIMARY KEY (media_item_id, source)
) WITHOUT ROWID;
