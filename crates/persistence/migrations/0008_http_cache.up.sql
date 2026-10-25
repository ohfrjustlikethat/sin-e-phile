-- 0008 — The persistent response cache.
--
-- SPEC.md Phase 4 requires "a persistent response cache with sensible TTLs per
-- resource type, and graceful offline behaviour".
--
-- IN THE APP DATABASE, not a cache directory. Same reasoning as ingest_jobs: §2.4
-- promises the app is one folder you can copy, and a second store is another way for
-- a half-copied folder to be inconsistent. A cache that survives the copy is also
-- simply better — moving to a new machine should not mean re-fetching everything.
CREATE TABLE http_cache (
    -- The full request URL, minus any API key. A key must never reach this table:
    -- the cache is copied with the app folder, and a key in it would travel too.
    -- ADR-0027 makes keys per-user and per-profile, and this is where that would
    -- leak if it were going to.
    url         TEXT    PRIMARY KEY,
    -- Which service, so a TTL policy and a purge can be applied per source.
    source      TEXT    NOT NULL,
    -- What kind of thing this is: 'detail', 'search', 'images', 'schedule'. TTLs
    -- differ by an order of magnitude between them — a film's cast does not change,
    -- an airing schedule changes weekly — so one global TTL would be wrong for
    -- everything.
    resource    TEXT    NOT NULL,
    body        TEXT    NOT NULL,
    status      INTEGER NOT NULL,
    -- HTTP validators, so a refresh can be conditional and cost nothing when the
    -- resource has not changed.
    etag        TEXT,
    last_modified TEXT,

    fetched_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    -- When this stops being fresh. NOT when it stops being USABLE: an expired entry
    -- is still served when the network is unreachable, because stale data beats an
    -- empty screen. Graceful offline behaviour is the requirement, and it is only
    -- possible if expiry and deletion are different things.
    expires_at  TEXT    NOT NULL,

    CHECK (status >= 100 AND status < 600)
);

-- Purging by age or by source, and finding what is stale enough to refresh.
CREATE INDEX idx_http_cache_expiry ON http_cache (expires_at);
CREATE INDEX idx_http_cache_source ON http_cache (source, resource);
