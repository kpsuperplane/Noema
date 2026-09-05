package notification

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/sha256"
	"crypto/x509"
	"encoding/hex"
	"encoding/pem"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestAPNSConfigurationIsProtectedRevisionedAndSecretSafe(t *testing.T) {
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
	key := testAPNSKey(t)
	status, err := service.ConfigureAPNS("TEAM123456", "KEYID12345", key, 0)
	if err != nil || !status.Configured || status.Revision != 1 || status.KeyFingerprint == nil {
		t.Fatalf("configured status = %#v, %v", status, err)
	}
	if _, err := service.ConfigureAPNS("TEAM123456", "KEYID12345", key, 0); err == nil {
		t.Fatal("stale provider revision was accepted")
	}
	credential, err := readAPNSCredential(paths.APNSProvider())
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(fmt.Sprintf("%#v", credential), "PRIVATE KEY") || credential.status().Topic != apnsTopic {
		t.Fatal("provider diagnostic exposed the private key")
	}
	info, err := os.Stat(paths.APNSProvider())
	if err != nil {
		t.Fatal(err)
	}
	if runtime.GOOS != "windows" && info.Mode().Perm() != 0o600 {
		t.Fatalf("provider file mode = %o", info.Mode().Perm())
	}
	removed, err := service.RemoveAPNS(context.Background(), 1)
	if err != nil || removed.Configured || removed.Revision != 2 {
		t.Fatalf("removed status = %#v, %v", removed, err)
	}
	data, err := os.ReadFile(paths.APNSProvider())
	if err != nil || strings.Contains(string(data), "PRIVATE KEY") {
		t.Fatalf("provider tombstone retained a private key: %v", err)
	}
}

func TestAPNSTokenAndJWTWireEncoding(t *testing.T) {
	privatePEM := testAPNSKey(t)
	private, err := parseAPNSKey(privatePEM)
	if err != nil {
		t.Fatal(err)
	}
	token, err := makeAPNSJWT(private, "TEAM123456", "KEYID12345", testTime)
	if err != nil || strings.Count(token, ".") != 2 || strings.Contains(token, "=") {
		t.Fatalf("provider token shape = %q, %v", token, err)
	}
	raw := []byte{0, 1, 2, 250, 255}
	encoded := "AAEC-v8"
	decoded, err := decodeAPNSToken(encoded)
	if err != nil || string(decoded) != string(raw) {
		t.Fatalf("decoded token = %x, %v", decoded, err)
	}
	for _, invalid := range []string{"", "AAEC+v8", "AAEC-v8=", " AAEC-v8"} {
		if _, err := decodeAPNSToken(invalid); err == nil {
			t.Fatalf("accepted token %q", invalid)
		}
	}
	checks := []struct {
		status  int
		body    string
		outcome store.APNSDeliveryOutcome
		code    string
	}{
		{200, "", store.APNSDelivered, ""},
		{403, "", store.APNSFailed, "provider_auth_rejected"},
		{429, "", store.APNSRetry, "remote_retry"},
		{410, `{"reason":"Unregistered"}`, store.APNSInvalid, ""},
		{400, `{"reason":"BadCollapseId"}`, store.APNSFailed, "remote_rejected"},
	}
	for _, check := range checks {
		outcome, code := apnsResponseOutcome(check.status, []byte(check.body))
		if outcome != check.outcome || code != check.code {
			t.Fatalf("status %d = %q %q", check.status, outcome, code)
		}
	}
	empty, err := marshalAPNSPayload(map[string]string{"value": ""})
	if err != nil {
		t.Fatal(err)
	}
	value := "<&>" + strings.Repeat("x", 4096-len(empty)-3)
	boundary, err := marshalAPNSPayload(map[string]string{"value": value})
	if err != nil || len(boundary) != 4096 || !bytes.Contains(boundary, []byte("<&>")) {
		t.Fatalf("boundary payload = %d bytes, %v", len(boundary), err)
	}
	if _, err := marshalAPNSPayload(map[string]string{"value": value + "x"}); err == nil {
		t.Fatal("oversized APNs payload was accepted")
	}
}

