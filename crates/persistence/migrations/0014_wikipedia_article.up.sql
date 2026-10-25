-- 0014 — Where a catalogue item's Wikipedia article lives (ADR-0033).
--
-- Phase 5 found that the catalogue held 0 synopses in 855,703 core items, so every
-- embedded document was title, year, genres and cast — and semantic search over it
-- behaved like a fuzzy title matcher. Wikipedia lead extracts are the text source,
-- joined through Wikidata's IMDb-id property P345.
--
-- A TABLE RATHER THAN A NEW `external_ids` SOURCE. `external_ids.source` carries a
-- CHECK constraint, and SQLite can only widen one by rebuilding the table — 3.2 million
-- rows to record a mapping that is not an identity anyway: an article title is where
-- prose lives, not another name for the work.
--
-- The extract itself goes to `media_items.synopsis`, which already exists and is what
-- the document builder reads. This table records the mapping and WHETHER THE FETCH HAS
-- HAPPENED, which is what makes a multi-hour run resumable.
CREATE TABLE wikipedia_article (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items (id) ON DELETE CASCADE,
    -- The article title, not its URL: the extracts API takes titles.
    article       TEXT    NOT NULL,
    -- NULL until the extract has been fetched. A row here with no fetch is work queued.
    fetched_at    TEXT
);

-- Resumption reads exactly this: what is mapped but not yet fetched. Partial, so it
-- indexes the shrinking queue rather than the whole table.
CREATE INDEX idx_wikipedia_pending ON wikipedia_article (media_item_id) WHERE fetched_at IS NULL;
