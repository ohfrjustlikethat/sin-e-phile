-- 0005 — Ingestion job state.
--
-- Lives in the APP database rather than its own file (author's ruling, 2026-09-01).
-- SPEC.md §2.4 promises the app is a folder you can copy; two database files is the
-- thing most likely to make that promise false, because a half-copied folder then
-- has two ways to be inconsistent instead of none. A table nobody reads costs
-- nothing.

CREATE TABLE ingest_jobs (
    id         INTEGER PRIMARY KEY,
    -- 'imdb', 'anilist', 'movielens'. One row per RUN, not per source: a second
    -- run of the same source is a new job, so history is preserved and a resumed
    -- run can be told apart from a fresh one.
    name       TEXT    NOT NULL,
    status     TEXT    NOT NULL DEFAULT 'running',
    started_at TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT    NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT,
    -- Why it stopped, when it stopped badly. Kept so a resumed run can report what
    -- it is resuming FROM, which is the difference between "resuming" and "starting
    -- again and hoping".
    error      TEXT,

    CHECK (status IN ('running', 'complete', 'failed'))
);

CREATE INDEX idx_ingest_jobs_name ON ingest_jobs (name, started_at DESC);

-- One row per step per job. A step is the unit of resumption.
CREATE TABLE ingest_steps (
    job_id      INTEGER NOT NULL REFERENCES ingest_jobs(id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    ordinal     INTEGER NOT NULL,
    status      TEXT    NOT NULL DEFAULT 'pending',

    -- OPAQUE to the runner. A byte offset, a last-seen id, a page token — whatever
    -- the step needs to say "carry on from here". Deliberately untyped: the runner
    -- must not need to understand any particular dataset's idea of progress, or it
    -- would need changing for every new source.
    cursor      TEXT,

    -- For progress reporting only. `items_total` is NULL until the step knows, which
    -- for a streamed file is never — so the UI must handle an unknown total rather
    -- than assuming a percentage is always available.
    items_done  INTEGER NOT NULL DEFAULT 0,
    items_total INTEGER,

    started_at  TEXT,
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),

    PRIMARY KEY (job_id, name),
    CHECK (status IN ('pending', 'running', 'complete', 'failed'))
);

CREATE INDEX idx_ingest_steps_order ON ingest_steps (job_id, ordinal);
