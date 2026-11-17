-- 0028 — The one file of a torrent a download wants (Phase 16, the author's brief item 1).
-- A source often points at one episode inside a season pack (its file index); without
-- this, a download fetched the whole pack. NULL: the whole torrent, as before.
ALTER TABLE downloads ADD COLUMN file_index INTEGER;
