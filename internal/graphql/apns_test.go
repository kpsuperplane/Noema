package graphql

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/notification"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestNativeNotificationGraphQLPreservesClientContractAndAuthority(t *testing.T) {
	ctx := context.Background()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	taskStore, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	now := time.Now()
	oldSession, newSession := sha256.Sum256([]byte("notification-old")), sha256.Sum256([]byte("notification-new"))
	if err := taskStore.CreateAnonymousSession(ctx, oldSession, now); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.RegisterPasskey(ctx, store.HumanPasskey{CredentialID: "notification-passkey", CredentialJSON: `{}`},
		store.RegistrationInitial, oldSession, newSession, now); err != nil {
		t.Fatal(err)
	}
	config, recovery, err := auth.LoadConfig(paths, "127.0.0.1:3737")
	if err != nil {
		t.Fatal(err)
	}
	authentication, err := auth.New(paths, taskStore, config, recovery)
	if err != nil {
		t.Fatal(err)
	}
	service, err := notification.New(paths, taskStore, config.Origin)
	if err != nil {
		t.Fatal(err)
	}
	handler := authentication.Handler(NewHandler(NewResolver(taskStore, nil, authentication,
		nil, nil, nil, nil, nil, nil, service)))
	access := seedNotificationNativeClient(t, taskStore)
	device := base64.RawURLEncoding.EncodeToString([]byte("device-token"))
	register := nativeGraphQLRequest(t, access, `mutation { registerClientNotifications(input: {
deviceToken: "`+device+`", environment: DEVELOPMENT}) { available enabled environment blocker } }`)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, register)
	if response.Code != http.StatusOK || !strings.Contains(response.Body.String(), `"enabled":true`) ||
		!strings.Contains(response.Body.String(), `"environment":"DEVELOPMENT"`) {
		t.Fatalf("notification registration = %d %s", response.Code, response.Body.String())
	}
	activity := base64.RawURLEncoding.EncodeToString([]byte("push-to-start"))
	live := nativeGraphQLRequest(t, access, `mutation { registerClientLiveActivities(input: {
pushToStartToken: "`+activity+`", environment: PRODUCTION, activeActivityIds: ["live_activity:one"]}) {
available enabled registered environment } }`)
	response = httptest.NewRecorder()
	handler.ServeHTTP(response, live)
	if response.Code != http.StatusOK || !strings.Contains(response.Body.String(), `"registered":true`) ||
		!strings.Contains(response.Body.String(), `"environment":"PRODUCTION"`) {
		t.Fatalf("Live Activity registration = %d %s", response.Code, response.Body.String())
	}
	nativeProviderWrite := nativeGraphQLRequest(t, access, `mutation { removeApnsProvider(expectedRevision: 0) { revision } }`)
	response = httptest.NewRecorder()
	handler.ServeHTTP(response, nativeProviderWrite)
	if response.Code != http.StatusOK || !bytes.Contains(response.Body.Bytes(), []byte("browser session authentication required")) {
		t.Fatalf("native provider write = %d %s", response.Code, response.Body.String())
	}
}

func seedNotificationNativeClient(t *testing.T, database *store.Store) string {
	t.Helper()
	now := time.Now()
	clientID := "noema-ios:abcdefghijklmnop"
	code, access := "notification-code", "notification-access"
	if err := database.InsertNativeOAuthCode(context.Background(), sha256.Sum256([]byte(code)), clientID,
		"Noema iOS", "noema://oauth/callback", "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
		now.Unix(), now.Add(10*time.Minute).Unix()); err != nil {
		t.Fatal(err)
	}
	if err := database.ExchangeNativeOAuthCode(context.Background(), sha256.Sum256([]byte(code)), clientID,
		"noema://oauth/callback", "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
		"90000000000000000000000000000000", sha256.Sum256([]byte(access)),
		sha256.Sum256([]byte("notification-refresh")), now.Unix()); err != nil {
		t.Fatal(err)
	}
	return access
}
