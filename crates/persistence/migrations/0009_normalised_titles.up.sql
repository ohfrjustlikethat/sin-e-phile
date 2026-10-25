-- 0009 — A normalised form of every title, for matching.
--
-- WHY A COLUMN RATHER THAN A FUNCTION. Matching AniList to the catalogue compares
-- titles with punctuation and casing removed — "Fullmetal Alchemist: Brotherhood"
-- and "Fullmetal Alchemist Brotherhood" are the same title written twice. SQLite
-- cannot compute that: it has no such function, and the rule lives in Rust
-- (tools/ingest/src/matching.rs::normalise) where it is tested.
--
-- The alternative is fetching candidates by raw title and normalising in Rust, which
-- finds only the titles that already agree — precisely the easy cases the normaliser
-- was written because they are NOT the whole problem.
--
-- Phase 5 wants this too: its exact-title short-circuit has to be robust to
-- punctuation, and a 100% top-1 rate over a fixture corpus will not survive
-- otherwise.
--
-- Populated by `ingest normalise`, not here: the rule is Rust and duplicating it in
-- SQL would create two definitions that silently drift.
ALTER TABLE titles ADD COLUMN normalised TEXT;

-- Deliberately NOT unique. Two different films genuinely share a normalised title,
-- and the matcher treats that as ambiguity to refuse rather than a constraint to
-- violate.
CREATE INDEX idx_titles_normalised ON titles (normalised);
