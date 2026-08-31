-- 0001 — Media identity.
--
-- SPEC.md §6.2: ONE canonical MediaItem, generic enough to be a film, a TV
-- episode, an anime season, and later a manga chapter WITHOUT a migration.
-- Identity is a stable internal id; everything else in the system references it.

-- The generic type. `kind` is the discriminator, and it carries all eight values
-- from the start — including the two Phase 24-25 needs — because adding a value
-- to a CHECK constraint later means rewriting the table, and the whole argument
-- for a generic schema is that those phases cost nothing extra.
CREATE TABLE media_items (
    id                INTEGER PRIMARY KEY,
    kind              TEXT    NOT NULL,

    -- Denormalised display title. `titles` below is authoritative and holds every
    -- variant; this is the one the UI shows, kept here so listing a rail is a
    -- single-table scan rather than a join per card. Written by the repository,
    -- never by hand.
    primary_title     TEXT    NOT NULL,
    -- Sort form: "Seven Samurai, The" territory, and for anime the romaji that
    -- users actually alphabetise by. Nullable because most items do not differ.
    sort_title        TEXT,

    release_year      INTEGER,
    -- Full date where known. Year alone is what IMDb gives for most of the
    -- catalogue, so the two are separate rather than one nullable date.
    release_date      TEXT,
    runtime_minutes   INTEGER,

    -- ISO 639-1 where known. The ORIGINAL language, not the user's.
    original_language TEXT,
    -- ISO 3166-1 alpha-2, comma-separated for co-productions. A join table would
    -- be more correct and is not worth it: nothing in this app queries by country
    -- except as a filter chip, and Phase 5 pushes filters through FTS anyway.
    countries         TEXT,

    synopsis          TEXT,
    -- 0-100. IMDb's 0-10 and TMDB's 0-10 both scale in; storing an integer avoids
    -- float comparison in ranking, which Phase 15 does a lot of.
    rating            INTEGER,
    rating_votes      INTEGER,

    -- Adult content flag from the source catalogues. Kept because IMDb ships it
    -- and filtering it out later is impossible if it was never stored.
    is_adult          INTEGER NOT NULL DEFAULT 0,

    created_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT    NOT NULL DEFAULT (datetime('now')),

    CHECK (kind IN (
        'film', 'episode', 'series', 'anime_film', 'anime_series',
        'live_channel', 'manga_chapter', 'comic_issue'
    )),
    CHECK (rating IS NULL OR (rating >= 0 AND rating <= 100)),
    CHECK (is_adult IN (0, 1))
);

CREATE INDEX idx_media_items_kind_year   ON media_items (kind, release_year);
CREATE INDEX idx_media_items_sort        ON media_items (kind, sort_title);
-- The Phase 3 exit criterion measures an INDEXED LOOKUP over 500,000 rows. This
-- is the index that criterion is measured against.
CREATE INDEX idx_media_items_title       ON media_items (primary_title);
CREATE INDEX idx_media_items_rating      ON media_items (kind, rating DESC);

-- External identity. One row per (source, item), so an item can carry an IMDb id,
-- a TMDB id, an AniList id and a MAL id simultaneously — which anime routinely
-- does, and which is the whole reason Phase 4's cross-mapping is possible.
CREATE TABLE external_ids (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    source        TEXT    NOT NULL,
    external_id   TEXT    NOT NULL,
    -- How the mapping was established. Phase 4 resolves conflicts between
    -- sources, and a mapping asserted by a dataset join is worth less than one
    -- confirmed by an exact id match — but only if we recorded which it was.
    confidence    REAL    NOT NULL DEFAULT 1.0,

    PRIMARY KEY (media_item_id, source),
    CHECK (source IN ('imdb', 'tmdb', 'tvdb', 'anilist', 'mal', 'movielens')),
    CHECK (confidence >= 0.0 AND confidence <= 1.0)
);

-- The reverse lookup, which is the one ingestion actually performs: "I have
-- tt0047478, which internal item is that?"
CREATE UNIQUE INDEX idx_external_ids_lookup ON external_ids (source, external_id);

-- Every title an item is known by.
--
-- SPEC.md §6.2 requires romaji / native / english variants explicitly. This is
-- also what makes search work for a user who types "Nausicaa" for
-- "風の谷のナウシカ", and it is why `titles` is a table and not three columns:
-- a long-running series accumulates dozens of regional titles.
CREATE TABLE titles (
    id            INTEGER PRIMARY KEY,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    title         TEXT    NOT NULL,
    -- 'primary' | 'original' | 'romaji' | 'native' | 'english' | 'alternative'
    variant       TEXT    NOT NULL,
    -- ISO 639-1, NULL for a transliteration that belongs to no single language.
    language      TEXT,
    -- ISO 3166-1 alpha-2 for a region-specific release title.
    region        TEXT,

    CHECK (variant IN ('primary', 'original', 'romaji', 'native', 'english', 'alternative'))
);

CREATE INDEX idx_titles_item    ON titles (media_item_id, variant);
-- Phase 5's exact-title short-circuit must never get a title wrong, and it looks
-- up by folded title text across every variant.
--
-- COLLATE NOCASE ON THE INDEX, not just in the query. SQLite will only use an
-- index whose collation matches the comparison's, so `WHERE title = ? COLLATE
-- NOCASE` against a BINARY index silently falls back to a full scan. Measured at
-- 500,000 rows: 26.7ms p50 scanning, 0.03ms p50 with this index. Both are inside
-- the Phase 3 budget, which is exactly why it would have gone unnoticed — and a
-- real IMDb catalogue is an order of magnitude larger than the fixture.
CREATE INDEX idx_titles_text    ON titles (title COLLATE NOCASE);
-- One row per (item, variant, language): a series has one English title, not
-- fourteen identical ones accumulated over fourteen ingestion runs.
CREATE UNIQUE INDEX idx_titles_unique
    ON titles (media_item_id, variant, COALESCE(language, ''), COALESCE(region, ''));
