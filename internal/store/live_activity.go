package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"strings"
	"time"

	"github.com/uptrace/bun"
)

// LiveActivityEvent identifies one ActivityKit remote event.
type LiveActivityEvent string

const (
	LiveActivityStart  LiveActivityEvent = "start"
	LiveActivityUpdate LiveActivityEvent = "update"
	LiveActivityEnd    LiveActivityEvent = "end"
)

// ClientTaskActivity is one client's durable Task activity session.
type ClientTaskActivity struct {
	ClientID, ActivityID, TaskSessionID, Lifecycle string
	UpdateToken                                    []byte `json:"-"`
	Projection                                     map[string]any
	ProjectionSignature, FocusedTaskID             string
	SessionStartedAt                               time.Time
	Suppressed                                     bool
	DismissedAt                                    *time.Time
}

// LiveActivityTarget combines one enabled registration and its Task session.
type LiveActivityTarget struct {
	Registration ClientLiveActivityRegistration
	Activity     *ClientTaskActivity
}

// LiveActivityDelivery is one claimed remote activity event.
type LiveActivityDelivery struct {
	ClientID, DeliveryKey, ActivityID string
	Token                             []byte `json:"-"`
	Environment                       APNSEnvironment
	Event                             LiveActivityEvent
	Payload                           map[string]any
	Urgency                           string
	TTLSeconds, Attempt               int
	CreatedAt                         time.Time
}

// NewLiveActivityDelivery contains one bounded remote event.
type NewLiveActivityDelivery struct {
	ClientID, DeliveryKey, ActivityID string
	Token                             []byte
	Environment                       APNSEnvironment
	Event                             LiveActivityEvent
	Payload                           map[string]any
	Urgency                           string
	TTLSeconds                        int
}

// RegisterClientLiveActivities saves push-to-start material and the client's active snapshot.
func (s *Store) RegisterClientLiveActivities(ctx context.Context, clientID string, token []byte, environment APNSEnvironment, active []string, now time.Time) error {
	if !validAPNSEnvironment(environment) || len(token) < 1 || len(token) > 1024 || len(active) > 64 {
		return errors.New("invalid Live Activity registration")
	}
	seen := make(map[string]struct{}, len(active))
	for _, id := range active {
		if !validActivityID(id) {
			return errors.New("invalid Live Activity identifier")
		}
		if _, exists := seen[id]; exists {
			return errors.New("duplicate Live Activity identifier")
		}
		seen[id] = struct{}{}
	}
	if active == nil {
		active = []string{}
	}
	snapshot, _ := json.Marshal(active)
	if len(snapshot) > 4096 {
		return errors.New("Live Activity snapshot is too large")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	var clientActive bool
	if err := tx.QueryRowContext(ctx, "SELECT EXISTS(SELECT 1 FROM clients WHERE client_id=? AND revoked_at IS NULL)", clientID).Scan(&clientActive); err != nil || !clientActive {
		return errors.Join(err, ErrSessionUnauthorized)
	}
	if err := insertLiveActivityObservation(ctx, tx, clientID, "snapshot", "", snapshot, now); err != nil {
		return err
	}
	rows, err := tx.QueryContext(ctx, `SELECT client_id FROM client_live_activity_registrations
WHERE environment=? AND push_to_start_token=? AND client_id<>?`, environment, token, clientID)
	if err != nil {
		return err
	}
	var transferred []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			_ = rows.Close()
			return err
		}
		transferred = append(transferred, id)
	}
	if err := errors.Join(rows.Err(), rows.Close()); err != nil {
		return err
	}
	for _, id := range transferred {
		if _, err := tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET status='suppressed',last_error_code='token_transferred',updated_at_ms=? WHERE client_id=? AND status='pending'`, millis(now), id); err != nil {
			return err
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM client_task_activities WHERE client_id=?", id); err != nil {
			return err
		}
		if _, err := tx.ExecContext(ctx, "DELETE FROM client_live_activity_registrations WHERE client_id=?", id); err != nil {
			return err
		}
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO client_live_activity_registrations
(client_id,push_to_start_token,environment,enabled,created_at_ms,updated_at_ms)
VALUES(?,?,?,1,?,?) ON CONFLICT(client_id) DO UPDATE SET push_to_start_token=excluded.push_to_start_token,
environment=excluded.environment,enabled=1,updated_at_ms=excluded.updated_at_ms`, clientID, token, environment, millis(now), millis(now)); err != nil {
		return err
	}
	activity, err := clientTaskActivityTx(ctx, tx, clientID)
	if err != nil {
		return err
	}
	replace := activity == nil
	if activity != nil && activity.Lifecycle == "starting" {
		var failed bool
		err = tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM live_activity_deliveries
WHERE client_id=? AND activity_id=? AND event='start' AND status IN ('failed','suppressed'))`, clientID, activity.ActivityID).Scan(&failed)
		replace = failed
	}
	if activity != nil && activity.Lifecycle == "active" {
		_, observed := seen[activity.ActivityID]
		replace = !observed
		if replace {
			_, err = tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET status='suppressed',last_error_code='activity_missing',updated_at_ms=? WHERE client_id=? AND status='pending'`, millis(now), clientID)
		}
	}
	if err != nil {
		return err
	}
	if replace {
		activityID, idErr := newID("live_activity")
		if idErr != nil {
			return idErr
		}
		sessionID, idErr := newID("task_activity")
		if idErr != nil {
			return idErr
		}
		_, err = tx.ExecContext(ctx, `INSERT INTO client_task_activities
(client_id,activity_id,task_session_id,lifecycle,latest_projection_json,latest_projection_signature,suppressed,session_started_at_ms,updated_at_ms)
VALUES(?,?,?,'starting','{}','',0,?,?) ON CONFLICT(client_id) DO UPDATE SET activity_id=excluded.activity_id,
task_session_id=excluded.task_session_id,lifecycle='starting',update_token=NULL,latest_projection_json='{}',
latest_projection_signature='',focused_task_id=NULL,session_started_at_ms=excluded.session_started_at_ms,
suppressed=0,dismissed_at_ms=NULL,updated_at_ms=excluded.updated_at_ms`, clientID, activityID, sessionID, millis(now), millis(now))
	}
	if err != nil {
		return err
	}
	return tx.Commit()
}

