-- 0018 — Subtitles (SPEC.md Phase 10): the nudge, the download quota, and the cache.

-- The viewer's manual correction, per file and per subtitle (Phase 10 E5: "the manual
-- nudge persists across sessions for the same file"). `file_key` names the media by
-- content where it can (the OpenSubtitles hash, so a renamed or moved file keeps its
-- nudge), else by address. `subtitle_key` says which subtitle: an embedded track, a
-- sidecar file, or a downloaded one. A nudge belongs to a pair, because another subtitle
-- for the same film is timed differently.
CREATE TABLE subtitle_nudges (
    file_key     TEXT    NOT NULL,
    subtitle_key TEXT    NOT NULL,
    offset_ms    INTEGER NOT NULL,
    updated_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (file_key, subtitle_key)
);

-- Downloads spent per provider per UTC day (Phase 10 E4: "the daily quota is respected
-- and never exceeded"). A download is reserved here BEFORE it is requested, by one
-- conditional UPDATE, so two at once cannot both take the last one. `server_remaining`
-- is what the provider last said is left, which can be lower than our own budget (the
-- quota is per account, and another app may share it).
CREATE TABLE subtitle_quota (
    provider         TEXT    NOT NULL,
    day              TEXT    NOT NULL,
    used             INTEGER NOT NULL DEFAULT 0,
    server_remaining INTEGER,
    PRIMARY KEY (provider, day),
    CHECK (used >= 0)
);

-- What was downloaded, so the same subtitle is never paid for twice. Keyed by the media
-- hash and language. `relative_path` is where the file was written, RELATIVE to ./data,
-- so it travels with a copied app folder (SPEC.md §2.4); the repository refuses an
-- absolute one.
CREATE TABLE subtitle_cache (
    media_hash TEXT NOT NULL,
    language   TEXT NOT NULL,
    provider   TEXT NOT NULL,
    file_id    TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (media_hash, language)
);
