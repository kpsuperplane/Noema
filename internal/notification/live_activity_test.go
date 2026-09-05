package notification

import (
	"bytes"
	"context"
	"io"
	"net/http"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestTaskLiveActivityProjectsStartUpdateAndEnd(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	clientID := seedAPNSClient(t, database)
	service, err := New(paths, database, "https://noema.example")
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	taskID := "task:11111111111111111111111111111111"
	if _, err := database.CreateTask(ctx, taskID, "**Build** the release", "correlation:live-test", testTime); err != nil {
		t.Fatal(err)
	}
	if _, err := database.StartTask(ctx, taskID, "run:11111111111111111111111111111111", testTime.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte("start-token"), store.APNSDevelopment, nil, testTime); err != nil {
		t.Fatal(err)
	}
	targets, err := database.LiveActivityTargets(ctx)
	if err != nil || len(targets) != 1 || targets[0].Activity == nil {
		t.Fatalf("targets = %#v, %v", targets, err)
	}
	projection, err := service.liveProjection(ctx)
	if err != nil || projection == nil || projection.Content["focusTitle"] != "Build the release" ||
		projection.Content["phase"] != "inProgress" || projection.Content["statusLabel"] != "In progress" {
		t.Fatalf("projection = %#v, %v", projection, err)
	}
	if err := service.applyLiveProjection(ctx, targets[0], projection); err != nil {
		t.Fatal(err)
	}
	start, err := database.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Second))
	if err != nil || start == nil || start.Event != store.LiveActivityStart {
		t.Fatalf("start = %#v, %v", start, err)
	}
	aps := start.Payload["aps"].(map[string]any)
	attributes := aps["attributes"].(map[string]any)
	if aps["attributes-type"] != liveActivityAttributesType || attributes["activityId"] != targets[0].Activity.ActivityID ||
		attributes["clientId"] != clientID || attributes["serverOrigin"] != "https://noema.example" {
		t.Fatalf("start payload = %#v", start.Payload)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *start, store.APNSDelivered, "", "start-apns", time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, clientID, start.ActivityID, []byte("update-token"), time.Now()); err != nil || !changed {
		t.Fatalf("update token = %v, %v", changed, err)
	}
	targets, _ = database.LiveActivityTargets(ctx)
	if err := service.applyLiveProjection(ctx, targets[0], projection); err != nil {
		t.Fatal(err)
	}
	update, err := database.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Second))
	if err != nil || update == nil || update.Event != store.LiveActivityUpdate || string(update.Token) != "update-token" {
		t.Fatalf("update = %#v, %v", update, err)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *update, store.APNSDelivered, "", "update-apns", time.Now()); err != nil {
		t.Fatal(err)
	}
	if _, err := database.FinishTask(ctx, taskID, "run:11111111111111111111111111111111", store.TaskCompleted, time.Now()); err != nil {
		t.Fatal(err)
	}
	targets, _ = database.LiveActivityTargets(ctx)
	if err := service.applyLiveProjection(ctx, targets[0], nil); err != nil {
		t.Fatal(err)
	}
	end, err := database.ClaimDueLiveActivityDelivery(ctx, time.Now().Add(time.Second))
	if err != nil || end == nil || end.Event != store.LiveActivityEnd || end.TTLSeconds != 600 {
		t.Fatalf("end = %#v, %v", end, err)
	}
	if end.Payload["aps"].(map[string]any)["content-state"].(map[string]any)["phase"] != "completed" {
		t.Fatalf("terminal payload = %#v", end.Payload)
	}
}

func TestLiveActivityAPNSRequestUsesActivityHeadersAndBoundedPayload(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := New(paths, database, "http://localhost:3737")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.ConfigureAPNS("TEAM123456", "KEYID12345", testAPNSKey(t), 0); err != nil {
		t.Fatal(err)
	}
	var captured *http.Request
	var body []byte
	service.apns = &http.Client{Transport: roundTripFunc(func(request *http.Request) (*http.Response, error) {
		captured = request
		body, _ = io.ReadAll(request.Body)
		header := make(http.Header)
		header.Set("apns-id", "live-apns")
		return &http.Response{StatusCode: http.StatusOK, Header: header, Body: io.NopCloser(bytes.NewReader(nil))}, nil
	})}
	value := store.LiveActivityDelivery{ClientID: "noema-ios:test", DeliveryKey: "live:update:test",
		ActivityID: "live_activity:test", Token: []byte{0, 1, 2}, Environment: store.APNSDevelopment,
		Event: store.LiveActivityUpdate, Payload: map[string]any{"aps": map[string]any{"event": "update"}},
		Urgency: "normal", TTLSeconds: 3600, CreatedAt: time.Now()}
	outcome, code, apnsID, _ := service.sendLiveActivity(context.Background(), value)
	if outcome != store.APNSDelivered || code != "" || apnsID != "live-apns" {
		t.Fatalf("send result = %q %q %q", outcome, code, apnsID)
	}
	if captured.Header.Get("apns-topic") != liveActivityTopic || captured.Header.Get("apns-push-type") != "liveactivity" ||
		captured.Header.Get("apns-priority") != "5" || captured.Header.Get("apns-collapse-id") == "" ||
		!strings.HasSuffix(captured.URL.Path, "/000102") || len(body) > 4096 {
		t.Fatalf("request = %#v, body=%d", captured.Header, len(body))
	}
}

