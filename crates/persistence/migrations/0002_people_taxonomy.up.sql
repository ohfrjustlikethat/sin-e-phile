-- 0002 — People, credits, and taxonomy.

CREATE TABLE people (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    -- Romaji/native split matters here for the same reason it does for titles:
    -- "Hayao Miyazaki" and "宮崎 駿" are one person, and Phase 5 must match either.
    name_native TEXT,
    birth_year  INTEGER,
    death_year  INTEGER,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_people_name ON people (name);

-- Credits are the join, and `role` is deliberately free-ish text constrained to a
-- small set. Phase 15's taste model cares enormously about director and much less
-- about the rest, so `role` is indexed with the item.
CREATE TABLE credits (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    person_id     INTEGER NOT NULL REFERENCES people(id)      ON DELETE CASCADE,
    role          TEXT    NOT NULL,
    -- Character played, for a cast credit. NOT NULL with an empty default rather
    -- than nullable, because it is part of the primary key and SQLite prohibits
    -- expressions there — COALESCE(character, '') is legal in an index but not in
    -- a PRIMARY KEY. An actor can hold two credits on one film for two roles, so
    -- the character genuinely belongs in the key.
    character     TEXT    NOT NULL DEFAULT '',
    -- Billing order. Nulls sort last, so a film with no ordering still lists.
    billing       INTEGER,

    PRIMARY KEY (media_item_id, person_id, role, character),
    CHECK (role IN (
        'director', 'writer', 'actor', 'composer', 'cinematographer',
        'editor', 'producer', 'studio', 'creator'
    ))
);

CREATE INDEX idx_credits_person ON credits (person_id, role);
CREATE INDEX idx_credits_item   ON credits (media_item_id, role, billing);

-- Genres are a closed vocabulary; keywords are an open one. They are separate
-- tables because they behave differently: genre is a filter with ~20 values,
-- keyword is a long tail with tens of thousands and is what the Phase 5 document
-- builder leans on for "films about grief".
CREATE TABLE genres (
    id   INTEGER PRIMARY KEY,
    name TEXT    NOT NULL UNIQUE
);

CREATE TABLE media_genres (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    genre_id      INTEGER NOT NULL REFERENCES genres(id)      ON DELETE CASCADE,
    PRIMARY KEY (media_item_id, genre_id)
);

CREATE INDEX idx_media_genres_genre ON media_genres (genre_id);

CREATE TABLE keywords (
    id   INTEGER PRIMARY KEY,
    name TEXT    NOT NULL UNIQUE
);

CREATE TABLE media_keywords (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    keyword_id    INTEGER NOT NULL REFERENCES keywords(id)    ON DELETE CASCADE,
    -- Source weighting: a keyword from TMDB's curated list is worth more than one
    -- inferred from a synopsis. Phase 5 uses this when composing the embedding
    -- document, so it must exist before Phase 5, not be retrofitted into it.
    weight        REAL    NOT NULL DEFAULT 1.0,
    PRIMARY KEY (media_item_id, keyword_id)
);

CREATE INDEX idx_media_keywords_keyword ON media_keywords (keyword_id);
