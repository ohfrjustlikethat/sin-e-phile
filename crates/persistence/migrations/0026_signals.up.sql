-- 0026 — What a profile does, for the taste model (SPEC.md Phase 15).
--
-- A viewing already lands in watch_events (played seconds, completed). What it lacked is
-- WHERE it stopped: abandoning at 5 minutes and at 80 mean different things, so the point
-- is kept. Rewatches and the time of day are derived from the dated log, not stored twice.
ALTER TABLE watch_events ADD COLUMN stopped_at_seconds INTEGER;

-- The lighter signals: a title opened from a search, and a detail page opened (whether it
-- was then played is read from watch_events). Local, never sent anywhere.
CREATE TABLE signals (
    id            INTEGER PRIMARY KEY,
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    kind          TEXT    NOT NULL,
    at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    CHECK (kind IN ('search', 'detail'))
);
CREATE INDEX idx_signals_profile ON signals (profile_id, kind, at);
