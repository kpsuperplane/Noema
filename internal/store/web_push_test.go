package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/json"
	"fmt"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestWebPushSchemaRegistrationOwnershipAndSecretSinks(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "version-thirteen.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(schemaAtVersion(13) + "\nPRAGMA user_version = 13;"); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	database, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	var version int
	if err := database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("schema version = %d, %v", version, err)
	}
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	session := addAuthenticatedPushSession(t, database, "owner", now)
	input := NewWebPushSubscription{OwnerHumanID: "human:local", SessionHash: session,
		Endpoint: "https://push.example/secret-endpoint", P256DH: strings.Repeat("p", 64),
		AuthSecret: strings.Repeat("a", 24)}
	first, err := database.RegisterWebPushSubscription(ctx, input, now)
	if err != nil {
		t.Fatal(err)
	}
	second, err := database.RegisterWebPushSubscription(ctx, input, now.Add(time.Second))
	if err != nil || second.ID != first.ID || second.Revision != first.Revision {
		t.Fatalf("repeat registration = %#v, %v", second, err)
	}
	wrong := sha256.Sum256([]byte("wrong-session"))
	foreign := input
	foreign.SessionHash = wrong
	if _, err := database.RegisterWebPushSubscription(ctx, foreign, now); err == nil {
		t.Fatal("unauthenticated session registered browser Push")
	}
	if id, err := database.WebPushSubscriptionID(ctx, "human:local", wrong, input.Endpoint); err != nil || id != "" {
		t.Fatalf("foreign status = %q, %v", id, err)
	}
	if removed, err := database.RemoveWebPushSubscription(ctx, "human:local", wrong, first.ID); err != nil || removed {
		t.Fatalf("foreign removal = %v, %v", removed, err)
	}
	encoded, err := json.Marshal(first)
	if err != nil {
		t.Fatal(err)
	}
	encodedInput, err := json.Marshal(input)
	if err != nil {
		t.Fatal(err)
	}
	for _, sink := range []string{string(encoded), string(encodedInput), fmt.Sprintf("%v", first),
		fmt.Sprintf("%#v", first), fmt.Sprintf("%v", input), fmt.Sprintf("%#v", input)} {
		if strings.Contains(sink, "secret-endpoint") || strings.Contains(sink, input.AuthSecret) || strings.Contains(sink, input.P256DH) {
			t.Fatalf("secret-bearing subscription escaped sink: %s", sink)
		}
	}
}

func TestWebPushSessionCleanupTransferAndExpiration(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	firstSession := addAuthenticatedPushSession(t, database, "first", now)
	endpoint := "https://push.example/one"
	first, err := database.RegisterWebPushSubscription(ctx, NewWebPushSubscription{
		OwnerHumanID: "human:local", SessionHash: firstSession, Endpoint: endpoint,
		P256DH: strings.Repeat("p", 64), AuthSecret: strings.Repeat("a", 24)}, now)
	if err != nil {
		t.Fatal(err)
	}
	if err := database.QueueWebPushNotification(ctx, pushTestNotification("event:first"), nil, now); err != nil {
		t.Fatal(err)
	}
	if err := database.DeleteBrowserSession(ctx, firstSession); err != nil {
		t.Fatal(err)
	}
	if exists := webPushSubscriptionExists(t, database, first.ID); exists {
		t.Fatal("logout retained browser Push subscription")
	}

	secondSession := addAuthenticatedPushSession(t, database, "second", now)
	second, err := database.RegisterWebPushSubscription(ctx, NewWebPushSubscription{
		OwnerHumanID: "human:local", SessionHash: secondSession, Endpoint: endpoint,
		P256DH: strings.Repeat("q", 64), AuthSecret: strings.Repeat("b", 24)}, now)
	if err != nil {
		t.Fatal(err)
	}
	thirdSession := addAuthenticatedPushSession(t, database, "third", now)
	third, err := database.RegisterWebPushSubscription(ctx, NewWebPushSubscription{
		OwnerHumanID: "human:local", SessionHash: thirdSession, Endpoint: endpoint,
		P256DH: strings.Repeat("r", 64), AuthSecret: strings.Repeat("c", 24)}, now)
	if err != nil || third.ID == second.ID || webPushSubscriptionExists(t, database, second.ID) {
		t.Fatalf("endpoint transfer = %#v, %v", third, err)
	}
	if _, err := database.db.Exec("UPDATE browser_sessions SET expires_at_ms = ? WHERE session_hash = ?",
		millis(now.Add(time.Second)), thirdSession[:]); err != nil {
		t.Fatal(err)
	}
	if _, err := database.DeleteExpiredBrowserSessions(ctx, now.Add(2*time.Second)); err != nil {
		t.Fatal(err)
	}
	if webPushSubscriptionExists(t, database, third.ID) {
		t.Fatal("session expiration retained browser Push subscription")
	}
}