// DisableClientLiveActivities retains the disabled preference and clears capability material.
func (s *Store) DisableClientLiveActivities(ctx context.Context, clientID string, now time.Time) error {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `INSERT INTO client_live_activity_registrations
(client_id,push_to_start_token,environment,enabled,created_at_ms,updated_at_ms)
SELECT client_id,NULL,NULL,0,?,? FROM clients WHERE client_id=? AND revoked_at IS NULL
ON CONFLICT(client_id) DO UPDATE SET push_to_start_token=NULL,environment=NULL,enabled=0,updated_at_ms=excluded.updated_at_ms`, millis(now), millis(now), clientID)
	if err != nil {
		return err
	}
	changed, _ := result.RowsAffected()
	if changed != 1 {
		return ErrSessionUnauthorized
	}
	if _, err := tx.ExecContext(ctx, `UPDATE client_task_activities SET lifecycle='dismissed',suppressed=1,
latest_projection_json='{}',latest_projection_signature='',focused_task_id=NULL,dismissed_at_ms=?,updated_at_ms=? WHERE client_id=?`, millis(now), millis(now), clientID); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET status='suppressed',last_error_code='activity_disabled',updated_at_ms=? WHERE client_id=? AND event<>'end' AND status='pending'`, millis(now), clientID); err != nil {
		return err
	}
	return tx.Commit()
}

// RegisterClientLiveActivityUpdate accepts only the current server activity identifier.
func (s *Store) RegisterClientLiveActivityUpdate(ctx context.Context, clientID, activityID string, token []byte, now time.Time) (bool, error) {
	if !validActivityID(activityID) || len(token) < 1 || len(token) > 1024 {
		return false, errors.New("invalid Live Activity update")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return false, err
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `UPDATE client_task_activities SET update_token=?,lifecycle='active',suppressed=0,
dismissed_at_ms=NULL,updated_at_ms=? WHERE client_id=? AND activity_id=? AND lifecycle IN ('starting','active')
AND EXISTS(SELECT 1 FROM client_live_activity_registrations WHERE client_id=? AND enabled=1)`, token, millis(now.UTC()), clientID, activityID, clientID)
	if err != nil {
		return false, err
	}
	changed, _ := result.RowsAffected()
	if changed == 1 {
		if err := insertLiveActivityObservation(ctx, tx, clientID, "update_token", activityID, []byte("[]"), now.UTC()); err != nil {
			return false, err
		}
	}
	return changed == 1, tx.Commit()
}

// DismissClientLiveActivity records one user dismissal and suppresses stale work.
func (s *Store) DismissClientLiveActivity(ctx context.Context, clientID, activityID string) (bool, error) {
	if !validActivityID(activityID) {
		return false, errors.New("invalid Live Activity identifier")
	}
	now := time.Now().UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return false, err
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `UPDATE client_task_activities SET lifecycle='dismissed',suppressed=1,
latest_projection_signature=CASE WHEN latest_projection_signature='' THEN ? ELSE latest_projection_signature END,
dismissed_at_ms=?,updated_at_ms=? WHERE client_id=? AND activity_id=? AND lifecycle<>'dismissed'`, strings.Repeat("0", 64), millis(now), millis(now), clientID, activityID)
	if err != nil {
		return false, err
	}
	changed, _ := result.RowsAffected()
	if changed == 1 {
		if err := insertLiveActivityObservation(ctx, tx, clientID, "dismissed", activityID, []byte("[]"), now); err != nil {
			return false, err
		}
		if _, err := tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET status='suppressed',last_error_code='activity_dismissed',updated_at_ms=? WHERE client_id=? AND activity_id=? AND event<>'end' AND status='pending'`, millis(now), clientID, activityID); err != nil {
			return false, err
		}
	}
	return changed == 1, tx.Commit()
}

