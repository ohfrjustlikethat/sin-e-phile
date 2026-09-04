-- Reverses 0010. Dropping the table takes every stored key with it, which is the
-- correct outcome for a down-migration: a secret must not survive the removal of the
-- thing that gave it meaning.
DROP TABLE IF EXISTS profile_settings;
