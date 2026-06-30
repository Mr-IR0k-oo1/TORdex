-- Phase 1 — Core Domain
-- Source → Service → CollectionSession → Artifact → Evidence → Knowledge → Relationship

CREATE TABLE IF NOT EXISTS services (
    id              TEXT        PRIMARY KEY,
    display_name    TEXT        NOT NULL,
    locator         TEXT        NOT NULL,
    kind            TEXT        NOT NULL DEFAULT 'website',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT services_kind_chk CHECK (
        kind IN ('website','api','rss_feed','document','repository','local_file','onion_service')
    ),
    CONSTRAINT services_locator_nonempty CHECK (length(locator) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS services_locator_uniq ON services (locator);

CREATE TABLE IF NOT EXISTS collection_sessions (
    id              TEXT        PRIMARY KEY,
    service_id      TEXT        NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    status          TEXT        NOT NULL DEFAULT 'pending',
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    error_message   TEXT,
    CONSTRAINT sessions_status_chk CHECK (
        status IN ('pending','running','completed','failed')
    )
);

CREATE INDEX IF NOT EXISTS sessions_service_id_idx ON collection_sessions (service_id);
CREATE INDEX IF NOT EXISTS sessions_status_idx ON collection_sessions (status);

CREATE TABLE IF NOT EXISTS artifacts (
    id              TEXT        PRIMARY KEY,
    session_id      TEXT        NOT NULL REFERENCES collection_sessions(id) ON DELETE CASCADE,
    kind            TEXT        NOT NULL,
    content_type    TEXT,
    byte_count      BIGINT      NOT NULL DEFAULT 0,
    sha256          TEXT        NOT NULL,
    storage_path    TEXT        NOT NULL,
    metadata        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT artifacts_kind_chk CHECK (
        kind IN ('html','pdf','image','json','text','binary','rss','xml','other')
    )
);

CREATE INDEX IF NOT EXISTS artifacts_session_id_idx ON artifacts (session_id);

CREATE TABLE IF NOT EXISTS evidence (
    id              TEXT        PRIMARY KEY,
    artifact_id     TEXT        NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    kind            TEXT        NOT NULL DEFAULT 'sha256',
    value           JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS evidence_artifact_id_idx ON evidence (artifact_id);

CREATE TABLE IF NOT EXISTS events (
    id              TEXT        PRIMARY KEY,
    session_id      TEXT        REFERENCES collection_sessions(id) ON DELETE CASCADE,
    topic           TEXT        NOT NULL,
    payload         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    occurred_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS events_session_id_idx ON events (session_id);
CREATE INDEX IF NOT EXISTS events_topic_idx ON events (topic);
CREATE INDEX IF NOT EXISTS events_occurred_at_idx ON events (occurred_at DESC);

CREATE TABLE IF NOT EXISTS knowledge_objects (
    id              TEXT        PRIMARY KEY,
    session_id      TEXT        NOT NULL REFERENCES collection_sessions(id) ON DELETE CASCADE,
    kind            TEXT        NOT NULL,
    content         JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS knowledge_session_id_idx ON knowledge_objects (session_id);
CREATE INDEX IF NOT EXISTS knowledge_kind_idx ON knowledge_objects (kind);

CREATE TABLE IF NOT EXISTS relationships (
    id              TEXT        PRIMARY KEY,
    source_id       TEXT        NOT NULL REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    target_id       TEXT        NOT NULL REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    kind            TEXT        NOT NULL,
    metadata        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS rel_source_id_idx ON relationships (source_id);
CREATE INDEX IF NOT EXISTS rel_target_id_idx ON relationships (target_id);
CREATE INDEX IF NOT EXISTS rel_kind_idx ON relationships (kind);