func TestWebPushDeliveryRetryResultsAndRevisionGuard(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	session := addAuthenticatedPushSession(t, database, "delivery", now)
	input := NewWebPushSubscription{OwnerHumanID: "human:local", SessionHash: session,
		Endpoint: "https://push.example/delivery", P256DH: strings.Repeat("p", 64),
		AuthSecret: strings.Repeat("a", 24)}
	subscription, err := database.RegisterWebPushSubscription(ctx, input, now)
	if err != nil {
		t.Fatal(err)
	}
	if err := database.QueueWebPushNotification(ctx, pushTestNotification("event:retry"), nil, now); err != nil {
		t.Fatal(err)
	}
	claim, err := database.ClaimDueWebPushDelivery(ctx, now.Add(2*time.Second))
	if err != nil || claim == nil || claim.Attempt != 1 {
		t.Fatalf("first claim = %#v, %v", claim, err)
	}
	if err := database.FinishWebPushDelivery(ctx, *claim, WebPushRetry, "remote_retry", now.Add(2*time.Second)); err != nil {
		t.Fatal(err)
	}
	cursor := now.Add(2 * time.Second)
	for attempt, delay := range []time.Duration{time.Minute, 5 * time.Minute, 30 * time.Minute} {
		cursor = cursor.Add(delay + time.Second)
		claim, err = database.ClaimDueWebPushDelivery(ctx, cursor)
		if err != nil || claim == nil || claim.Attempt != attempt+2 {
			t.Fatalf("retry claim %d = %#v, %v", attempt+2, claim, err)
		}
		if err := database.FinishWebPushDelivery(ctx, *claim, WebPushRetry, "remote_retry", cursor); err != nil {
			t.Fatal(err)
		}
	}
	if next, err := database.NextWebPushDelivery(ctx); err != nil || next != nil {
		t.Fatalf("terminal retry = %v, %v", next, err)
	}
	if err := database.QueueWebPushNotification(ctx, pushTestNotification("event:expired"), nil, now); err != nil {
		t.Fatal(err)
	}
	old, err := database.ClaimDueWebPushDelivery(ctx, now.Add(2*time.Second))
	if err != nil || old == nil {
		t.Fatalf("expired claim = %#v, %v", old, err)
	}
	input.P256DH = strings.Repeat("z", 64)
	rotated, err := database.RegisterWebPushSubscription(ctx, input, now.Add(3*time.Second))
	if err != nil || rotated.Revision != subscription.Revision+1 {
		t.Fatalf("key rotation = %#v, %v", rotated, err)
	}
	if err := database.FinishWebPushDelivery(ctx, *old, WebPushExpired, "expired", now.Add(4*time.Second)); err != nil {
		t.Fatal(err)
	}
	if !webPushSubscriptionExists(t, database, subscription.ID) {
		t.Fatal("stale expired response removed refreshed subscription")
	}
}

func addAuthenticatedPushSession(t *testing.T, database *Store, label string, now time.Time) [32]byte {
	t.Helper()
	session := sha256.Sum256([]byte(label + ":authenticated"))
	credentialID := "push-test-" + label
	if _, err := database.db.Exec(`INSERT INTO human_passkeys
(credential_id, credential_json, created_at_ms, updated_at_ms) VALUES (?, '{}', ?, ?)`,
		credentialID, millis(now), millis(now)); err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.Exec(`INSERT INTO browser_sessions
(session_hash, state, passkey_id, recent_passkey_at_ms, created_at_ms, expires_at_ms)
VALUES (?, 'authenticated', ?, ?, ?, ?)`, session[:], credentialID, millis(now),
		millis(now), millis(now.Add(24*time.Hour))); err != nil {
		t.Fatal(err)
	}
	return session
}

func pushTestNotification(key string) WebPushNotification {
	return WebPushNotification{EventKey: key, Title: "Noema", Body: "Done",
		NavigatePath: "/", Urgency: "normal", TTLSeconds: 3600}
}

func webPushSubscriptionExists(t *testing.T, database *Store, id string) bool {
	t.Helper()
	var exists bool
	if err := database.db.QueryRow("SELECT EXISTS(SELECT 1 FROM web_push_subscriptions WHERE subscription_id = ?)", id).Scan(&exists); err != nil {
		t.Fatal(err)
	}
	return exists
}
