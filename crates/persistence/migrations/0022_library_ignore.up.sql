-- 0022 — "Not a film or episode" (SPEC.md Phase 12's review queue). A home video or a
-- clip in a library folder is answered once: set here, it never returns to the queue,
-- across rescans and relaunches. Clearing it puts the file back.
ALTER TABLE local_files ADD COLUMN ignored_at TEXT;
