-- Phase 20 — TORdex Intelligence Kernel persistent store
-- Event-sourced kernel with object management and snapshots.

CREATE TABLE IF NOT EXISTS kernel_events (
    id              TEXT        PRIMARY KEY,
    aggregate_id    TEXT        NOT NULL,
    aggregate_type  TEXT        NOT NULL,
    event_type      TEXT        NOT NULL,
    version         BIGINT      NOT NULL,
    data            JSONB       NOT NULL DEFAULT '{}'::jsonb,
    metadata        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT kernel_events_version_chk CHECK (version > 0)
);

CREATE INDEX IF NOT EXISTS kernel_events_aggregate_id_idx ON kernel_events (aggregate_id);
CREATE INDEX IF NOT EXISTS kernel_events_aggregate_type_idx ON kernel_events (aggregate_type);
CREATE INDEX IF NOT EXISTS kernel_events_event_type_idx ON kernel_events (event_type);
CREATE INDEX IF NOT EXISTS kernel_events_timestamp_idx ON kernel_events (timestamp DESC);

CREATE TABLE IF NOT EXISTS kernel_snapshots (
    aggregate_id    TEXT        PRIMARY KEY,
    aggregate_type  TEXT        NOT NULL,
    version         BIGINT      NOT NULL,
    state           BYTEA       NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS kernel_snapshots_aggregate_type_idx ON kernel_snapshots (aggregate_type);

CREATE TABLE IF NOT EXISTS kernel_objects (
    id              TEXT        PRIMARY KEY,
    kind            TEXT        NOT NULL,
    label           TEXT        NOT NULL,
    data            BYTEA       NOT NULL DEFAULT ''::bytea,
    created_at      BIGINT      NOT NULL,
    updated_at      BIGINT      NOT NULL
);

CREATE INDEX IF NOT EXISTS kernel_objects_kind_idx ON kernel_objects (kind);
CREATE INDEX IF NOT EXISTS kernel_objects_label_idx ON kernel_objects (label);

CREATE TABLE IF NOT EXISTS kernel_object_links (
    id              TEXT        PRIMARY KEY,
    source_id       TEXT        NOT NULL REFERENCES kernel_objects(id) ON DELETE CASCADE,
    target_id       TEXT        NOT NULL REFERENCES kernel_objects(id) ON DELETE CASCADE,
    kind            TEXT        NOT NULL,
    created_at      BIGINT      NOT NULL
);

CREATE INDEX IF NOT EXISTS kernel_links_source_id_idx ON kernel_object_links (source_id);
CREATE INDEX IF NOT EXISTS kernel_links_target_id_idx ON kernel_object_links (target_id);
CREATE INDEX IF NOT EXISTS kernel_links_kind_idx ON kernel_object_links (kind);

CREATE TABLE IF NOT EXISTS kernel_storage (
    key             TEXT        PRIMARY KEY,
    value           BYTEA       NOT NULL,
    content_type    TEXT,
    created_at      BIGINT      NOT NULL
);

CREATE INDEX IF NOT EXISTS kernel_storage_key_prefix_idx ON kernel_storage (key text_pattern_ops);
