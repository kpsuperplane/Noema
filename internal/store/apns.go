package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"
)

// APNSEnvironment selects one Apple Push service.
type APNSEnvironment string

const (
	APNSDevelopment APNSEnvironment = "development"
	APNSProduction  APNSEnvironment = "production"
)

// ClientNotificationRegistration is one native client Push capability.
type ClientNotificationRegistration struct {
	ClientID    string
	DeviceToken []byte `json:"-"`
	Environment APNSEnvironment
	Revision    int64
}

// ClientLiveActivityRegistration holds one client's push-to-start capability.
type ClientLiveActivityRegistration struct {
	ClientID         string
	PushToStartToken []byte `json:"-"`
	Environment      *APNSEnvironment
	Enabled          bool
}

// APNSNotification is one bounded native alert.
type APNSNotification struct {
	EventKey, Title, Body, Route, Urgency string
	TaskID                                *string
	TTLSeconds                            int
}

// APNSDelivery is one claimed native alert.
type APNSDelivery struct {
	Registration ClientNotificationRegistration
	Notification APNSNotification
	Attempt      int
	CreatedAt    time.Time
}

// APNSDeliveryOutcome identifies one transport result.
type APNSDeliveryOutcome string

const (
	APNSDelivered  APNSDeliveryOutcome = "delivered"
	APNSSuppressed APNSDeliveryOutcome = "suppressed"
	APNSFailed     APNSDeliveryOutcome = "failed"
	APNSRetry      APNSDeliveryOutcome = "retry"
	APNSInvalid    APNSDeliveryOutcome = "invalid_token"
)

// ClientNotificationRegistration returns one active client registration.
func (s *Store) ClientNotificationRegistration(ctx context.Context, clientID string) (*ClientNotificationRegistration, error) {
	value, err := scanClientNotification(s.db.QueryRowContext(ctx, `SELECT r.client_id,
r.device_token, r.environment, r.revision FROM client_notification_registrations r
JOIN clients c USING (client_id) WHERE r.client_id = ? AND c.revoked_at IS NULL`, clientID))
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	return &value, err
}

// RegisterClientNotifications creates, refreshes, or transfers one device token.
func (s *Store) RegisterClientNotifications(ctx context.Context, clientID string, token []byte, environment APNSEnvironment, now time.Time) error {
	if !validAPNSEnvironment(environment) || len(token) < 1 || len(token) > 1024 {
		return errors.New("invalid client notification registration")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	var active bool
	if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM clients
WHERE client_id = ? AND revoked_at IS NULL)`, clientID).Scan(&active); err != nil || !active {
		return errors.Join(err, ErrSessionUnauthorized)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM client_notification_registrations
WHERE environment = ? AND device_token = ? AND client_id <> ?`, environment, token, clientID); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM apns_deliveries WHERE client_id = ? AND EXISTS (
SELECT 1 FROM client_notification_registrations WHERE client_id = ?
AND (environment <> ? OR device_token <> ?))`, clientID, clientID, environment, token); err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO client_notification_registrations
(client_id, device_token, environment, revision, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, 1, ?, ?) ON CONFLICT(client_id) DO UPDATE SET
device_token = excluded.device_token, environment = excluded.environment,
revision = revision + CASE WHEN device_token <> excluded.device_token OR environment <> excluded.environment THEN 1 ELSE 0 END,
updated_at_ms = excluded.updated_at_ms`, clientID, token, environment, millis(now.UTC()), millis(now.UTC()))
	if err != nil {
		return err
	}
	return tx.Commit()
}

// DisableClientNotifications removes one client registration and its deliveries.
func (s *Store) DisableClientNotifications(ctx context.Context, clientID string) error {
	_, err := s.db.ExecContext(ctx, "DELETE FROM client_notification_registrations WHERE client_id = ?", clientID)
	return err
}

