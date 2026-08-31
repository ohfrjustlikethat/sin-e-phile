-- 0004 — Profiles, watch history, collections, local files, and settings.
--
-- Everything here is the USER's data, which changes the rules: the catalogue can
-- be rebuilt from datasets, but this cannot. It is what the portable-archive
-- export in Phase 3 and the backup-on-migrate both exist to protect.

CREATE TABLE profiles (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL UNIQUE,
    -- Multiple profiles share one catalogue but not one taste model. Phase 15
    -- keys everything on profile_id, so it must exist from the start.
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT    NOT NULL DEFAULT (datetime('now')),
    CHECK (is_default IN (0, 1))
);

-- Exactly one default. A partial unique index is the cheapest way to say that in
-- SQLite, and it makes "two defaults" unrepresentable rather than merely unlikely.
CREATE UNIQUE INDEX idx_profiles_one_default ON profiles (is_default) WHERE is_default = 1;

-- Every discrete watching event, append-only.
--
-- Deliberately an event log rather than a mutable "watched" flag: Phase 15's
-- recommender needs to know that you watched something three times in 2019 and
-- never since, and a boolean throws that away permanently. Storage is trivial;
-- the information is not recoverable once discarded.
CREATE TABLE watch_events (
    id             INTEGER PRIMARY KEY,
    profile_id     INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id  INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    started_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    ended_at       TEXT,
    -- Seconds actually watched, which is not ended_at - started_at when the user
    -- pauses, seeks, or leaves it running. Phase 15 weights by this.
    watched_seconds INTEGER NOT NULL DEFAULT 0,
    -- Did it reach the completion threshold. Stored rather than derived because
    -- the threshold may change and past judgements should not silently move.
    completed      INTEGER NOT NULL DEFAULT 0,
    CHECK (completed IN (0, 1))
);

CREATE INDEX idx_watch_events_profile ON watch_events (profile_id, started_at DESC);
CREATE INDEX idx_watch_events_item    ON watch_events (media_item_id);

-- Current resume position — mutable, one row per (profile, item), unlike the log.
CREATE TABLE playback_positions (
    profile_id      INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id   INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    position_seconds INTEGER NOT NULL,
    duration_seconds INTEGER,
    -- Chosen audio and subtitle track, so resuming restores the whole state and
    -- not merely the timestamp.
    audio_language   TEXT,
    subtitle_language TEXT,
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (profile_id, media_item_id)
);

CREATE INDEX idx_playback_positions_recent ON playback_positions (profile_id, updated_at DESC);

CREATE TABLE watchlist_items (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    added_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    -- Why it was added. SPEC.md's discovery-first framing wants the watchlist to
    -- remember its own reasons, so a rail can say "you added this after Ran".
    reason        TEXT,
    PRIMARY KEY (profile_id, media_item_id)
);

CREATE INDEX idx_watchlist_added ON watchlist_items (profile_id, added_at DESC);

-- User-made collections, and the curated ones the app ships.
CREATE TABLE collections (
    id          INTEGER PRIMARY KEY,
    profile_id  INTEGER          REFERENCES profiles(id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    description TEXT,
    -- A NULL profile_id with is_curated = 1 is an app-shipped collection, visible
    -- to every profile.
    is_curated  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    CHECK (is_curated IN (0, 1)),
    CHECK (is_curated = 1 OR profile_id IS NOT NULL)
);

CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL REFERENCES collections(id)  ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id)  ON DELETE CASCADE,
    position      INTEGER,
    note          TEXT,
    PRIMARY KEY (collection_id, media_item_id)
);

-- Files on disk. Phase 12 scans and identifies; this is where the result lands.
CREATE TABLE local_files (
    id             INTEGER PRIMARY KEY,
    path           TEXT    NOT NULL UNIQUE,
    size_bytes     INTEGER NOT NULL,
    -- Filesystem mtime, so a rescan can skip unchanged files without hashing them.
    modified_at    TEXT,
    -- Populated lazily and only when needed — hashing a library of 4GB files is
    -- expensive, and path+size+mtime settles the overwhelming majority of cases.
    content_hash   TEXT,
    duration_seconds INTEGER,
    video_codec    TEXT,
    audio_codec    TEXT,
    width          INTEGER,
    height         INTEGER,
    scanned_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_local_files_hash ON local_files (content_hash);

-- The identification result, kept SEPARATE from the file itself.
--
-- Phase 12's whole exit criterion is a false-confident rate under 1%, which is
-- only measurable if a match carries its confidence and its method. Folding this
-- into local_files would make "the file" and "our guess about the file"
-- indistinguishable, and re-identifying after a parser improvement impossible.
CREATE TABLE local_file_matches (
    local_file_id INTEGER PRIMARY KEY REFERENCES local_files(id) ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL    REFERENCES media_items(id) ON DELETE CASCADE,
    confidence    REAL    NOT NULL,
    -- 'filename' | 'hash' | 'metadata' | 'manual'
    method        TEXT    NOT NULL,
    -- Set when the user corrects a wrong match. Never overwritten by a rescan:
    -- a human decision outranks the parser permanently.
    is_confirmed  INTEGER NOT NULL DEFAULT 0,
    matched_at    TEXT    NOT NULL DEFAULT (datetime('now')),

    CHECK (confidence >= 0.0 AND confidence <= 1.0),
    CHECK (method IN ('filename', 'hash', 'metadata', 'manual')),
    CHECK (is_confirmed IN (0, 1))
);

CREATE INDEX idx_local_file_matches_item ON local_file_matches (media_item_id);

-- Source addon configuration (Phase 6). Ships EMPTY — SPEC.md §2.1 forbids this
-- table having any seeded content whatsoever, and the posture guard enforces
-- that no URL reaches the repository at all.
CREATE TABLE sources_config (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    -- Opaque to this crate: Phase 6 defines the manifest shape.
    manifest    TEXT    NOT NULL,
    is_enabled  INTEGER NOT NULL DEFAULT 1,
    priority    INTEGER NOT NULL DEFAULT 0,
    added_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    CHECK (is_enabled IN (0, 1))
);

-- Key-value settings. Deliberately untyped text: settings are read once at
-- startup and written by a settings screen, so a schema per setting buys nothing
-- and costs a migration every time one is added.
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
