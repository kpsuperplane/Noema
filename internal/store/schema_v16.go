package store

// schemaV16SQL adds native Apple notification registrations and durable alerts.
const schemaV16SQL = `
CREATE TABLE client_notification_registrations (
    client_id TEXT PRIMARY KEY REFERENCES clients(client_id) ON DELETE CASCADE,
    device_token BLOB NOT NULL CHECK (length(device_token) BETWEEN 1 AND 1024),
    environment TEXT NOT NULL CHECK (environment IN ('development','production')),
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (environment, device_token)
) STRICT;

CREATE TABLE apns_deliveries (
    client_id TEXT NOT NULL REFERENCES client_notification_registrations(client_id) ON DELETE CASCADE,
    event_key TEXT NOT NULL CHECK (length(event_key) BETWEEN 1 AND 256),
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 600),
    body TEXT NOT NULL CHECK (length(body) <= 2048),
    route TEXT NOT NULL CHECK (route IN ('chat','task')),
    task_id TEXT CHECK (task_id IS NULL OR length(task_id) BETWEEN 1 AND 128),
    urgency TEXT NOT NULL CHECK (urgency IN ('normal','high')),
    ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds BETWEEN 0 AND 604800),
    status TEXT NOT NULL CHECK (status IN ('pending','delivered','suppressed','failed')),
    available_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    last_error_code TEXT CHECK (last_error_code IS NULL OR length(last_error_code) BETWEEN 1 AND 128),
    apns_id TEXT CHECK (apns_id IS NULL OR length(apns_id) BETWEEN 1 AND 128),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (client_id, event_key)
) STRICT;
CREATE INDEX apns_deliveries_due
ON apns_deliveries(status, available_at_ms, client_id, event_key);

CREATE TABLE client_live_activity_registrations (
    client_id TEXT PRIMARY KEY REFERENCES clients(client_id) ON DELETE CASCADE,
    push_to_start_token BLOB CHECK (push_to_start_token IS NULL OR length(push_to_start_token) BETWEEN 1 AND 1024),
    environment TEXT CHECK (environment IS NULL OR environment IN ('development','production')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    CHECK ((push_to_start_token IS NULL) = (environment IS NULL)),
    CHECK (enabled = 1 OR push_to_start_token IS NULL),
    UNIQUE (environment, push_to_start_token)
) STRICT;

CREATE TABLE client_live_activities (
    client_id TEXT NOT NULL REFERENCES client_live_activity_registrations(client_id) ON DELETE CASCADE,
    activity_id TEXT NOT NULL CHECK (activity_id GLOB 'live_activity:*' AND length(activity_id) <= 256),
    update_token BLOB CHECK (update_token IS NULL OR length(update_token) BETWEEN 1 AND 1024),
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (client_id, activity_id)
) STRICT;
`