// QueueAPNSNotification fans one event to active clients once.
func (s *Store) QueueAPNSNotification(ctx context.Context, value APNSNotification, visible map[string]struct{}, now time.Time) error {
	if err := validateAPNSNotification(value); err != nil {
		return err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	rows, err := tx.QueryContext(ctx, `SELECT r.client_id FROM client_notification_registrations r
JOIN clients c USING (client_id) WHERE c.revoked_at IS NULL ORDER BY r.client_id`)
	if err != nil {
		return err
	}
	var ids []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			_ = rows.Close()
			return err
		}
		ids = append(ids, id)
	}
	if err := errors.Join(rows.Err(), rows.Close()); err != nil {
		return err
	}
	for _, id := range ids {
		status := "pending"
		if value.Route == "chat" {
			if _, ok := visible[id]; ok {
				status = "suppressed"
			}
		}
		if _, err := tx.ExecContext(ctx, `INSERT INTO apns_deliveries
(client_id, event_key, title, body, route, task_id, urgency, ttl_seconds,
status, available_at_ms, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(client_id, event_key) DO UPDATE SET
title = CASE WHEN status = 'pending' THEN excluded.title ELSE title END,
body = CASE WHEN status = 'pending' THEN excluded.body ELSE body END,
updated_at_ms = excluded.updated_at_ms`, id, value.EventKey, value.Title, value.Body,
			value.Route, value.TaskID, value.Urgency, value.TTLSeconds, status,
			millis(now.Add(time.Second)), millis(now), millis(now)); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// ClaimDueAPNSDelivery claims one due alert and increments its attempt.
func (s *Store) ClaimDueAPNSDelivery(ctx context.Context, now time.Time) (*APNSDelivery, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, err
	}
	defer func() { _ = tx.Rollback() }()
	_, err = tx.ExecContext(ctx, `UPDATE apns_deliveries SET status = 'failed', last_error_code = 'registration_unavailable', updated_at_ms = ?
WHERE status = 'pending' AND NOT EXISTS (SELECT 1 FROM client_notification_registrations r
JOIN clients c USING (client_id) WHERE r.client_id = apns_deliveries.client_id AND c.revoked_at IS NULL)`, millis(now))
	if err != nil {
		return nil, err
	}
	var clientID, eventKey string
	err = tx.QueryRowContext(ctx, `SELECT client_id, event_key FROM apns_deliveries
WHERE status = 'pending' AND available_at_ms <= ? ORDER BY available_at_ms, client_id, event_key LIMIT 1`, millis(now)).Scan(&clientID, &eventKey)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, tx.Commit()
	}
	if err != nil {
		return nil, err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE apns_deliveries SET attempt_count = attempt_count + 1,
updated_at_ms = ? WHERE client_id = ? AND event_key = ?`, millis(now), clientID, eventKey); err != nil {
		return nil, err
	}
	value, err := scanAPNSDelivery(tx.QueryRowContext(ctx, `SELECT r.client_id, r.device_token,
r.environment, r.revision, d.event_key, d.title, d.body, d.route, d.task_id, d.urgency,
d.ttl_seconds, d.attempt_count, d.created_at_ms FROM apns_deliveries d
JOIN client_notification_registrations r USING (client_id)
WHERE d.client_id = ? AND d.event_key = ?`, clientID, eventKey))
	if err != nil {
		return nil, err
	}
	return &value, tx.Commit()
}

// FinishAPNSDelivery saves one result against the exact registration revision.
func (s *Store) FinishAPNSDelivery(ctx context.Context, value APNSDelivery, outcome APNSDeliveryOutcome, code, apnsID string, now time.Time) error {
	if len(code) > 128 || strings.TrimSpace(code) != code || len(apnsID) > 128 || strings.TrimSpace(apnsID) != apnsID {
		return errors.New("invalid APNs error code")
	}
	if outcome == APNSInvalid {
		_, err := s.db.ExecContext(ctx, `DELETE FROM client_notification_registrations
WHERE client_id = ? AND revision = ? AND device_token = ? AND EXISTS (
SELECT 1 FROM apns_deliveries WHERE client_id = ? AND event_key = ? AND status = 'pending')`,
			value.Registration.ClientID, value.Registration.Revision, value.Registration.DeviceToken,
			value.Registration.ClientID, value.Notification.EventKey)
		return err
	}
	status, available := string(outcome), now
	if outcome == APNSRetry {
		status = "pending"
		delay := time.Minute
		if value.Attempt == 2 {
			delay = 5 * time.Minute
		}
		if value.Attempt == 3 {
			delay = 30 * time.Minute
		}
		if value.Attempt >= 4 {
			status = "failed"
		}
		available = now.Add(delay)
	} else if outcome != APNSDelivered && outcome != APNSSuppressed && outcome != APNSFailed {
		return errors.New("invalid APNs delivery outcome")
	}
	_, err := s.db.ExecContext(ctx, `UPDATE apns_deliveries SET status = ?, available_at_ms = ?,
last_error_code = NULLIF(?, ''), apns_id = COALESCE(NULLIF(?, ''), apns_id), updated_at_ms = ? WHERE client_id = ? AND event_key = ?
AND status = 'pending'
AND EXISTS (SELECT 1 FROM client_notification_registrations WHERE client_id = ? AND revision = ? AND device_token = ?)`,
		status, millis(available), code, apnsID, millis(now), value.Registration.ClientID,
		value.Notification.EventKey, value.Registration.ClientID, value.Registration.Revision, value.Registration.DeviceToken)
	return err
}

// NextAPNSDelivery returns the next retry or send time.
func (s *Store) NextAPNSDelivery(ctx context.Context) (*time.Time, error) {
	var value sql.NullInt64
	if err := s.db.QueryRowContext(ctx, "SELECT MIN(available_at_ms) FROM apns_deliveries WHERE status = 'pending'").Scan(&value); err != nil {
		return nil, err
	}
	return nullTimePointer(value), nil
}

// FailPendingAPNS makes pending alerts terminal after provider removal.
func (s *Store) FailPendingAPNS(ctx context.Context, code string, now time.Time) error {
	if code == "" || len(code) > 128 || strings.TrimSpace(code) != code {
		return errors.New("invalid APNs error code")
	}
	_, err := s.db.ExecContext(ctx, `UPDATE apns_deliveries SET status = 'failed',
last_error_code = ?, updated_at_ms = ? WHERE status = 'pending'`, code, millis(now))
	return err
}

// ClientLiveActivityRegistration returns one client's current capability.
func (s *Store) ClientLiveActivityRegistration(ctx context.Context, clientID string) (*ClientLiveActivityRegistration, error) {
	var value ClientLiveActivityRegistration
	var token []byte
	var environment sql.NullString
	var enabled bool
	err := s.db.QueryRowContext(ctx, `SELECT r.client_id, r.push_to_start_token, r.environment, r.enabled
FROM client_live_activity_registrations r JOIN clients c USING (client_id)
WHERE r.client_id = ? AND c.revoked_at IS NULL`, clientID).Scan(&value.ClientID, &token, &environment, &enabled)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	value.PushToStartToken, value.Enabled = token, enabled
	if environment.Valid {
		env := APNSEnvironment(environment.String)
		value.Environment = &env
	}
	return &value, nil
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
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	var clientActive bool
	if err := tx.QueryRowContext(ctx, "SELECT EXISTS(SELECT 1 FROM clients WHERE client_id = ? AND revoked_at IS NULL)", clientID).Scan(&clientActive); err != nil || !clientActive {
		return errors.Join(err, ErrSessionUnauthorized)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM client_live_activity_registrations
WHERE environment = ? AND push_to_start_token = ? AND client_id <> ?`, environment, token, clientID); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO client_live_activity_registrations
(client_id, push_to_start_token, environment, enabled, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, 1, ?, ?) ON CONFLICT(client_id) DO UPDATE SET
push_to_start_token = excluded.push_to_start_token, environment = excluded.environment,
enabled = 1, updated_at_ms = excluded.updated_at_ms`, clientID, token, environment, millis(now), millis(now)); err != nil {
		return err
	}
	existing := make(map[string][]byte)
	rows, err := tx.QueryContext(ctx, "SELECT activity_id, update_token FROM client_live_activities WHERE client_id = ?", clientID)
	if err != nil {
		return err
	}
	for rows.Next() {
		var id string
		var update []byte
		if err := rows.Scan(&id, &update); err != nil {
			_ = rows.Close()
			return err
		}
		existing[id] = update
	}
	if err := errors.Join(rows.Err(), rows.Close()); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM client_live_activities WHERE client_id = ?", clientID); err != nil {
		return err
	}
	for _, id := range active {
		var update any
		if token, ok := existing[id]; ok && token != nil {
			update = token
		}
		if _, err := tx.ExecContext(ctx, `INSERT INTO client_live_activities
