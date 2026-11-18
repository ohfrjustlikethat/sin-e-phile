-- 0029 — The item-item matrix (Phase 17, Layer 2): each title's best neighbours by who
-- loved both, built on this machine from MovieLens' ratings (ADR-0019: never shipped).
-- Rebuilt whole when it is rebuilt, so it keeps no history; a title that goes takes its
-- rows with it. About 18,500 titles with 50 neighbours each.
CREATE TABLE item_neighbours (
    media_item_id INTEGER NOT NULL REFERENCES media_items (id) ON DELETE CASCADE,
    neighbour_id  INTEGER NOT NULL REFERENCES media_items (id) ON DELETE CASCADE,
    score         REAL    NOT NULL,
    PRIMARY KEY (media_item_id, neighbour_id)
) WITHOUT ROWID;
