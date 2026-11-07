DROP INDEX IF EXISTS idx_signals_profile;
DROP TABLE IF EXISTS signals;
ALTER TABLE watch_events DROP COLUMN stopped_at_seconds;
