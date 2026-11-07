-- 0024 — Profiles proper (SPEC.md Phase 14). Migration 0004 made `profiles` and keyed
-- everything personal on it; this gives a profile what the picker and the kids profile
-- need. The PIN is not here: it is a wrapped secret in profile_settings (key 'pin'),
-- like the keys, so a copied data folder carries no PIN at all.
ALTER TABLE profiles ADD COLUMN avatar_colour TEXT;
-- A kids profile shows only what is positively known to suit children (fail closed).
ALTER TABLE profiles ADD COLUMN is_kids INTEGER NOT NULL DEFAULT 0 CHECK (is_kids IN (0, 1));
-- The highest certification allowed when the profile has a TMDB key: 'U', 'PG', '12'.
ALTER TABLE profiles ADD COLUMN kids_limit TEXT;
-- The picker lists the most recently used first.
ALTER TABLE profiles ADD COLUMN last_used_at TEXT;
