-- 0003 — Series, seasons, episodes, and the numbering problem.
--
-- SPEC.md §6.2 is explicit: "Anime specifically requires absolute vs seasonal
-- episode numbering reconciliation, romaji/English/native title variants, and
-- sub/dub track awareness. Design for this in Phase 3 rather than patching it in
-- Phase 12."
--
-- THE PROBLEM, stated once so the shape of these tables makes sense.
--
-- A long-running anime has no single correct episode number. TVDB says
-- S03E07. AniList says episode 59. MAL splits the same run into separate
-- "seasons" that are really cours and restarts at 1. A fansub release names the
-- file "- 59 -". A Blu-ray box calls it disc 2 episode 3. All five refer to the
-- same broadcast, and Phase 12 will be handed a filename containing exactly one
-- of those numbers and asked which episode it is.
--
-- Storing one canonical number and converting on the fly loses information: the
-- conversions are not arithmetic, because cours split unevenly, recaps are
-- numbered by some sources and not others, and specials interleave. So the schema
-- stores the numbers it was GIVEN, per source, and reconciles by lookup.

CREATE TABLE series (
    -- A series IS a media_item (kind 'series' or 'anime_series'). This is a
    -- one-to-one extension table rather than more columns on media_items, so that
    -- a film row does not carry a dozen always-NULL series columns across 500,000
    -- rows — which is what the Phase 3 lookup budget is measured over.
    media_item_id  INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    -- 'continuing' | 'ended' | 'cancelled' | 'upcoming'
    status         TEXT,
    start_year     INTEGER,
    end_year       INTEGER,
    total_episodes INTEGER,
    -- Anime is frequently produced in cours (roughly 12-13 episode broadcast
    -- blocks). Recording whether a series is cour-structured tells Phase 12
    -- whether a "season 2" from one source is likely to be the same run.
    is_cour_based  INTEGER NOT NULL DEFAULT 0,

    CHECK (status IS NULL OR status IN ('continuing', 'ended', 'cancelled', 'upcoming')),
    CHECK (is_cour_based IN (0, 1))
);

CREATE TABLE seasons (
    id             INTEGER PRIMARY KEY,
    series_id      INTEGER NOT NULL REFERENCES series(media_item_id) ON DELETE CASCADE,
    -- 0 is the conventional "specials" season and is deliberately allowed.
    season_number  INTEGER NOT NULL,
    name           TEXT,
    year           INTEGER,
    episode_count  INTEGER,

    UNIQUE (series_id, season_number),
    CHECK (season_number >= 0)
);

CREATE TABLE episodes (
    media_item_id   INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    series_id       INTEGER NOT NULL REFERENCES series(media_item_id) ON DELETE CASCADE,
    season_id       INTEGER          REFERENCES seasons(id) ON DELETE SET NULL,

    -- BOTH numbering schemes, side by side, as SPEC.md §6.2 requires. Either may
    -- be NULL: a special has no absolute number in most catalogues, and a
    -- straight-to-streaming release sometimes has no seasonal one.
    season_number   INTEGER,
    episode_number  INTEGER,
    absolute_number INTEGER,

    air_date        TEXT,
    -- A recap or clip show. Sources disagree about whether these consume an
    -- episode number, which is the single largest cause of off-by-N drift
    -- between AniList and TVDB numbering.
    is_recap        INTEGER NOT NULL DEFAULT 0,
    is_special      INTEGER NOT NULL DEFAULT 0,

    CHECK (is_recap IN (0, 1)),
    CHECK (is_special IN (0, 1))
);

CREATE INDEX idx_episodes_seasonal ON episodes (series_id, season_number, episode_number);
CREATE INDEX idx_episodes_absolute ON episodes (series_id, absolute_number);
CREATE INDEX idx_episodes_season   ON episodes (season_id);

-- The reconciliation table: what each SOURCE calls this episode.
--
-- This is the part that cannot be derived. When Phase 12 parses "Series - 59" it
-- looks up (series, source, absolute 59) and gets the internal episode id
-- directly, instead of trying to compute which season episode 59 falls in — a
-- computation that is wrong often enough to matter, and wrong silently.
CREATE TABLE episode_numbering (
    episode_id      INTEGER NOT NULL REFERENCES episodes(media_item_id) ON DELETE CASCADE,
    source          TEXT    NOT NULL,
    season_number   INTEGER,
    episode_number  INTEGER,
    absolute_number INTEGER,

    PRIMARY KEY (episode_id, source),
    CHECK (source IN ('imdb', 'tmdb', 'tvdb', 'anilist', 'mal', 'movielens'))
);

-- Both directions are looked up: "what does TVDB call this?" and "what is TVDB's
-- S03E07?". The second is the one Phase 12 needs and the one worth indexing.
CREATE INDEX idx_episode_numbering_seasonal
    ON episode_numbering (source, season_number, episode_number);
CREATE INDEX idx_episode_numbering_absolute
    ON episode_numbering (source, absolute_number);

-- Sub/dub awareness (§6.2).
--
-- Which language tracks an item is KNOWN to exist in — a catalogue fact, distinct
-- from what a particular file on disk happens to contain (that is `local_files`,
-- migration 0005). A user who watches anime subbed and refuses dubs needs this at
-- selection time in Phase 9, before any file exists.
CREATE TABLE media_language_tracks (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    -- 'audio' | 'subtitle'
    track_kind    TEXT    NOT NULL,
    -- ISO 639-1.
    language      TEXT    NOT NULL,
    -- A dub is an audio track not in the original language; a sub is a subtitle
    -- track. Stored rather than inferred, because "original language" is itself
    -- unreliable for co-productions.
    is_original   INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (media_item_id, track_kind, language),
    CHECK (track_kind IN ('audio', 'subtitle')),
    CHECK (is_original IN (0, 1))
);

CREATE INDEX idx_language_tracks_lookup ON media_language_tracks (track_kind, language);
