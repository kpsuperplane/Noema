package store

// schemaV14SQL adds browser Web Push registrations and durable delivery state.
const schemaV14SQL = `
CREATE TABLE web_push_subscriptions (
    subscription_id TEXT PRIMARY KEY CHECK (subscription_id GLOB 'push_subscription:*'),
    owner_human_id TEXT NOT NULL CHECK (owner_human_id = 'human:local'),
    browser_session_hash BLOB NOT NULL
        REFERENCES browser_sessions(session_hash) ON DELETE CASCADE
        CHECK (length(browser_session_hash) = 32),
    endpoint TEXT NOT NULL UNIQUE CHECK (length(endpoint) BETWEEN 1 AND 2048),
    p256dh TEXT NOT NULL CHECK (length(p256dh) BETWEEN 40 AND 256),
    auth_secret TEXT NOT NULL CHECK (length(auth_secret) BETWEEN 16 AND 128),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX web_push_subscriptions_session
ON web_push_subscriptions(browser_session_hash, subscription_id);

CREATE TABLE web_push_deliveries (
    subscription_id TEXT NOT NULL
        REFERENCES web_push_subscriptions(subscription_id) ON DELETE CASCADE,
    event_key TEXT NOT NULL CHECK (length(event_key) BETWEEN 1 AND 256),
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 600),
    body TEXT NOT NULL CHECK (length(body) <= 2048),
    navigate_path TEXT NOT NULL CHECK (length(navigate_path) BETWEEN 1 AND 1024),
    urgency TEXT NOT NULL CHECK (urgency IN ('normal', 'high')),
    ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds BETWEEN 0 AND 604800),
    status TEXT NOT NULL CHECK (status IN ('pending', 'delivered', 'suppressed', 'failed')),
    available_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error_code TEXT CHECK (last_error_code IS NULL OR length(last_error_code) BETWEEN 1 AND 128),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (subscription_id, event_key)
) STRICT;
CREATE INDEX web_push_deliveries_due
ON web_push_deliveries(status, available_at_ms, subscription_id, event_key);

CREATE TABLE web_push_projection_state (
    state_id INTEGER PRIMARY KEY CHECK (state_id = 1),
    primary_conversation_id TEXT,
    primary_sequence INTEGER NOT NULL DEFAULT 0 CHECK (primary_sequence >= 0),
    updated_at_ms INTEGER NOT NULL
) STRICT;
INSERT INTO web_push_projection_state (state_id, primary_sequence, updated_at_ms)
VALUES (1, 0, CAST(strftime('%s', 'now') AS INTEGER) * 1000);
`
