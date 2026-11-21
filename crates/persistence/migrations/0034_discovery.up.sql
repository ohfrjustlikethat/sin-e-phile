-- 0034 — The discovery engine's memory (Phase 18): which arm first put each title in front of a
-- profile, in which part of the day, and what the profile said of a recommendation. The bandit
-- learns from what each arm's titles came to (played, finished, abandoned, saved, rated, dismissed),
-- read against these rows; Home leaves out what was dismissed.
CREATE TABLE discovery_shown (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    arm           TEXT    NOT NULL,
    context       TEXT    NOT NULL,
    first_shown   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (profile_id, media_item_id),
    CHECK (arm IN ('familiar', 'stretch', 'bridge', 'blind_spot', 'canon', 'deep_cut')),
    CHECK (context IN ('day', 'evening', 'late'))
) WITHOUT ROWID;

-- "Not for me", "not right now", "already seen": one per profile and title, the latest kept.
CREATE TABLE dismissals (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    kind          TEXT    NOT NULL,
    context       TEXT    NOT NULL,
    at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (profile_id, media_item_id),
    CHECK (kind IN ('not_for_me', 'not_right_now', 'already_seen')),
    CHECK (context IN ('day', 'evening', 'late'))
) WITHOUT ROWID;
