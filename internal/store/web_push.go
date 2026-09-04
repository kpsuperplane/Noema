package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"net/url"
	"strings"
	"time"
)

// WebPushSubscription is one browser-bound Push registration.
type WebPushSubscription struct {
	ID, OwnerHumanID string
	Endpoint         string   `json:"-"`
	P256DH           string   `json:"-"`
	AuthSecret       string   `json:"-"`
	SessionHash      [32]byte `json:"-"`
	Revision         int64
}

// NewWebPushSubscription contains one validated browser registration.
type NewWebPushSubscription struct {
	OwnerHumanID string
	Endpoint     string   `json:"-"`
	P256DH       string   `json:"-"`
	AuthSecret   string   `json:"-"`
	SessionHash  [32]byte `json:"-"`
}

// WebPushNotification is one bounded notification for the local human.
type WebPushNotification struct {
	EventKey, Title, Body, NavigatePath, Urgency string
	TTLSeconds                                   int
}

// WebPushDelivery is one claimed outbound browser notification.
type WebPushDelivery struct {
	Subscription WebPushSubscription
	Notification WebPushNotification
	Attempt      int
}

// WebPushDeliveryOutcome identifies one transport result.
type WebPushDeliveryOutcome string

const (
	WebPushDelivered  WebPushDeliveryOutcome = "delivered"
	WebPushSuppressed WebPushDeliveryOutcome = "suppressed"
	WebPushFailed     WebPushDeliveryOutcome = "failed"
	WebPushRetry      WebPushDeliveryOutcome = "retry"
	WebPushExpired    WebPushDeliveryOutcome = "expired"
)

// WebPushPrimaryCheckpoint is the durable primary Chat scan position.
type WebPushPrimaryCheckpoint struct {
	ConversationID string
	Sequence       int64
}

// RegisterWebPushSubscription creates or refreshes one session-owned endpoint.
func (s *Store) RegisterWebPushSubscription(
	ctx context.Context, input NewWebPushSubscription, now time.Time,
) (WebPushSubscription, error) {
	if input.OwnerHumanID != "human:local" || len(input.Endpoint) > 2048 ||
		len(input.P256DH) < 40 || len(input.P256DH) > 256 ||
		len(input.AuthSecret) < 16 || len(input.AuthSecret) > 128 {
		return WebPushSubscription{}, errors.New("invalid Web Push subscription")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return WebPushSubscription{}, err
	}
	defer func() { _ = tx.Rollback() }()
	var active bool
	if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM browser_sessions
WHERE session_hash = ? AND state = 'authenticated' AND expires_at_ms > ?)`,
		input.SessionHash[:], millis(now)).Scan(&active); err != nil {
		return WebPushSubscription{}, err
	}
	if !active {
		return WebPushSubscription{}, ErrSessionUnauthorized
	}
	current, found, err := webPushSubscriptionByEndpoint(ctx, tx, input.Endpoint)
	if err != nil {
		return WebPushSubscription{}, err
	}
	if found && (current.OwnerHumanID != input.OwnerHumanID || current.SessionHash != input.SessionHash) {
		if _, err := tx.ExecContext(ctx, "DELETE FROM web_push_subscriptions WHERE subscription_id = ?", current.ID); err != nil {
			return WebPushSubscription{}, err
		}
		found = false
	}
	if found {
		_, err = tx.ExecContext(ctx, `UPDATE web_push_subscriptions SET p256dh = ?, auth_secret = ?,
revision = revision + CASE WHEN p256dh <> ? OR auth_secret <> ? THEN 1 ELSE 0 END,
updated_at_ms = ? WHERE subscription_id = ?`, input.P256DH, input.AuthSecret,
			input.P256DH, input.AuthSecret, millis(now), current.ID)
	} else {
		current.ID, err = newID("push_subscription")
		if err == nil {
			_, err = tx.ExecContext(ctx, `INSERT INTO web_push_subscriptions
