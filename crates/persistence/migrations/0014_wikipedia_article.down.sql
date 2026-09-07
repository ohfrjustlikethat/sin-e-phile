-- Reverses 0014. The synopses it wrote into media_items are left alone deliberately:
-- they are catalogue data, and dropping the mapping is not a reason to throw away text
-- that took hours to fetch.
DROP INDEX IF EXISTS idx_wikipedia_pending;
DROP TABLE IF EXISTS wikipedia_article;
