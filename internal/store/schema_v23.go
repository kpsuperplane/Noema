package store

// schemaV23SQL replaces the v16 activity snapshot with durable Task activity sessions and delivery attempts.
const schemaV23SQL = `
CREATE TABLE client_task_activities (
    client_id TEXT PRIMARY KEY REFERENCES client_live_activity_registrations(client_id) ON DELETE CASCADE,
    activity_id TEXT NOT NULL UNIQUE CHECK (activity_id GLOB 'live_activity:*' AND length(activity_id) <= 256),
    task_session_id TEXT NOT NULL CHECK (task_session_id GLOB 'task_activity:*' AND length(task_session_id) <= 256),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('starting','active','ending','dismissed')),
    update_token BLOB UNIQUE CHECK (update_token IS NULL OR length(update_token) BETWEEN 1 AND 1024),
    latest_projection_json TEXT NOT NULL DEFAULT '{}' CHECK (
        json_valid(latest_projection_json) AND json_type(latest_projection_json) = 'object'
        AND length(CAST(latest_projection_json AS BLOB)) <= 4096
    ),
    latest_projection_signature TEXT NOT NULL DEFAULT '' CHECK (
        latest_projection_signature = '' OR (
            length(latest_projection_signature) = 64
            AND latest_projection_signature = lower(latest_projection_signature)
            AND latest_projection_signature NOT GLOB '*[^0-9a-f]*'
        )
    ),
    focused_task_id TEXT CHECK (focused_task_id IS NULL OR (
        focused_task_id GLOB 'task:*' AND length(focused_task_id) <= 256
    )),
    session_started_at_ms INTEGER NOT NULL,
    suppressed INTEGER NOT NULL DEFAULT 0 CHECK (suppressed IN (0,1)),
    dismissed_at_ms INTEGER,
    updated_at_ms INTEGER NOT NULL,
    CHECK (update_token IS NULL OR activity_id IS NOT NULL)
) STRICT;

INSERT INTO client_task_activities (
    client_id, activity_id, task_session_id, lifecycle, update_token,
    session_started_at_ms, suppressed, dismissed_at_ms, updated_at_ms
)
SELECT r.client_id,
       COALESCE((SELECT a.activity_id FROM client_live_activities a WHERE a.client_id=r.client_id
                 AND (SELECT COUNT(*) FROM client_live_activities duplicate
                      WHERE duplicate.activity_id=a.activity_id)=1
                 AND (a.update_token IS NULL OR (SELECT COUNT(*) FROM client_live_activities duplicate
                      WHERE duplicate.update_token=a.update_token)=1)
                 ORDER BY a.updated_at_ms DESC,a.activity_id LIMIT 1),
                'live_activity:' || lower(hex(randomblob(16)))),
       'task_activity:' || lower(hex(randomblob(16))),
       CASE WHEN r.enabled=0 THEN 'dismissed'
            WHEN EXISTS(SELECT 1 FROM client_live_activities a WHERE a.client_id=r.client_id) THEN 'active'
            ELSE 'starting' END,
       (SELECT a.update_token FROM client_live_activities a WHERE a.client_id=r.client_id
        AND (SELECT COUNT(*) FROM client_live_activities duplicate
             WHERE duplicate.activity_id=a.activity_id)=1
        AND (a.update_token IS NULL OR (SELECT COUNT(*) FROM client_live_activities duplicate
             WHERE duplicate.update_token=a.update_token)=1)
        ORDER BY a.updated_at_ms DESC,a.activity_id LIMIT 1),
       r.updated_at_ms,
       CASE WHEN r.enabled=0 THEN 1 ELSE 0 END,
       CASE WHEN r.enabled=0 THEN r.updated_at_ms ELSE NULL END,
       r.updated_at_ms
FROM client_live_activity_registrations r;

DROP TABLE client_live_activities;

CREATE TABLE live_activity_deliveries (
    client_id TEXT NOT NULL,
    delivery_key TEXT NOT NULL CHECK (length(trim(delivery_key)) BETWEEN 1 AND 256),
    activity_id TEXT NOT NULL CHECK (activity_id GLOB 'live_activity:*' AND length(activity_id) <= 256),
    token BLOB NOT NULL CHECK (length(token) BETWEEN 1 AND 1024),
    environment TEXT NOT NULL CHECK (environment IN ('development','production')),
    event TEXT NOT NULL CHECK (event IN ('start','update','end')),
    payload_json TEXT NOT NULL CHECK (
        json_valid(payload_json) AND json_type(payload_json) = 'object'
        AND length(CAST(payload_json AS BLOB)) <= 4096
    ),
    urgency TEXT NOT NULL CHECK (urgency IN ('normal','high')),
    ttl_seconds INTEGER NOT NULL CHECK (ttl_seconds BETWEEN 0 AND 604800),
    status TEXT NOT NULL CHECK (status IN ('pending','delivered','suppressed','failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 4),
    available_at_ms INTEGER NOT NULL,
    last_error_code TEXT CHECK (last_error_code IS NULL OR length(last_error_code) BETWEEN 1 AND 128),
    apns_id TEXT CHECK (apns_id IS NULL OR (length(apns_id) BETWEEN 1 AND 128)),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (client_id, delivery_key)
) STRICT;
CREATE INDEX live_activity_deliveries_due
ON live_activity_deliveries(status, available_at_ms, client_id, delivery_key)
WHERE status = 'pending';

CREATE TABLE live_activity_observations (
    observation_id TEXT PRIMARY KEY CHECK (
        observation_id GLOB 'live_activity_observation:*' AND length(observation_id) <= 256
    ),
    client_id TEXT NOT NULL REFERENCES clients(client_id) ON DELETE CASCADE,
    event TEXT NOT NULL CHECK (event IN ('snapshot','update_token','dismissed')),
    activity_id TEXT CHECK (activity_id IS NULL OR (
        activity_id GLOB 'live_activity:*' AND length(activity_id) <= 256
    )),
    active_activity_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (
        json_valid(active_activity_ids_json) AND json_type(active_activity_ids_json) = 'array'
        AND length(CAST(active_activity_ids_json AS BLOB)) <= 4096
    ),
    created_at_ms INTEGER NOT NULL,
    CHECK ((event='snapshot' AND activity_id IS NULL)
        OR (event<>'snapshot' AND activity_id IS NOT NULL AND active_activity_ids_json='[]'))
) STRICT;
CREATE INDEX live_activity_observations_timeline
ON live_activity_observations(client_id, created_at_ms, observation_id);
`