(subscription_id, owner_human_id, browser_session_hash, endpoint, p256dh, auth_secret,
 revision, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)`,
				current.ID, input.OwnerHumanID, input.SessionHash[:], input.Endpoint,
				input.P256DH, input.AuthSecret, millis(now), millis(now))
		}
	}
	if err != nil {
		return WebPushSubscription{}, err
	}
	current, found, err = webPushSubscriptionByEndpoint(ctx, tx, input.Endpoint)
	if err != nil || !found {
		return WebPushSubscription{}, errors.Join(err, errors.New("registered Web Push subscription is unavailable"))
	}
	if err := tx.Commit(); err != nil {
		return WebPushSubscription{}, err
	}
	return current, nil
}

// WebPushSubscriptionID returns the caller-owned identifier for one endpoint.
func (s *Store) WebPushSubscriptionID(
	ctx context.Context, owner string, session [32]byte, endpoint string,
) (string, error) {
	var id string
	err := s.db.QueryRowContext(ctx, `SELECT subscription_id FROM web_push_subscriptions
WHERE owner_human_id = ? AND browser_session_hash = ? AND endpoint = ?`,
		owner, session[:], endpoint).Scan(&id)
	if errors.Is(err, sql.ErrNoRows) {
		return "", nil
	}
	return id, err
}

// OwnsWebPushSubscription reports whether the session owns one active registration.
func (s *Store) OwnsWebPushSubscription(
	ctx context.Context, owner string, session [32]byte, id string,
) (bool, error) {
	var exists bool
	err := s.db.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM web_push_subscriptions
WHERE subscription_id = ? AND owner_human_id = ? AND browser_session_hash = ?)`,
		id, owner, session[:]).Scan(&exists)
	return exists, err
}

// RemoveWebPushSubscription removes one caller-owned registration and its deliveries.
func (s *Store) RemoveWebPushSubscription(
	ctx context.Context, owner string, session [32]byte, id string,
) (bool, error) {
	result, err := s.db.ExecContext(ctx, `DELETE FROM web_push_subscriptions
WHERE subscription_id = ? AND owner_human_id = ? AND browser_session_hash = ?`,
		id, owner, session[:])
	if err != nil {
		return false, err
	}
	changed, err := result.RowsAffected()
	return changed == 1, err
}

