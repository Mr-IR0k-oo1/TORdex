-- Layer 1 — Collections.
-- A row per collection attempt. The "session" grouping (Layer 2) will sit on
-- top of this and aggregate multiple attempts under a single session id.
-- For now, each attempt is its own logical collection.

CREATE TABLE IF NOT EXISTS collections (
    id               TEXT        PRIMARY KEY,
    source_id        TEXT        NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    idempotency_key  TEXT,
    started_at       TIMESTAMPTZ NOT NULL,
    completed_at     TIMESTAMPTZ,
    status           TEXT        NOT NULL,
    collector_used   TEXT        NOT NULL,
    final_url        TEXT,
    content_type     TEXT,
    byte_count       BIGINT      NOT NULL DEFAULT 0,
    http_status      INTEGER,
    error_message    TEXT,
    body             BYTEA,
    metadata         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT collections_status_chk CHECK (
        status IN ('pending','running','succeeded','failed','cancelled','rate_limited')
    ),
    CONSTRAINT collections_collector_chk CHECK (
        collector_used IN ('http','browser_lightpanda','browser_chromium')
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS collections_source_idempotency_uniq
    ON collections (source_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS collections_source_id_started_at_idx
    ON collections (source_id, started_at DESC);
CREATE INDEX IF NOT EXISTS collections_status_idx
    ON collections (status);