(client_id, activity_id, update_token, updated_at_ms) VALUES (?, ?, ?, ?)`, clientID, id, update, millis(now)); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// RegisterClientLiveActivityUpdate saves a token only for an observed active identifier.
func (s *Store) RegisterClientLiveActivityUpdate(ctx context.Context, clientID, activityID string, token []byte, now time.Time) (bool, error) {
	if !validActivityID(activityID) || len(token) < 1 || len(token) > 1024 {
		return false, errors.New("invalid Live Activity update")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE client_live_activities SET update_token = ?, updated_at_ms = ?
WHERE client_id = ? AND activity_id = ? AND EXISTS (SELECT 1 FROM client_live_activity_registrations
WHERE client_id = ? AND enabled = 1)`, token, millis(now), clientID, activityID, clientID)
	if err != nil {
		return false, err
	}
	changed, err := result.RowsAffected()
	return changed == 1, err
}

// DismissClientLiveActivity removes one caller-owned active identifier.
func (s *Store) DismissClientLiveActivity(ctx context.Context, clientID, activityID string) (bool, error) {
	if !validActivityID(activityID) {
		return false, errors.New("invalid Live Activity identifier")
	}
	result, err := s.db.ExecContext(ctx, "DELETE FROM client_live_activities WHERE client_id = ? AND activity_id = ?", clientID, activityID)
	if err != nil {
		return false, err
	}
	changed, err := result.RowsAffected()
	return changed == 1, err
}

