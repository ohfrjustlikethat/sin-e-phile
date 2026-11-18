-- 0030 — Meaning for what is new (ADR-0054 step 2): each title the catalogue gained after the
-- published meaning file, asked at launch for a synopsis and given a vector in the companion
-- graph beside the published one. `asked_at` stops a title nobody describes being asked at
-- every launch; `embedded_at` stops it being embedded twice.
CREATE TABLE new_meaning (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items (id) ON DELETE CASCADE,
    -- The last time a synopsis was asked for; NULL when it had one already.
    asked_at      TEXT,
    -- Where its synopsis came from: 'tmdb', 'anilist' or 'wikipedia'; NULL while none has.
    source        TEXT,
    -- When its vector went into the companion graph.
    embedded_at   TEXT
) WITHOUT ROWID;
