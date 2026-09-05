package store

import (
	"context"
	"database/sql"
	"fmt"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestNativeNotificationSchemaConvergesFromFreshV14AndV15(t *testing.T) {
	for _, version := range []int{0, 14, 15} {
		t.Run(fmt.Sprint("v", version), func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "noema.sqlite3")
			if version > 0 {
				legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
				if err != nil {
					t.Fatal(err)
				}
				if _, err := legacy.Exec(schemaAtVersion(version) + fmt.Sprintf("\nPRAGMA user_version = %d;", version)); err != nil {
					t.Fatal(err)
				}
				if err := legacy.Close(); err != nil {
					t.Fatal(err)
				}
			}
			database, err := Open(context.Background(), path)
			if err != nil {
				t.Fatal(err)
			}
			defer database.Close()
			var current int
			if err := database.db.QueryRow("PRAGMA user_version").Scan(&current); err != nil || current != 16 {
				t.Fatalf("schema version = %d, %v", current, err)
			}
			for _, table := range []string{"task_runs", "client_notification_registrations", "apns_deliveries", "client_live_activity_registrations"} {
				var count int
				if err := database.db.QueryRow("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name=?", table).Scan(&count); err != nil || count != 1 {
					t.Fatalf("table %s = %d, %v", table, count, err)
				}
			}
		})
	}
}

func TestNativeNotificationOwnershipDeliveryAndRevocation(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	_, refreshA := seedNativeFamily(t, database, testNativeClient, "6", now.Unix())
	clientB := "noema-ios:abcdefghijklmnop"
	_, _ = seedNativeFamily(t, database, clientB, "7", now.Unix())
	token := []byte("native-device-token")
	if err := database.RegisterClientNotifications(ctx, testNativeClient, token, APNSDevelopment, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterClientNotifications(ctx, testNativeClient, token, APNSDevelopment, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	registration, err := database.ClientNotificationRegistration(ctx, testNativeClient)
	if err != nil || registration == nil || registration.Revision != 1 {
		t.Fatalf("repeat registration = %#v, %v", registration, err)
	}
	if err := database.QueueAPNSNotification(ctx, APNSNotification{EventKey: "chat-turn:1", Title: "Noema", Body: "Done",
		Route: "chat", Urgency: "normal", TTLSeconds: 3600}, map[string]struct{}{testNativeClient: {}}, now); err != nil {
		t.Fatal(err)
	}
	if claimed, err := database.ClaimDueAPNSDelivery(ctx, now.Add(time.Minute)); err != nil || claimed != nil {
		t.Fatalf("visible client delivery = %#v, %v", claimed, err)
	}
	if err := database.QueueAPNSNotification(ctx, APNSNotification{EventKey: "chat-turn:2", Title: "Noema", Body: "Done",
		Route: "chat", Urgency: "normal", TTLSeconds: 3600}, nil, now); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimDueAPNSDelivery(ctx, now.Add(time.Minute))
	if err != nil || claimed == nil || claimed.Attempt != 1 {
		t.Fatalf("claimed delivery = %#v, %v", claimed, err)
	}
	if strings.Contains(fmt.Sprintf("%#v", claimed), string(token)) {
		t.Fatal("delivery diagnostic exposed device token")
	}
	if err := database.FinishAPNSDelivery(ctx, *claimed, APNSRetry, "remote_retry", "", now.Add(time.Minute)); err != nil {
		t.Fatal(err)
	}
	claimed, err = database.ClaimDueAPNSDelivery(ctx, now.Add(3*time.Minute))
	if err != nil || claimed == nil || claimed.Attempt != 2 {
		t.Fatalf("retry delivery = %#v, %v", claimed, err)
	}
	if err := database.FinishAPNSDelivery(ctx, *claimed, APNSInvalid, "", "", now.Add(3*time.Minute)); err != nil {
		t.Fatal(err)
	}
	if registration, err := database.ClientNotificationRegistration(ctx, testNativeClient); err != nil || registration != nil {
		t.Fatalf("invalid token registration = %#v, %v", registration, err)
	}
	if err := database.RegisterClientNotifications(ctx, testNativeClient, token, APNSProduction, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterClientNotifications(ctx, clientB, token, APNSProduction, now); err != nil {
		t.Fatal(err)
	}
	if registration, _ := database.ClientNotificationRegistration(ctx, testNativeClient); registration != nil {
		t.Fatal("transferred token retained old owner")
	}
	if err := database.RegisterClientNotifications(ctx, testNativeClient, []byte("other-token"), APNSProduction, now); err != nil {
		t.Fatal(err)
	}
	if _, _, err := database.RevokeNativeOAuthFamily(ctx, refreshA, now.Add(time.Hour).Unix()); err != nil {
		t.Fatal(err)
	}
	if registration, _ := database.ClientNotificationRegistration(ctx, testNativeClient); registration != nil {
		t.Fatal("OAuth revocation retained notification registration")
	}
}

func TestLiveActivityTokensStayClientOwnedAndDisableCleanly(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	_, refresh := seedNativeFamily(t, database, testNativeClient, "8", now.Unix())
	activity := "live_activity:one"
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("push-to-start"), APNSDevelopment, []string{activity}, now); err != nil {
		t.Fatal(err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient, activity, []byte("update-token"), now); err != nil || !changed {
		t.Fatalf("update registration = %v, %v", changed, err)
	}
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("push-to-start"), APNSDevelopment, []string{activity}, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	var retained []byte
	if err := database.db.QueryRow("SELECT update_token FROM client_live_activities WHERE client_id = ? AND activity_id = ?", testNativeClient, activity).Scan(&retained); err != nil || string(retained) != "update-token" {
		t.Fatalf("retained update token = %q, %v", retained, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient, "live_activity:other", []byte("token"), now); err != nil || changed {
		t.Fatalf("unobserved update registration = %v, %v", changed, err)
	}
	if changed, err := database.DismissClientLiveActivity(ctx, testNativeClient, activity); err != nil || !changed {
		t.Fatalf("dismiss = %v, %v", changed, err)
	}
	if err := database.DisableClientLiveActivities(ctx, testNativeClient, now); err != nil {
		t.Fatal(err)
	}
	registration, err := database.ClientLiveActivityRegistration(ctx, testNativeClient)
	if err != nil || registration == nil || registration.Enabled || registration.Environment != nil || len(registration.PushToStartToken) != 0 {
		t.Fatalf("disabled registration = %#v, %v", registration, err)
	}
	if _, _, err := database.RevokeNativeOAuthFamily(ctx, refresh, now.Add(time.Hour).Unix()); err != nil {
		t.Fatal(err)
	}
	if registration, _ := database.ClientLiveActivityRegistration(ctx, testNativeClient); registration != nil {
		t.Fatal("OAuth revocation retained Live Activity state")
	}
}