// DisableClientLiveActivities keeps the preference and removes all token material.
func (s *Store) DisableClientLiveActivities(ctx context.Context, clientID string, now time.Time) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	if _, err := tx.ExecContext(ctx, "DELETE FROM client_live_activities WHERE client_id = ?", clientID); err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO client_live_activity_registrations
(client_id, push_to_start_token, environment, enabled, created_at_ms, updated_at_ms)
SELECT client_id, NULL, NULL, 0, ?, ? FROM clients WHERE client_id = ? AND revoked_at IS NULL
ON CONFLICT(client_id) DO UPDATE SET push_to_start_token = NULL, environment = NULL,
enabled = 0, updated_at_ms = excluded.updated_at_ms`, millis(now), millis(now), clientID)
	if err != nil {
		return err
	}
	return tx.Commit()
}

func scanClientNotification(row rowScanner) (ClientNotificationRegistration, error) {
	var value ClientNotificationRegistration
	err := row.Scan(&value.ClientID, &value.DeviceToken, &value.Environment, &value.Revision)
	return value, err
}

func scanAPNSDelivery(row rowScanner) (APNSDelivery, error) {
	var value APNSDelivery
	var taskID sql.NullString
	var created int64
	err := row.Scan(&value.Registration.ClientID, &value.Registration.DeviceToken,
		&value.Registration.Environment, &value.Registration.Revision, &value.Notification.EventKey,
		&value.Notification.Title, &value.Notification.Body, &value.Notification.Route, &taskID,
		&value.Notification.Urgency, &value.Notification.TTLSeconds, &value.Attempt, &created)
	if taskID.Valid {
		value.Notification.TaskID = &taskID.String
	}
	value.CreatedAt = time.UnixMilli(created).UTC()
	return value, err
}

func validateAPNSNotification(value APNSNotification) error {
	if strings.TrimSpace(value.EventKey) == "" || len(value.EventKey) > 256 ||
		strings.TrimSpace(value.Title) == "" || len(value.Title) > 600 || len(value.Body) > 2048 ||
		(value.Route != "chat" && value.Route != "task") ||
		(value.Route == "chat" && value.TaskID != nil) ||
		(value.Route == "task" && (value.TaskID == nil || !validTaskID(*value.TaskID))) ||
		(value.Urgency != "normal" && value.Urgency != "high") || value.TTLSeconds < 0 || value.TTLSeconds > 604800 {
		return errors.New("invalid APNs notification")
	}
	return nil
}

func validAPNSEnvironment(value APNSEnvironment) bool {
	return value == APNSDevelopment || value == APNSProduction
}
func validActivityID(value string) bool {
	return strings.HasPrefix(value, "live_activity:") && len(value) <= 256 && strings.TrimSpace(value) == value
}

func (value ClientNotificationRegistration) String() string {
	return fmt.Sprintf("ClientNotificationRegistration{ClientID:%q, Environment:%q, Revision:%d}", value.ClientID, value.Environment, value.Revision)
}
func (value ClientNotificationRegistration) GoString() string { return value.String() }
func (value ClientLiveActivityRegistration) String() string {
	return fmt.Sprintf("ClientLiveActivityRegistration{ClientID:%q, Enabled:%t}", value.ClientID, value.Enabled)
}
func (value ClientLiveActivityRegistration) GoString() string { return value.String() }

func (value APNSDelivery) String() string {
	return fmt.Sprintf("APNSDelivery{ClientID:%q, EventKey:%q, Attempt:%d}",
		value.Registration.ClientID, value.Notification.EventKey, value.Attempt)
}

func (value APNSDelivery) GoString() string { return value.String() }
