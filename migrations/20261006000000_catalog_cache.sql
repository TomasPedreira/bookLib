CREATE TABLE catalog_cache (
    cache_key TEXT PRIMARY KEY NOT NULL,
    status INTEGER NOT NULL CHECK (status IN (200, 404)),
    payload TEXT,
    fetched_at INTEGER NOT NULL,
    fresh_until INTEGER NOT NULL,
    stale_until INTEGER NOT NULL
);
CREATE INDEX catalog_cache_expiry ON catalog_cache(stale_until);
