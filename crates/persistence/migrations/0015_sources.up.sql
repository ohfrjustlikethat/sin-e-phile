-- 0015 — The source backend registry and installed addons (SPEC.md Phase 6).
--
-- Global, not per profile: a source is where films come FROM, like the downloaded
-- files and the metadata cache that §15 Phase 14 also shares across profiles. What a
-- profile prefers among the results is Phase 9's scoring, not which sources exist.

-- One row per backend the app has ever registered, built-in or addon. The id is
-- `SourceBackend::id()`, which is why that id must never change once shipped.
CREATE TABLE source_backends (
    id              TEXT    PRIMARY KEY,
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    -- The user's order. Gaps are harmless; `reorder` rewrites 0..n.
    position        INTEGER NOT NULL,

    -- Health (§15 Phase 6: "per-backend health tracking"). Counts, not a score: what
    -- a score means is Phase 9's to decide, and counts can become any score later.
    successes       INTEGER NOT NULL DEFAULT 0,
    failures        INTEGER NOT NULL DEFAULT 0,
    timeouts        INTEGER NOT NULL DEFAULT 0,
    -- Exponentially weighted, so a backend that was slow last month and fast today
    -- reads as fast. NULL until it has answered once.
    latency_ms      REAL,
    last_error      TEXT,
    last_outcome_at TEXT
);

-- An addon is a backend whose definition the USER supplied: the manifest they
-- confirmed, kept exactly as validated, and the address it came from so it can be
-- refreshed. Removing the addon removes its registry row with it.
CREATE TABLE addons (
    backend_id   TEXT PRIMARY KEY REFERENCES source_backends (id) ON DELETE CASCADE,
    manifest_url TEXT NOT NULL,
    manifest     TEXT NOT NULL,
    installed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
