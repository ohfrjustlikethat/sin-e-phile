DROP INDEX IF EXISTS idx_language_tracks_lookup;
DROP TABLE IF EXISTS media_language_tracks;

DROP INDEX IF EXISTS idx_episode_numbering_absolute;
DROP INDEX IF EXISTS idx_episode_numbering_seasonal;
DROP TABLE IF EXISTS episode_numbering;

DROP INDEX IF EXISTS idx_episodes_season;
DROP INDEX IF EXISTS idx_episodes_absolute;
DROP INDEX IF EXISTS idx_episodes_seasonal;
DROP TABLE IF EXISTS episodes;

DROP TABLE IF EXISTS seasons;
DROP TABLE IF EXISTS series;
