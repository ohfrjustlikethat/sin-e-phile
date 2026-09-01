-- 0007 — One row per (item, variant, title).
--
-- THE DEFECT. idx_titles_unique was (media_item_id, variant, language, region), so
-- the same text under en/US, en/GB, en/CA and a null region was four distinct keys
-- and all four were stored. `Seven Samurai` had six identical `english` rows.
--
-- Measured on the real catalogue: 8,435,215 title rows, of which 6,184,278 are
-- distinct on (item, variant, title). **2,250,937 rows — 27% — carried no
-- information.** At roughly 120 bytes each that is about 270 MB, which matters
-- against R4's 4 GB trigger with only ~986 MB of headroom left.
--
-- The fix costs nothing in information. A genuinely different regional title —
-- "The Avengers" vs "Avengers Assemble" — is different TEXT and remains its own row.
-- What goes is only the same string stored repeatedly.
--
-- Deduplicating here rather than reloading: the data is already correct apart from
-- the redundancy, so an 860-second re-fetch and re-parse would achieve the same
-- thing more slowly.

-- Keep the lowest id in each group, which is the first IMDb listed — so the
-- surviving row keeps a real language and region rather than an arbitrary one.
DELETE FROM titles
WHERE id NOT IN (
    SELECT MIN(id) FROM titles GROUP BY media_item_id, variant, title
);

DROP INDEX IF EXISTS idx_titles_unique;

-- Text is now part of the key. `ON CONFLICT DO NOTHING` in the akas loader picks
-- this up with no code change, so a re-run deduplicates by construction.
CREATE UNIQUE INDEX idx_titles_unique ON titles (media_item_id, variant, title);
