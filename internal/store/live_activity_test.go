package store

import (
	"context"
	"strings"
	"testing"
	"time"
)

func TestLiveActivityLifecycleFencesTokensAndDismissals(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("start-one"), APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, testNativeClient)
	if err != nil || activity == nil || activity.Lifecycle != "starting" || activity.TaskSessionID == "" {
		t.Fatalf("starting state = %#v, %v", activity, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient,
		"live_activity:stale", []byte("wrong"), now); err != nil || changed {
		t.Fatalf("stale update = %v, %v", changed, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient,
		activity.ActivityID, []byte("update-one"), now); err != nil || !changed {
		t.Fatalf("current update = %v, %v", changed, err)
	}
	if changed, err := database.DismissClientLiveActivity(ctx, testNativeClient, activity.ActivityID); err != nil || !changed {
		t.Fatalf("dismissal = %v, %v", changed, err)
	}
	dismissed, err := database.ClientTaskActivity(ctx, testNativeClient)
	if err != nil || dismissed == nil || dismissed.Lifecycle != "dismissed" || !dismissed.Suppressed ||
		dismissed.ProjectionSignature != strings.Repeat("0", 64) {
		t.Fatalf("dismissed state = %#v, %v", dismissed, err)
	}
	var observations int
	if err := database.db.QueryRow("SELECT COUNT(*) FROM live_activity_observations WHERE client_id=?", testNativeClient).Scan(&observations); err != nil || observations != 3 {
		t.Fatalf("observations = %d, %v", observations, err)
	}
}

func TestLiveActivityDeliveryRetriesAndInvalidationUseExactToken(t *testing.T) {
	database := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)
	_, _ = seedNativeFamily(t, database, testNativeClient, "b", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("start-one"), APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, _ := database.ClientTaskActivity(ctx, testNativeClient)
	payload := map[string]any{"aps": map[string]any{"event": "start"}}
	value := NewLiveActivityDelivery{ClientID: testNativeClient, DeliveryKey: "live:start:" + activity.TaskSessionID,
		ActivityID: activity.ActivityID, Token: []byte("start-one"), Environment: APNSProduction,
		Event: LiveActivityStart, Payload: payload, Urgency: "high", TTLSeconds: 3600}
	if err := database.QueueLiveActivityDelivery(ctx, value, now); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimDueLiveActivityDelivery(ctx, now)
	if err != nil || claimed == nil || claimed.Attempt != 1 {
		t.Fatalf("first claim = %#v, %v", claimed, err)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *claimed, APNSRetry, "remote_retry", "", now); err != nil {
		t.Fatal(err)
	}
	claimed, err = database.ClaimDueLiveActivityDelivery(ctx, now.Add(2*time.Minute))
	if err != nil || claimed == nil || claimed.Attempt != 2 {
		t.Fatalf("retry claim = %#v, %v", claimed, err)
	}
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("start-two"), APNSProduction, nil, now.Add(3*time.Minute)); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *claimed, APNSInvalid, "", "", now.Add(4*time.Minute)); err != nil {
		t.Fatal(err)
	}
	registration, err := database.ClientLiveActivityRegistration(ctx, testNativeClient)
	if err != nil || registration == nil || string(registration.PushToStartToken) != "start-two" {
		t.Fatalf("stale invalidation changed token = %#v, %v", registration, err)
	}
}