// LiveActivityTargets returns enabled, paired client targets in stable order.
func (s *Store) LiveActivityTargets(ctx context.Context) ([]LiveActivityTarget, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT r.client_id,r.push_to_start_token,r.environment,r.enabled,
a.client_id,a.activity_id,a.task_session_id,a.lifecycle,a.update_token,a.latest_projection_json,
a.latest_projection_signature,a.focused_task_id,a.session_started_at_ms,a.suppressed,a.dismissed_at_ms
FROM client_live_activity_registrations r JOIN clients c USING(client_id)
LEFT JOIN client_task_activities a USING(client_id) WHERE c.revoked_at IS NULL AND r.enabled=1 ORDER BY r.client_id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var targets []LiveActivityTarget
	for rows.Next() {
		var target LiveActivityTarget
		var environment sql.NullString
		var activityClient, activityID, lifecycle, projection, signature, focused sql.NullString
		var session sql.NullString
		var update []byte
		var started, dismissed sql.NullInt64
		var suppressed sql.NullBool
		if err := rows.Scan(&target.Registration.ClientID, &target.Registration.PushToStartToken, &environment,
			&target.Registration.Enabled, &activityClient, &activityID, &session, &lifecycle, &update, &projection,
			&signature, &focused, &started, &suppressed, &dismissed); err != nil {
			return nil, err
		}
		if environment.Valid {
			env := APNSEnvironment(environment.String)
			target.Registration.Environment = &env
		}
		if activityClient.Valid {
			value := ClientTaskActivity{ClientID: activityClient.String, ActivityID: activityID.String,
				TaskSessionID: session.String, Lifecycle: lifecycle.String, UpdateToken: update,
				ProjectionSignature: signature.String, FocusedTaskID: focused.String,
				SessionStartedAt: fromMillis(started.Int64), Suppressed: suppressed.Bool,
				DismissedAt: nullTimePointer(dismissed)}
			if err := json.Unmarshal([]byte(projection.String), &value.Projection); err != nil {
				return nil, err
			}
			target.Activity = &value
		}
		targets = append(targets, target)
	}
	return targets, rows.Err()
}

// ClientTaskActivity returns one caller-owned Task activity session.
func (s *Store) ClientTaskActivity(ctx context.Context, clientID string) (*ClientTaskActivity, error) {
	return clientTaskActivityTx(ctx, s.db, clientID)
}