func TestTaskAttentionQueuesNativeWithoutWebPush(t *testing.T) {
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
	if err := database.RegisterClientNotifications(context.Background(), clientID, []byte("task-device-token"), store.APNSDevelopment, testTime); err != nil {
		t.Fatal(err)
	}
	service, err := New(paths, database, "http://localhost:3737")
	if err != nil || service.Available() {
		t.Fatalf("HTTP service = %v, %v", service.Available(), err)
	}
	taskID := "task:0123456789abcdef0123456789abcdef"
	eventKey := "task-alert:0123456789abcdef0123456789abcdef"
	if err := service.QueueTaskAttention(context.Background(), eventKey, strings.Repeat("Title ", 200), strings.Repeat("Body ", 300), taskID, "/tasks/one"); err != nil {
		t.Fatal(err)
	}
	delivery, err := database.ClaimDueAPNSDelivery(context.Background(), time.Now().Add(time.Minute))
	if err != nil || delivery == nil {
		t.Fatalf("native Task alert = %#v, %v", delivery, err)
	}
	if delivery.Notification.EventKey != eventKey || delivery.Notification.Route != "task" ||
		delivery.Notification.TaskID == nil || *delivery.Notification.TaskID != taskID ||
		delivery.Notification.Urgency != "high" || delivery.Notification.TTLSeconds != 86400 ||
		len(delivery.Notification.Title) > 600 || len(delivery.Notification.Body) > 600 {
		t.Fatalf("native Task alert = %#v", delivery.Notification)
	}
}

func TestTaskWorkEventsReconcileDurableAttention(t *testing.T) {
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
	if err := database.RegisterClientNotifications(context.Background(), clientID, []byte("task-event-device"), store.APNSDevelopment, testTime); err != nil {
		t.Fatal(err)
	}
	service, err := New(paths, database, "http://localhost:3737")
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	accountID := "provider_account:codex:default"
	if _, err := database.CreateProviderAccount(ctx, provider.Account{
		ID: accountID, ProviderKind: "codex", AccountKey: "default", DisplayName: "Codex",
		AuthMethod: provider.AuthOAuthDeviceCode, IsActive: true, IsDefault: true,
		Status: provider.StatusAuthenticated, Metadata: provider.AccountMetadata{}, CreatedAt: testTime, UpdatedAt: testTime,
	}); err != nil {
		t.Fatal(err)
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{
			Role: role, ProviderKind: "codex", ProviderAccountID: accountID,
			SelectionMode: store.ModelSelectionNoemaRecommended,
		})
	}
	if created, err := database.ConfirmHostedModelAssignments(ctx, accountID, assignments); err != nil || !created {
		t.Fatalf("model assignments = %t, %v", created, err)
	}
	blocked := createNotificationTask(t, database, "Blocked Task", "blocked")
	_, run, found, err := database.ClaimTaskExecution(ctx, testTime)
	if err != nil || !found || run.TaskID != blocked.ID {
		t.Fatalf("blocked claim = %#v, %t, %v", run, found, err)
	}
	if err := database.StartTaskExecution(ctx, run.ID, run.Generation, testTime); err != nil {
		t.Fatal(err)
	}
	if err := database.BlockTaskExecution(ctx, run.ID, run.Generation, "clarification", "Choose a target.", "Two targets remain.", nil, testTime); err != nil {
		t.Fatal(err)
	}
	if err := service.reconcileTasks(ctx); err != nil {
		t.Fatal(err)
	}
	delivery, err := database.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || delivery == nil || !strings.HasPrefix(delivery.Notification.EventKey, "task-gate:") || delivery.Notification.TaskID == nil || *delivery.Notification.TaskID != blocked.ID {
		t.Fatalf("gate event delivery = %#v, %v", delivery, err)
	}
	if err := database.FinishAPNSDelivery(ctx, *delivery, store.APNSDelivered, "", "apns:gate", time.Now()); err != nil {
		t.Fatal(err)
	}

	completed := createNotificationTask(t, database, "Completed Task", "completed")
	_, planner, found, err := database.ClaimTaskExecution(ctx, testTime)
	if err != nil || !found || planner.TaskID != completed.ID {
		t.Fatalf("planner claim = %#v, %t, %v", planner, found, err)
	}
	if err := database.StartTaskExecution(ctx, planner.ID, planner.Generation, testTime); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskPlanning(ctx, planner.ID, planner.Generation, "simple", testTime); err != nil {
		t.Fatal(err)
	}
	_, executor, found, err := database.ClaimTaskExecution(ctx, testTime)
	if err != nil || !found || executor.Kind != "executor" {
		t.Fatalf("executor claim = %#v, %t, %v", executor, found, err)
	}
	if err := database.StartTaskExecution(ctx, executor.ID, executor.Generation, testTime); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskExecution(ctx, executor.ID, executor.Generation, false, testTime); err != nil {
		t.Fatal(err)
	}
	_, reviewer, found, err := database.ClaimTaskExecution(ctx, testTime)
	if err != nil || !found || reviewer.Kind != "reviewer" {
		t.Fatalf("reviewer claim = %#v, %t, %v", reviewer, found, err)
	}
	if err := database.StartTaskExecution(ctx, reviewer.ID, reviewer.Generation, testTime); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskReview(ctx, reviewer.ID, reviewer.Generation, "approve", "Complete.", true, testTime); err != nil {
		t.Fatal(err)
	}
	if err := service.reconcileTasks(ctx); err != nil {
		t.Fatal(err)
	}
	delivery, err = database.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute))
	wantKey := "task-complete:" + completed.ID + ":1"
	if err != nil || delivery == nil || delivery.Notification.EventKey != wantKey || delivery.Notification.TaskID == nil || *delivery.Notification.TaskID != completed.ID {
		t.Fatalf("completion event delivery = %#v, %v", delivery, err)
	}
	if err := database.FinishAPNSDelivery(ctx, *delivery, store.APNSDelivered, "", "apns:complete", time.Now()); err != nil {
		t.Fatal(err)
	}
	restarted, err := New(paths, database, "http://localhost:3737")
	if err != nil {
		t.Fatal(err)
	}
	if err := restarted.reconcileTasks(ctx); err != nil {
		t.Fatal(err)
	}
	if duplicate, err := database.ClaimDueAPNSDelivery(ctx, time.Now().Add(time.Minute)); err != nil || duplicate != nil {
		t.Fatalf("replayed event delivery = %#v, %v", duplicate, err)
	}
}

