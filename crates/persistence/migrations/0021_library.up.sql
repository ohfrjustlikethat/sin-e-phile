-- 0021 — The local library (SPEC.md Phase 12). local_files and local_file_matches
-- were designed in 0004; this adds where files live, what the probe found, and what
-- the review queue and the user's decisions need.

-- The folders the user pointed the app at. Their own paths on their own machine, as
-- machine-specific as local_files.path (the portability test allows both by name).
CREATE TABLE library_folders (
    id         INTEGER PRIMARY KEY,
    path       TEXT NOT NULL UNIQUE,
    added_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    scanned_at TEXT
);

-- Which folder a file came from: removing a folder removes its files. By trigger rather
-- than REFERENCES: SQLite cannot drop a column that carries a foreign key, and the down
-- migration must be able to.
ALTER TABLE local_files ADD COLUMN folder_id INTEGER;
CREATE TRIGGER library_folder_removed AFTER DELETE ON library_folders
BEGIN
    DELETE FROM local_files WHERE folder_id = old.id;
END;
-- Set when a file disappears. A file that reappears elsewhere with the same size and
-- content is a move, and keeps its row and its match (Phase 12 E4); one that does not
-- is removed at the end of the scan.
ALTER TABLE local_files ADD COLUMN missing_since TEXT;
-- What FFprobe found beyond 0004's columns: audio languages (comma-separated ISO
-- 639-1) and the container's own title tag.
ALTER TABLE local_files ADD COLUMN audio_languages TEXT;
ALTER TABLE local_files ADD COLUMN container_title TEXT;
-- NULL until probed. Probing is the slow part of a scan, so it is done once.
ALTER TABLE local_files ADD COLUMN probed_at TEXT;

CREATE INDEX idx_local_files_folder ON local_files (folder_id);
-- Move detection asks "a missing file of this size?".
CREATE INDEX idx_local_files_size ON local_files (size_bytes);

-- A file holding several episodes ("S01E01E02") matches the first, and says how many.
ALTER TABLE local_file_matches ADD COLUMN covers INTEGER NOT NULL DEFAULT 1;

-- What the review queue offers for a file it was not sure about, best first.
CREATE TABLE review_candidates (
    local_file_id INTEGER NOT NULL REFERENCES local_files(id)  ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    score         REAL    NOT NULL,
    rank          INTEGER NOT NULL,
    PRIMARY KEY (local_file_id, media_item_id),
    CHECK (score >= 0.0 AND score <= 1.0)
);

-- The user's decisions, keyed by what a name says (the comparable title, the year, and
-- whether it is an episode), so one decision settles every file named the same way:
-- a whole series' episodes, or a film's copies.
CREATE TABLE match_memory (
    pattern       TEXT    PRIMARY KEY,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    decided_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