func clientTaskActivityTx(ctx context.Context, query interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}, clientID string) (*ClientTaskActivity, error) {
	var value ClientTaskActivity
	var projection string
	var dismissed sql.NullInt64
	var started int64
	err := query.QueryRowContext(ctx, `SELECT client_id,activity_id,task_session_id,lifecycle,update_token,
latest_projection_json,latest_projection_signature,COALESCE(focused_task_id,''),session_started_at_ms,suppressed,dismissed_at_ms
FROM client_task_activities WHERE client_id=?`, clientID).Scan(&value.ClientID, &value.ActivityID, &value.TaskSessionID,
		&value.Lifecycle, &value.UpdateToken, &projection, &value.ProjectionSignature, &value.FocusedTaskID,
		&started, &value.Suppressed, &dismissed)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if err := json.Unmarshal([]byte(projection), &value.Projection); err != nil {
		return nil, err
	}
	value.SessionStartedAt, value.DismissedAt = fromMillis(started), nullTimePointer(dismissed)
	return &value, nil
}

func insertLiveActivityObservation(ctx context.Context, tx bun.Tx, clientID, event, activityID string, active []byte, now time.Time) error {
	id, err := newID("live_activity_observation")
	if err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM live_activity_observations WHERE created_at_ms<=?", millis(now.Add(-30*24*time.Hour))); err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO live_activity_observations
(observation_id,client_id,event,activity_id,active_activity_ids_json,created_at_ms) VALUES(?,?,?,NULLIF(?,''),?,?)`,
		id, clientID, event, activityID, string(active), millis(now))
	return err
}

// UpdateClientTaskActivityProjection stores the exact last queued display state.
func (s *Store) UpdateClientTaskActivityProjection(ctx context.Context, clientID string, projection map[string]any, signature, focusedTaskID string, now time.Time) (bool, error) {
	encoded, err := json.Marshal(projection)
	if err != nil || len(encoded) > 4096 || signature != "" && (len(signature) != 64 || signature != strings.ToLower(signature)) {
		return false, errors.New("invalid Live Activity projection")
	}
	for _, value := range signature {
		if !strings.ContainsRune("0123456789abcdef", value) {
			return false, errors.New("invalid Live Activity projection")
		}
	}
	result, err := s.db.ExecContext(ctx, `UPDATE client_task_activities SET latest_projection_json=?,
latest_projection_signature=?,focused_task_id=NULLIF(?,''),updated_at_ms=? WHERE client_id=?
AND lifecycle IN ('starting','active') AND suppressed=0`, string(encoded), signature, focusedTaskID, millis(now.UTC()), clientID)
	if err != nil {
		return false, err
	}
	changed, _ := result.RowsAffected()
	return changed == 1, nil
}

// EnsureClientTaskActivitySession starts a new session after a cleared dismissal.
func (s *Store) EnsureClientTaskActivitySession(ctx context.Context, clientID string, now time.Time) (bool, error) {
	activityID, err := newID("live_activity")
	if err != nil {
		return false, err
	}
	sessionID, err := newID("task_activity")
	if err != nil {
		return false, err
	}
	result, err := s.db.ExecContext(ctx, `UPDATE client_task_activities SET activity_id=?,task_session_id=?,
lifecycle='starting',update_token=NULL,latest_projection_json='{}',latest_projection_signature='',
focused_task_id=NULL,session_started_at_ms=?,suppressed=0,dismissed_at_ms=NULL,updated_at_ms=?
WHERE client_id=? AND lifecycle='dismissed' AND latest_projection_signature=''
AND EXISTS(SELECT 1 FROM client_live_activity_registrations WHERE client_id=? AND enabled=1)`,
		activityID, sessionID, millis(now.UTC()), millis(now.UTC()), clientID, clientID)
	if err != nil {
		return false, err
	}
	changed, _ := result.RowsAffected()
	return changed == 1, nil
}

// ClearClientTaskActivityDismissal permits a later, different Task session.
func (s *Store) ClearClientTaskActivityDismissal(ctx context.Context, clientID string, now time.Time) (bool, error) {
	result, err := s.db.ExecContext(ctx, `UPDATE client_task_activities SET latest_projection_json='{}',
latest_projection_signature='',focused_task_id=NULL,updated_at_ms=? WHERE client_id=? AND lifecycle='dismissed'
AND suppressed=1 AND (latest_projection_json<>'{}' OR latest_projection_signature<>'' OR focused_task_id IS NOT NULL)`, millis(now.UTC()), clientID)
	if err != nil {
		return false, err
	}
	changed, _ := result.RowsAffected()
	return changed == 1, nil
}

// MarkClientTaskActivityEnding fences the current session before its end delivery.
func (s *Store) MarkClientTaskActivityEnding(ctx context.Context, clientID, activityID string, now time.Time) (bool, error) {
	result, err := s.db.ExecContext(ctx, `UPDATE client_task_activities SET lifecycle='ending',updated_at_ms=?
WHERE client_id=? AND activity_id=? AND lifecycle IN ('starting','active')`, millis(now.UTC()), clientID, activityID)
	if err != nil {
		return false, err
	}
	changed, _ := result.RowsAffected()
	return changed == 1, nil
}

// QueueLiveActivityDelivery stores one exact payload and token snapshot.
func (s *Store) QueueLiveActivityDelivery(ctx context.Context, value NewLiveActivityDelivery, now time.Time) error {
	encoded, err := json.Marshal(value.Payload)
	if err != nil || len(encoded) > 4096 || !validActivityID(value.ActivityID) || len(value.Token) < 1 || len(value.Token) > 1024 ||
		strings.TrimSpace(value.DeliveryKey) == "" || len(value.DeliveryKey) > 256 || !validAPNSEnvironment(value.Environment) ||
		value.Event != LiveActivityStart && value.Event != LiveActivityUpdate && value.Event != LiveActivityEnd ||
		value.Urgency != "normal" && value.Urgency != "high" || value.TTLSeconds < 0 || value.TTLSeconds > 604800 {
		return errors.New("invalid Live Activity delivery")
	}
	_, err = s.db.ExecContext(ctx, `INSERT INTO live_activity_deliveries
(client_id,delivery_key,activity_id,token,environment,event,payload_json,urgency,ttl_seconds,status,available_at_ms,created_at_ms,updated_at_ms)
VALUES(?,?,?,?,?,?,?,?,?,CASE WHEN ?='end' OR EXISTS(SELECT 1 FROM client_task_activities a
JOIN client_live_activity_registrations r USING(client_id) JOIN clients c USING(client_id)
WHERE c.revoked_at IS NULL AND r.enabled=1 AND r.environment=? AND a.client_id=? AND a.activity_id=?
AND a.suppressed=0 AND ((?='start' AND a.lifecycle='starting' AND r.push_to_start_token=?)
OR (?='update' AND a.lifecycle='active' AND a.update_token=?))) THEN 'pending' ELSE 'suppressed' END,?,?,?)
ON CONFLICT(client_id,delivery_key) DO UPDATE SET
activity_id=CASE WHEN status='pending' THEN excluded.activity_id ELSE activity_id END,
token=CASE WHEN status='pending' THEN excluded.token ELSE token END,
environment=CASE WHEN status='pending' THEN excluded.environment ELSE environment END,
event=CASE WHEN status='pending' THEN excluded.event ELSE event END,
payload_json=CASE WHEN status='pending' THEN excluded.payload_json ELSE payload_json END,
urgency=CASE WHEN status='pending' THEN excluded.urgency ELSE urgency END,
ttl_seconds=CASE WHEN status='pending' THEN excluded.ttl_seconds ELSE ttl_seconds END,updated_at_ms=excluded.updated_at_ms`,
		value.ClientID, value.DeliveryKey, value.ActivityID, value.Token, value.Environment, value.Event,
		string(encoded), value.Urgency, value.TTLSeconds, value.Event, value.Environment, value.ClientID,
		value.ActivityID, value.Event, value.Token, value.Event, value.Token,
		millis(now.UTC()), millis(now.UTC()), millis(now.UTC()))
	return err
}

// ClaimDueLiveActivityDelivery claims one current due delivery.
func (s *Store) ClaimDueLiveActivityDelivery(ctx context.Context, now time.Time) (*LiveActivityDelivery, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, err
	}
	defer func() { _ = tx.Rollback() }()
	if _, err := tx.ExecContext(ctx, "DELETE FROM live_activity_deliveries WHERE status<>'pending' AND created_at_ms<=?", millis(now.Add(-30*24*time.Hour))); err != nil {
		return nil, err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET status='suppressed',last_error_code='stale_delivery',updated_at_ms=?
WHERE status='pending' AND event<>'end' AND NOT EXISTS(SELECT 1 FROM client_task_activities a
JOIN client_live_activity_registrations r USING(client_id) JOIN clients c USING(client_id)
WHERE c.revoked_at IS NULL AND r.enabled=1 AND r.environment=live_activity_deliveries.environment
AND a.client_id=live_activity_deliveries.client_id AND a.activity_id=live_activity_deliveries.activity_id
AND a.suppressed=0 AND ((live_activity_deliveries.event='start' AND a.lifecycle='starting' AND r.push_to_start_token=live_activity_deliveries.token)
OR (live_activity_deliveries.event='update' AND a.lifecycle='active' AND a.update_token=live_activity_deliveries.token)))`, millis(now)); err != nil {
		return nil, err
	}
	var clientID, key string
	err = tx.QueryRowContext(ctx, `SELECT client_id,delivery_key FROM live_activity_deliveries
WHERE status='pending' AND available_at_ms<=? ORDER BY available_at_ms,client_id,delivery_key LIMIT 1`, millis(now)).Scan(&clientID, &key)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, tx.Commit()
	}
	if err != nil {
		return nil, err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET attempt_count=attempt_count+1,updated_at_ms=?
WHERE client_id=? AND delivery_key=? AND status='pending'`, millis(now), clientID, key); err != nil {
		return nil, err
	}
	value, err := scanLiveActivityDelivery(tx.QueryRowContext(ctx, `SELECT client_id,delivery_key,activity_id,token,
environment,event,payload_json,urgency,ttl_seconds,attempt_count,created_at_ms FROM live_activity_deliveries
WHERE client_id=? AND delivery_key=? AND status='pending'`, clientID, key))
	if err != nil {
		return nil, err
	}
	return &value, tx.Commit()
}

// FinishLiveActivityDelivery saves one result against the exact claimed token.
func (s *Store) FinishLiveActivityDelivery(ctx context.Context, value LiveActivityDelivery, outcome APNSDeliveryOutcome, code, apnsID string, now time.Time) error {
	if len(code) > 128 || strings.TrimSpace(code) != code || len(apnsID) > 128 || strings.TrimSpace(apnsID) != apnsID || !isASCIIString(apnsID) {
		return errors.New("invalid Live Activity delivery result")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	if outcome == APNSInvalid {
		if value.Event == LiveActivityStart {
			_, err = tx.ExecContext(ctx, `UPDATE client_live_activity_registrations SET push_to_start_token=NULL,environment=NULL,updated_at_ms=? WHERE client_id=? AND push_to_start_token=?`, millis(now), value.ClientID, value.Token)
		} else {
			_, err = tx.ExecContext(ctx, `UPDATE client_task_activities SET update_token=NULL,lifecycle='ending',updated_at_ms=? WHERE client_id=? AND update_token=?`, millis(now), value.ClientID, value.Token)
		}
		if err == nil {
			err = finishLiveActivityTx(ctx, tx, value, "failed", "invalid_device_token", apnsID, now)
		}
	} else if outcome == APNSRetry {
		status, delay := "pending", time.Minute
		if value.Attempt == 2 {
			delay = 5 * time.Minute
		} else if value.Attempt == 3 {
			delay = 30 * time.Minute
		} else if value.Attempt >= 4 {
			status = "failed"
		}
		_, err = tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET status=?,available_at_ms=?,last_error_code=NULLIF(?,''),
apns_id=COALESCE(NULLIF(?,''),apns_id),updated_at_ms=? WHERE client_id=? AND delivery_key=? AND status='pending' AND token=?`,
			status, millis(now.Add(delay)), code, apnsID, millis(now), value.ClientID, value.DeliveryKey, value.Token)
	} else {
		status := string(outcome)
		if outcome != APNSDelivered && outcome != APNSSuppressed && outcome != APNSFailed {
			return errors.New("invalid Live Activity delivery outcome")
		}
		err = finishLiveActivityTx(ctx, tx, value, status, code, apnsID, now)
	}
	if err != nil {
		return err
	}
	if outcome == APNSDelivered && value.Event == LiveActivityEnd {
		_, err = tx.ExecContext(ctx, `UPDATE client_task_activities SET lifecycle='dismissed',suppressed=1,
latest_projection_json=CASE WHEN lifecycle='ending' THEN '{}' ELSE latest_projection_json END,
latest_projection_signature=CASE WHEN lifecycle='ending' THEN '' ELSE latest_projection_signature END,
focused_task_id=CASE WHEN lifecycle='ending' THEN NULL ELSE focused_task_id END,dismissed_at_ms=?,updated_at_ms=?
WHERE client_id=? AND activity_id=?`, millis(now), millis(now), value.ClientID, value.ActivityID)
	}
	if err != nil {
		return err
	}
	return tx.Commit()
}

