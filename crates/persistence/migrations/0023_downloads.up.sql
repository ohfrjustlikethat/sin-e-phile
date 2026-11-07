-- 0023 — The download queue (SPEC.md Phase 13). One row per download, whichever backend
-- moves the bytes (librqbit by default, the user's own qBittorrent when configured,
-- ADR-0029), so the queue, its order and its history survive a restart.
CREATE TABLE downloads (
    id            INTEGER PRIMARY KEY,
    -- What it is, when known: a finished download files into the library already matched.
    media_item_id INTEGER REFERENCES media_items(id) ON DELETE SET NULL,
    title         TEXT    NOT NULL,
    -- How to fetch it: a magnet link, or "infohash:<40 hex>".
    source        TEXT    NOT NULL,
    -- A .torrent's own bytes, when that is how it came.
    torrent       BLOB,
    backend       TEXT    NOT NULL DEFAULT 'librqbit',
    -- The backend's own name for it (the info hash for both backends).
    backend_id    TEXT,
    -- Higher first.
    priority      INTEGER NOT NULL DEFAULT 0,
    -- queued | active | paused | done | failed | cancelled
    state         TEXT    NOT NULL DEFAULT 'queued',
    total_bytes   INTEGER,
    done_bytes    INTEGER NOT NULL DEFAULT 0,
    -- This download's own limit, KiB/s; NULL is the global limit only.
    limit_kib     INTEGER,
    -- Do not start before this moment (UTC, ISO 8601): "after midnight".
    not_before    TEXT,
    -- Why it was queued, in one sentence (the advisor's, or the user's).
    reason        TEXT,
    error         TEXT,
    -- The finished video file. The user's own disk, like local_files.path.
    file_path     TEXT,
    added_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    finished_at   TEXT,
    CHECK (state IN ('queued', 'active', 'paused', 'done', 'failed', 'cancelled')),
    CHECK (backend IN ('librqbit', 'qbittorrent')),
    CHECK (limit_kib IS NULL OR limit_kib > 0)
);

CREATE INDEX idx_downloads_state ON downloads (state, priority DESC, id);