func TestDismissedLiveActivityRestartsForDifferentTask(t *testing.T) {
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(context.Background(), paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	clientID := seedAPNSClient(t, database)
	if err := database.RegisterClientLiveActivities(context.Background(), clientID,
		[]byte("start-token"), store.APNSDevelopment, nil, testTime); err != nil {
		t.Fatal(err)
	}
	activity, _ := database.ClientTaskActivity(context.Background(), clientID)
	oldProjection, err := makeLiveProjection(map[string]any{"focusTaskId": "task:old"}, "task:old")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.UpdateClientTaskActivityProjection(context.Background(), clientID,
		oldProjection.Content, oldProjection.Signature, oldProjection.FocusedTaskID, testTime); err != nil {
		t.Fatal(err)
	}
	if changed, err := database.DismissClientLiveActivity(context.Background(), clientID, activity.ActivityID); err != nil || !changed {
		t.Fatalf("dismiss = %v, %v", changed, err)
	}
	targets, _ := database.LiveActivityTargets(context.Background())
	newProjection, err := makeLiveProjection(map[string]any{"focusTaskId": "task:new"}, "task:new")
	if err != nil {
		t.Fatal(err)
	}
	service, err := New(paths, database, "https://noema.example")
	if err != nil {
		t.Fatal(err)
	}
	if err := service.reconcileLiveActivities(context.Background()); err != nil {
		t.Fatal(err)
	}
	if delivery, err := database.ClaimDueLiveActivityDelivery(context.Background(), time.Now()); err != nil || delivery != nil {
		t.Fatalf("unconfigured delivery = %#v, %v", delivery, err)
	}
	if err := service.applyLiveProjection(context.Background(), targets[0], newProjection); err != nil {
		t.Fatal(err)
	}
	replacement, err := database.ClientTaskActivity(context.Background(), clientID)
	if err != nil || replacement == nil || replacement.Lifecycle != "starting" || replacement.Suppressed ||
		replacement.TaskSessionID == activity.TaskSessionID || replacement.FocusedTaskID != "task:new" {
		t.Fatalf("replacement = %#v, %v", replacement, err)
	}
}

func TestLiveTranscriptUpdateUsesOldestRunningToolAndLatestCompletedLine(t *testing.T) {
	text := func(value string) *string { return &value }
	base := testTime
	items := []store.TaskRunItem{
		{Sequence: 4, Round: 2, Kind: "tool_call", Status: "running", Payload: map[string]any{"name": "task.files.write", "arguments": map[string]any{"path": "RESULT.md"}}, UpdatedAt: base.Add(4 * time.Second)},
		{Sequence: 3, Round: 2, Kind: "assistant_output", Status: "completed", Content: text("Preparing the next action."), UpdatedAt: base.Add(3 * time.Second)},
		{Sequence: 2, Round: 1, Kind: "tool_call", Status: "running", Payload: map[string]any{"name": "task.files.read", "arguments": map[string]any{"path": "TASK.md"}}, UpdatedAt: base.Add(2 * time.Second)},
		{Sequence: 1, Round: 1, Kind: "assistant_output", Status: "completed", Content: text("Preparing the first action."), UpdatedAt: base.Add(time.Second)},
	}
	label, updated := liveTranscriptUpdate(items)
	if label != "Reading TASK.md" || updated == nil || !updated.Equal(base.Add(2*time.Second)) {
		t.Fatalf("active update = %#v, %v", label, updated)
	}
	items[0].Status = "completed"
	items[2].Status = "completed"
	label, updated = liveTranscriptUpdate(items)
	if label != "Preparing the next action." || updated == nil || !updated.Equal(base.Add(3*time.Second)) {
		t.Fatalf("completed update = %#v, %v", label, updated)
	}
}

type roundTripFunc func(*http.Request) (*http.Response, error)

func (fn roundTripFunc) RoundTrip(request *http.Request) (*http.Response, error) { return fn(request) }
