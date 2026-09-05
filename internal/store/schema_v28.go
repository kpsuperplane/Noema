package store

// schemaV28SQL stores explicit web-provider choices and public URLs returned by web tools.
const schemaV28SQL = `
CREATE TABLE provider_capability_bindings (
    binding_id TEXT PRIMARY KEY,
    tool_name TEXT NOT NULL CHECK (tool_name IN ('web.search', 'web.fetch', 'web.browse')),
    capability_id TEXT NOT NULL CHECK (capability_id = tool_name),
    provider_account_id TEXT NOT NULL REFERENCES provider_accounts(provider_account_id) ON DELETE CASCADE,
    route_position INTEGER NOT NULL DEFAULT 0 CHECK (route_position >= 0),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK (route_position = 0 OR tool_name = 'web.browse'),
    UNIQUE(tool_name, capability_id, route_position),
    UNIQUE(tool_name, capability_id, provider_account_id)
) STRICT;

CREATE TABLE observed_urls (
    normalized_url TEXT PRIMARY KEY CHECK (length(normalized_url) BETWEEN 1 AND 2048),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('search_result', 'fetched_link', 'browser_link')),
    source_event_reference TEXT NOT NULL CHECK (length(source_event_reference) BETWEEN 1 AND 500),
    first_observed_at_ms INTEGER NOT NULL,
    last_observed_at_ms INTEGER NOT NULL
) STRICT;
`
