-- 0010 — Per-profile settings, and the only place a user's own API key lives.
--
-- ADR-0027: no TMDB key ever ships. Each user supplies their own, per profile, in
-- settings, under their own acceptance of TMDB's terms, and may remove it at any time.
--
-- WHY A SECOND TABLE RATHER THAN A COLUMN ON `settings`. `settings` is global — one
-- row per key, no profile. Multiple profiles share one catalogue but not one taste
-- model (migration 0004), and they must not share a TMDB key either: a household
-- profile is a different person, and their key is theirs. Encoding the profile into
-- the global key string ("profile.3.tmdb_key") would work and would make every query
-- a string-prefix match, with no foreign key and nothing to cascade on delete.
CREATE TABLE profile_settings (
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    key        TEXT    NOT NULL,
    -- Opaque bytes, not text. A secret is stored wrapped (see
    -- crates/persistence/src/secrets.rs) and wrapped bytes are not valid UTF-8.
    -- Non-secret settings store their UTF-8 in here too; the column is the union.
    value      BLOB    NOT NULL,
    -- Whether `value` is wrapped. Stored rather than inferred from the key name,
    -- because inferring it means a renamed key silently returns ciphertext as if it
    -- were a value, and the first thing that would notice is a failing API call.
    is_secret  INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT    NOT NULL DEFAULT (datetime('now')),

    PRIMARY KEY (profile_id, key),
    CHECK (is_secret IN (0, 1))
);

-- ON DELETE CASCADE above is the point: deleting a profile must take its key with it.
-- A key that outlived the profile it belonged to would be a secret nobody owns.