// QueueWebPushNotification fans one event to active browser registrations once.
func (s *Store) QueueWebPushNotification(
	ctx context.Context, value WebPushNotification, visible map[string]struct{}, now time.Time,
) error {
	if err := validateWebPushNotification(value); err != nil {
		return err
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	rows, err := tx.QueryContext(ctx, `SELECT subscription_id FROM web_push_subscriptions
WHERE owner_human_id = 'human:local' ORDER BY subscription_id`)
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
		if _, ok := visible[id]; ok {
			status = "suppressed"
		}
		if _, err := tx.ExecContext(ctx, `INSERT INTO web_push_deliveries
(subscription_id, event_key, title, body, navigate_path, urgency, ttl_seconds,
 status, available_at_ms, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(subscription_id, event_key) DO UPDATE SET
 title = CASE WHEN status = 'pending' THEN excluded.title ELSE title END,
 body = CASE WHEN status = 'pending' THEN excluded.body ELSE body END,
 updated_at_ms = excluded.updated_at_ms`, id, value.EventKey, value.Title, value.Body,
			value.NavigatePath, value.Urgency, value.TTLSeconds, status,
			millis(now.Add(time.Second)), millis(now), millis(now)); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// ClaimDueWebPushDelivery claims one due delivery and increments its attempt.
func (s *Store) ClaimDueWebPushDelivery(ctx context.Context, now time.Time) (*WebPushDelivery, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, err
	}
	defer func() { _ = tx.Rollback() }()
	_, err = tx.ExecContext(ctx, `UPDATE web_push_deliveries SET status = 'failed',
last_error_code = 'expired', updated_at_ms = ? WHERE status = 'pending'
AND created_at_ms + ttl_seconds * 1000 <= ?`, millis(now), millis(now))
	if err != nil {
		return nil, err
	}
	var id, key string
	err = tx.QueryRowContext(ctx, `SELECT subscription_id, event_key FROM web_push_deliveries
WHERE status = 'pending' AND available_at_ms <= ?
ORDER BY available_at_ms, subscription_id, event_key LIMIT 1`, millis(now)).Scan(&id, &key)
	if errors.Is(err, sql.ErrNoRows) {
		if err := tx.Commit(); err != nil {
			return nil, err
		}
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE web_push_deliveries SET attempt_count = attempt_count + 1,
updated_at_ms = ? WHERE subscription_id = ? AND event_key = ?`, millis(now), id, key); err != nil {
		return nil, err
	}
	value, err := scanWebPushDelivery(tx.QueryRowContext(ctx, `SELECT
s.subscription_id, s.owner_human_id, s.browser_session_hash, s.endpoint, s.p256dh,
s.auth_secret, s.revision, d.event_key, d.title, d.body, d.navigate_path, d.urgency,
d.ttl_seconds, d.attempt_count FROM web_push_deliveries d
JOIN web_push_subscriptions s USING (subscription_id)
WHERE d.subscription_id = ? AND d.event_key = ?`, id, key))
	if err != nil {
		return nil, err
	}
	return &value, tx.Commit()
}

// FinishWebPushDelivery saves one transport result against the claimed registration revision.
func (s *Store) FinishWebPushDelivery(
	ctx context.Context, value WebPushDelivery, outcome WebPushDeliveryOutcome,
	errorCode string, now time.Time,
) error {
	if errorCode != "" && (len(errorCode) > 128 || strings.TrimSpace(errorCode) != errorCode) {
		return errors.New("invalid Web Push error code")
	}
	now = now.UTC()
	if outcome == WebPushExpired {
		_, err := s.db.ExecContext(ctx, `DELETE FROM web_push_subscriptions
WHERE subscription_id = ? AND revision = ?`, value.Subscription.ID, value.Subscription.Revision)
		return err
	}
	status := string(outcome)
	available := now
	if outcome == WebPushRetry {
		status = "pending"
		delay := time.Minute
		switch value.Attempt {
		case 2:
			delay = 5 * time.Minute
		case 3:
			delay = 30 * time.Minute
		default:
			if value.Attempt >= 4 {
				status = "failed"
			}
		}
		available = now.Add(delay)
	} else if outcome != WebPushDelivered && outcome != WebPushSuppressed && outcome != WebPushFailed {
		return errors.New("invalid Web Push delivery outcome")
	}
	_, err := s.db.ExecContext(ctx, `UPDATE web_push_deliveries SET status = ?, available_at_ms = ?,
last_error_code = NULLIF(?, ''), updated_at_ms = ? WHERE subscription_id = ? AND event_key = ?
AND EXISTS (SELECT 1 FROM web_push_subscriptions WHERE subscription_id = ? AND revision = ?)`,
		status, millis(available), errorCode, millis(now), value.Subscription.ID,
		value.Notification.EventKey, value.Subscription.ID, value.Subscription.Revision)
	return err
}

// NextWebPushDelivery returns the next retry or send time.
func (s *Store) NextWebPushDelivery(ctx context.Context) (*time.Time, error) {
	var value sql.NullInt64
	if err := s.db.QueryRowContext(ctx, `SELECT MIN(available_at_ms) FROM web_push_deliveries
WHERE status = 'pending'`).Scan(&value); err != nil {
		return nil, err
	}
	return nullTimePointer(value), nil
}

// WebPushPrimarySource returns the current primary Chat and its latest item sequence.
func (s *Store) WebPushPrimarySource(ctx context.Context) (string, int64, error) {
	var id string
	var sequence int64
	err := s.db.QueryRowContext(ctx, `SELECT c.conversation_id,
COALESCE(MAX(i.sequence_index), 0) FROM local_human_state h
JOIN conversations c ON c.conversation_id = h.primary_conversation_id
LEFT JOIN conversation_items i ON i.conversation_id = c.conversation_id
WHERE h.state_id = 1 GROUP BY c.conversation_id`).Scan(&id, &sequence)
	if errors.Is(err, sql.ErrNoRows) {
		return "", 0, nil
	}
	return id, sequence, err
}

// WebPushPrimaryCheckpoint returns the durable primary Chat scan position.
func (s *Store) WebPushPrimaryCheckpoint(ctx context.Context) (WebPushPrimaryCheckpoint, error) {
	var value WebPushPrimaryCheckpoint
	err := s.db.QueryRowContext(ctx, `SELECT COALESCE(primary_conversation_id, ''), primary_sequence
FROM web_push_projection_state WHERE state_id = 1`).Scan(&value.ConversationID, &value.Sequence)
	return value, err
}

// AdvanceWebPushPrimaryCheckpoint saves a monotonic primary Chat scan position.
func (s *Store) AdvanceWebPushPrimaryCheckpoint(
	ctx context.Context, conversationID string, sequence int64, now time.Time,
) error {
	if conversationID == "" || sequence < 0 {
		return errors.New("invalid Web Push checkpoint")
	}
	_, err := s.db.ExecContext(ctx, `UPDATE web_push_projection_state SET
primary_conversation_id = ?, primary_sequence = CASE
WHEN primary_conversation_id = ? THEN max(primary_sequence, ?) ELSE ? END,
updated_at_ms = ? WHERE state_id = 1`, conversationID, conversationID,
		sequence, sequence, millis(now.UTC()))
	return err
}

// WebPushPrimaryItems returns one ordered notification scan batch.
func (s *Store) WebPushPrimaryItems(
	ctx context.Context, conversationID string, after int64, limit int,
) ([]ConversationItem, error) {
	if after < 0 || limit < 1 || limit > 100 {
		return nil, errors.New("invalid Web Push item page")
	}
	rows, err := s.db.QueryContext(ctx, `SELECT item_id, conversation_id,
COALESCE(turn_id, ''), COALESCE(parent_item_id, ''), sequence_index, kind, status,
author_actor_id, COALESCE(content_text, ''), COALESCE(provider_content_text, ''),
payload_json, metadata_json, created_at_ms FROM conversation_items
WHERE conversation_id = ? AND sequence_index > ? ORDER BY sequence_index LIMIT ?`,
		conversationID, after, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	items := make([]ConversationItem, 0, limit)
	for rows.Next() {
		item, err := scanConversationItem(rows)
		if err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	return items, rows.Err()
}

func webPushSubscriptionByEndpoint(
	ctx context.Context, query rowQuery, endpoint string,
) (WebPushSubscription, bool, error) {
	var value WebPushSubscription
	var hash []byte
	err := query.QueryRowContext(ctx, `SELECT subscription_id, owner_human_id,
browser_session_hash, endpoint, p256dh, auth_secret, revision
FROM web_push_subscriptions WHERE endpoint = ?`, endpoint).Scan(&value.ID,
		&value.OwnerHumanID, &hash, &value.Endpoint, &value.P256DH, &value.AuthSecret, &value.Revision)
	if errors.Is(err, sql.ErrNoRows) {
		return WebPushSubscription{}, false, nil
	}
	if err != nil || len(hash) != 32 {
		return WebPushSubscription{}, false, errors.Join(err, errors.New("invalid stored Web Push session"))
	}
	copy(value.SessionHash[:], hash)
	return value, true, nil
}

func scanWebPushDelivery(row rowScanner) (WebPushDelivery, error) {
	var value WebPushDelivery
	var hash []byte
	err := row.Scan(&value.Subscription.ID, &value.Subscription.OwnerHumanID, &hash,
		&value.Subscription.Endpoint, &value.Subscription.P256DH, &value.Subscription.AuthSecret,
		&value.Subscription.Revision, &value.Notification.EventKey, &value.Notification.Title,
		&value.Notification.Body, &value.Notification.NavigatePath, &value.Notification.Urgency,
		&value.Notification.TTLSeconds, &value.Attempt)
	if err != nil || len(hash) != 32 {
		return WebPushDelivery{}, errors.Join(err, errors.New("invalid stored Web Push delivery"))
	}
	copy(value.Subscription.SessionHash[:], hash)
	return value, nil
}

func validateWebPushNotification(value WebPushNotification) error {
	parsed, err := url.ParseRequestURI(value.NavigatePath)
	if strings.TrimSpace(value.EventKey) == "" || len(value.EventKey) > 256 ||
		strings.TrimSpace(value.Title) == "" || len(value.Title) > 600 || len(value.Body) > 2048 ||
		err != nil || parsed.IsAbs() || parsed.Host != "" || !strings.HasPrefix(value.NavigatePath, "/") ||
		strings.HasPrefix(value.NavigatePath, "//") ||
		(value.Urgency != "normal" && value.Urgency != "high") ||
		value.TTLSeconds < 0 || value.TTLSeconds > 604800 {
		return errors.New("invalid Web Push notification")
	}
	return nil
}

type rowQuery interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func (value WebPushSubscription) String() string {
	return fmt.Sprintf("WebPushSubscription{ID:%q, OwnerHumanID:%q, Revision:%d}",
		value.ID, value.OwnerHumanID, value.Revision)
}

func (value WebPushSubscription) GoString() string { return value.String() }

func (value NewWebPushSubscription) String() string {
	return fmt.Sprintf("NewWebPushSubscription{OwnerHumanID:%q}", value.OwnerHumanID)
}

func (value NewWebPushSubscription) GoString() string { return value.String() }