func finishLiveActivityTx(ctx context.Context, tx bun.Tx, value LiveActivityDelivery, status, code, apnsID string, now time.Time) error {
	_, err := tx.ExecContext(ctx, `UPDATE live_activity_deliveries SET status=?,last_error_code=NULLIF(?,''),
apns_id=COALESCE(NULLIF(?,''),apns_id),updated_at_ms=? WHERE client_id=? AND delivery_key=? AND status='pending' AND token=?`,
		status, code, apnsID, millis(now), value.ClientID, value.DeliveryKey, value.Token)
	return err
}

// NextLiveActivityDelivery returns the next retry or send time.
func (s *Store) NextLiveActivityDelivery(ctx context.Context) (*time.Time, error) {
	var value sql.NullInt64
	if err := s.db.QueryRowContext(ctx, "SELECT MIN(available_at_ms) FROM live_activity_deliveries WHERE status='pending'").Scan(&value); err != nil {
		return nil, err
	}
	return nullTimePointer(value), nil
}

// FailPendingLiveActivityDeliveries makes pending activity events terminal after provider removal.
func (s *Store) FailPendingLiveActivityDeliveries(ctx context.Context, code string, now time.Time) error {
	if code == "" || len(code) > 128 || strings.TrimSpace(code) != code {
		return errors.New("invalid Live Activity delivery diagnostic")
	}
	_, err := s.db.ExecContext(ctx, `UPDATE live_activity_deliveries SET status='failed',
last_error_code=?,updated_at_ms=? WHERE status='pending'`, code, millis(now.UTC()))
	return err
}

