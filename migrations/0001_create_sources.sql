-- Layer 0 — Sources.
-- Each row is a declarative description of something the platform can collect.
-- The locator format depends on `kind`: URLs for web/api/rss, owner/repo for
-- repositories, absolute paths for local files.

CREATE TABLE IF NOT EXISTS sources (
    id              TEXT        PRIMARY KEY,
    kind            TEXT        NOT NULL,
    display_name    TEXT        NOT NULL,
    locator         TEXT        NOT NULL,
    routing_policy  TEXT        NOT NULL DEFAULT 'auto',
    hints           JSONB       NOT NULL DEFAULT '{}'::jsonb,
    status          TEXT        NOT NULL DEFAULT 'active',
    tags            TEXT[]      NOT NULL DEFAULT ARRAY[]::TEXT[],
    metadata        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT sources_kind_chk CHECK (
        kind IN ('website','onion_service','api','repository','document','rss_feed','local_file','paper')
    ),
    CONSTRAINT sources_routing_policy_chk CHECK (
        routing_policy IN ('auto','http','browser')
    ),
    CONSTRAINT sources_status_chk CHECK (
        status IN ('active','paused','errored')
    ),
    CONSTRAINT sources_display_name_nonempty CHECK (length(display_name) > 0),
    CONSTRAINT sources_locator_nonempty CHECK (length(locator) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS sources_kind_locator_uniq
    ON sources (kind, locator);

CREATE INDEX IF NOT EXISTS sources_status_idx ON sources (status);
CREATE INDEX IF NOT EXISTS sources_kind_idx ON sources (kind);