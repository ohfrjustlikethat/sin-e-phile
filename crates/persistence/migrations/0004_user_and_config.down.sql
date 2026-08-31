DROP TABLE IF EXISTS settings;
DROP TABLE IF EXISTS sources_config;

DROP INDEX IF EXISTS idx_local_file_matches_item;
DROP TABLE IF EXISTS local_file_matches;
DROP INDEX IF EXISTS idx_local_files_hash;
DROP TABLE IF EXISTS local_files;

DROP TABLE IF EXISTS collection_items;
DROP TABLE IF EXISTS collections;

DROP INDEX IF EXISTS idx_watchlist_added;
DROP TABLE IF EXISTS watchlist_items;

DROP INDEX IF EXISTS idx_playback_positions_recent;
DROP TABLE IF EXISTS playback_positions;

DROP INDEX IF EXISTS idx_watch_events_item;
DROP INDEX IF EXISTS idx_watch_events_profile;
DROP TABLE IF EXISTS watch_events;

DROP INDEX IF EXISTS idx_profiles_one_default;
DROP TABLE IF EXISTS profiles;
