package notification

import (
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
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
