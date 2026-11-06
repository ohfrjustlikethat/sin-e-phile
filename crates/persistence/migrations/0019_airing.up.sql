-- 0019 — When a series' next episode airs (SPEC.md Phase 11, ADR-0030).
--
-- AniList knows an episode is scheduled before IMDb lists it, so "Episode 7 airs
-- Thursday" must not need an `episodes` row to exist. One row per series: the next
-- airing only, replaced as the schedule moves on. `episode_number` is AniList's own,
-- per-entry number (ADR-0031: never converted into a season and episode by arithmetic).
CREATE TABLE airing_schedule (
    series_id      INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    episode_number INTEGER NOT NULL,
    -- Unix seconds, UTC.
    airs_at        INTEGER NOT NULL,
    updated_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