func createNotificationTask(t *testing.T, database *store.Store, title, key string) store.Task {
	t.Helper()
	id, err := store.NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(context.Background(), id, title, "correlation:notification:"+key, testTime)
	if err != nil {
		t.Fatal(err)
	}
	sum := sha256.Sum256([]byte("queue_task\x00" + key))
	queued, err := database.QueueTask(context.Background(), task.ID, task.Revision, task.Generation, store.TaskCommand{
		Name: "queue_task", ClientMutationID: key, RequestDigest: hex.EncodeToString(sum[:]), CorrelationID: "correlation:notification:queue:" + key,
	}, testTime)
	if err != nil {
		t.Fatal(err)
	}
	return queued.Task
}

var testTime = time.Date(2026, 9, 5, 12, 0, 0, 0, time.UTC)

func testAPNSKey(t *testing.T) string {
	t.Helper()
	private, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := x509.MarshalPKCS8PrivateKey(private)
	if err != nil {
		t.Fatal(err)
	}
	return string(pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: encoded}))
}

func seedAPNSClient(t *testing.T, database *store.Store) string {
	t.Helper()
	clientID := "noema-ios:notification-test"
	code := sha256.Sum256([]byte("notification-code"))
	if err := database.InsertNativeOAuthCode(context.Background(), code, clientID, "Noema iOS",
		"noema://oauth/callback", "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
		testTime.Unix(), testTime.Add(time.Minute).Unix()); err != nil {
		t.Fatal(err)
	}
	if err := database.ExchangeNativeOAuthCode(context.Background(), code, clientID, "noema://oauth/callback",
		"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk", "a0000000000000000000000000000000",
		sha256.Sum256([]byte("notification-access")), sha256.Sum256([]byte("notification-refresh")), testTime.Unix()); err != nil {
		t.Fatal(err)
	}
	return clientID
}
