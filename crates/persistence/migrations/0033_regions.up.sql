-- 0033 — Regions of the catalogue (Phase 18, ADR-0055): the country a title comes from and the
-- film movement it belongs to, from Wikidata (P495 and P135), so the discovery engine can name a
-- blind spot ("Iranian New Wave") and a stretch ("but from Japan"). The IMDb datasets carry
-- neither. English names, as Wikidata labels them, not ISO codes: the Soviet Union and West
-- Germany have none, and are where much of cinema came from.
CREATE TABLE title_regions (
    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    axis          TEXT    NOT NULL,
    name          TEXT    NOT NULL,
    PRIMARY KEY (media_item_id, axis, name),
    CHECK (axis IN ('country', 'movement'))
) WITHOUT ROWID;

-- Which titles Wikidata has been asked about, answered or not. A title it knows nothing of is not
-- asked again each hour; and "you have never watched anything from Iran" may be said only when
-- every title the profile has met was asked.
CREATE TABLE regions_asked (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    asked_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);
