DROP TABLE neighbours_asked;

CREATE TABLE item_neighbours_movielens (
    media_item_id INTEGER NOT NULL REFERENCES media_items (id) ON DELETE CASCADE,
    neighbour_id  INTEGER NOT NULL REFERENCES media_items (id) ON DELETE CASCADE,
    score         REAL    NOT NULL,
    PRIMARY KEY (media_item_id, neighbour_id)
) WITHOUT ROWID;

INSERT INTO item_neighbours_movielens (media_item_id, neighbour_id, score)
    SELECT media_item_id, neighbour_id, score FROM item_neighbours WHERE source = 'movielens';

DROP TABLE item_neighbours;
ALTER TABLE item_neighbours_movielens RENAME TO item_neighbours;
