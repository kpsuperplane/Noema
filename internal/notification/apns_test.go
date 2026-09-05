package notification

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/sha256"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
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