func scanLiveActivityDelivery(row rowScanner) (LiveActivityDelivery, error) {
	var value LiveActivityDelivery
	var payload string
	var created int64
	err := row.Scan(&value.ClientID, &value.DeliveryKey, &value.ActivityID, &value.Token, &value.Environment,
		&value.Event, &payload, &value.Urgency, &value.TTLSeconds, &value.Attempt, &created)
	if err != nil {
		return value, err
	}
	err = json.Unmarshal([]byte(payload), &value.Payload)
	value.CreatedAt = fromMillis(created)
	return value, err
}

func isASCIIString(value string) bool {
	for _, b := range []byte(value) {
		if b > 127 {
			return false
		}
	}
	return true
}

// CurrentTaskRun returns the exact run owned by the current Task generation.
func (s *Store) CurrentTaskRun(ctx context.Context, task Task) (*TaskRun, error) {
	if task.CurrentRunID == "" {
		return nil, nil
	}
	run, err := taskRunTx(ctx, s.db, task.CurrentRunID)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if run.TaskID != task.ID || run.Generation != task.Generation {
		return nil, nil
	}
	return &run, nil
}

// TaskArtifactCount returns the exact current output-family count.
func (s *Store) TaskArtifactCount(ctx context.Context, taskID string) (int, error) {
	var count int
	err := s.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM artifacts WHERE owner_object_type='task'
AND owner_object_id=? AND deleted_at_ms IS NULL`, taskID).Scan(&count)
	return count, err
}
