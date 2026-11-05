-- 0016 — Links the user added themselves, for the direct HTTP backend (Phase 6).
--
-- SPEC.md §2.1 names "user-supplied M3U/direct HTTP" as one of the three legal
-- reference sources. Every row here was typed, pasted or imported by the user; the app
-- ships none and never fetches a list of them from anywhere.
CREATE TABLE direct_links (
    id       INTEGER PRIMARY KEY,
    title    TEXT    NOT NULL,
    year     INTEGER,
    -- One row per address: importing the same playlist twice updates, not duplicates.
    url      TEXT    NOT NULL UNIQUE,
    added_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
