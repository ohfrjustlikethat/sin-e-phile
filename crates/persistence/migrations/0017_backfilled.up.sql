-- 0017 — Rows created by search-triggered backfill (ADR-0030 layer 3, Phase 6).
--
-- When a search finds nothing and a source finds the film anyway, a minimal row is
-- created from what the source reported, so the next search finds it. Such a row is a
-- GUESS — a title and a year from one source — and it must be told apart from the
-- authoritative catalogue, so that a bulk refresh reconciles it rather than adding a
-- second copy, and so that a later match to a real IMDb id merges it (Phase 12).
--
-- The value is the backend id that reported it. NULL, the default, is every row the
-- datasets produced.
ALTER TABLE media_items ADD COLUMN backfilled_from TEXT;

-- Reconciliation reads exactly the backfilled rows; partial, so it costs nothing for
-- the 2.7 million that are not.
CREATE INDEX idx_media_items_backfilled ON media_items (id) WHERE backfilled_from IS NOT NULL;
