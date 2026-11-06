-- 0020 — The viewer's own rating of what they watched (SPEC.md Phase 11's end card,
-- ADR-0043; read by Phase 15's taste model as an explicit signal). One per profile and
-- title; rating again replaces it. 1 to 5.
CREATE TABLE ratings (
    profile_id    INTEGER NOT NULL REFERENCES profiles(id)    ON DELETE CASCADE,
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    stars         INTEGER NOT NULL,
    rated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (profile_id, media_item_id),
    CHECK (stars BETWEEN 1 AND 5)
);
