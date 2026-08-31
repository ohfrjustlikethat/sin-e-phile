-- 0006 — Which tier a title is in.
--
-- Phase 4's two-tier scope (author's ruling, 2026-09-01): every kept-type title is
-- indexed and findable; only the popular core carries cast, crew, alternative titles
-- and embeddings. Measured: enriching everything would be a 26.53 GB database, and
-- the core tier brings it to 3.11 GB.
--
-- STORED, NOT DERIVED. The rule is "10+ votes, or unrated and released within two
-- years", and the second half moves with the calendar — so recomputing it later
-- would silently give a different answer than the ingestion did, and Phase 5 would
-- look for embeddings on titles that never got one.
ALTER TABLE media_items ADD COLUMN in_core INTEGER NOT NULL DEFAULT 0;

-- Phase 5 asks "which titles have embeddings?" and Phase 17 asks "what can I
-- actually recommend?". Both are this predicate, over the whole table.
CREATE INDEX idx_media_items_core ON media_items (in_core, kind) WHERE in_core = 1;
