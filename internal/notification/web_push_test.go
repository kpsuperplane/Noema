package notification

import (
	"context"
	"crypto/ecdh"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestVAPIDConfigurationIsProtectedStableAndHTTPSBound(t *testing.T) {
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
	plain, err := New(paths, database, "http://localhost:3737")
	if err != nil || plain.Available() {
		t.Fatalf("HTTP availability = %v, %v", plain.Available(), err)
	}
	if _, err := os.Stat(paths.WebPushVAPID()); !os.IsNotExist(err) {
		t.Fatalf("HTTP origin created key: %v", err)
	}
	first, err := New(paths, database, "https://localhost:3737")
	if err != nil || !first.Available() || first.ApplicationServerKey() == "" {
		t.Fatalf("HTTPS service = %v, %q, %v", first.Available(), first.ApplicationServerKey(), err)
	}
	second, err := New(paths, database, "https://localhost:3737")
	if err != nil || second.ApplicationServerKey() != first.ApplicationServerKey() {
		t.Fatalf("reloaded public key = %q, %v", second.ApplicationServerKey(), err)
	}
	info, err := os.Stat(paths.WebPushVAPID())
	if err != nil {
		t.Fatal(err)
	}
	if runtime.GOOS != "windows" && info.Mode().Perm() != 0o600 {
		t.Fatalf("VAPID file mode = %o", info.Mode().Perm())
	}
}

func TestPrimaryFinalAnswerPresenceAndRegistration(t *testing.T) {
	ctx := context.Background()
	paths, err := home.FromRoot(filepath.Join(t.TempDir(), "home"))
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	database, err := store.Open(ctx, paths.Database())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	now := time.Now().UTC().Truncate(time.Second)
	session := addNotificationSession(t, database, now)
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	service, err := New(paths, database, "https://localhost:3737")
	if err != nil {
		t.Fatal(err)
	}
	p256dh, auth := notificationSubscriptionMaterial(t)
	endpoint := "https://push.example/secret-capability"
	registeredID, err := service.Register(ctx, session, endpoint, p256dh, auth)
	if err != nil {
		t.Fatal(err)
	}
	if id, err := service.Status(ctx, session, endpoint); err != nil || id != registeredID {
		t.Fatalf("registration status = %q, %v", id, err)
	}
	if _, err := service.Register(ctx, session, "https://127.0.0.1/push", p256dh, auth); err == nil {
		t.Fatal("private Push endpoint was accepted")
	}
	if err := service.reconcilePrimary(ctx); err != nil {
		t.Fatal(err)
	}
	presenceContext, cancelPresence := context.WithCancel(ctx)
	lease, err := service.Presence(presenceContext, session, registeredID)
	if err != nil {
		t.Fatal(err)
	}
	<-lease
	first, _, err := database.BeginConversationTurn(ctx, conversation.ID, "First", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CompleteConversationTurn(ctx, first, "Hidden while focused", "", nil, now); err != nil {
		t.Fatal(err)
	}
	if err := service.reconcilePrimary(ctx); err != nil {
		t.Fatal(err)
	}
	if claimed, err := database.ClaimDueWebPushDelivery(ctx, time.Now().Add(time.Minute)); err != nil || claimed != nil {
		t.Fatalf("focused delivery = %#v, %v", claimed, err)
	}
	cancelPresence()
	<-lease
	second, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Second", nil, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CompleteConversationTurn(ctx, second,
		"**A final answer** with `useful` detail", "", nil, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := service.reconcilePrimary(ctx); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimDueWebPushDelivery(ctx, time.Now().Add(time.Minute))
	if err != nil || claimed == nil {
		t.Fatalf("final-answer delivery = %#v, %v", claimed, err)
	}
	if claimed.Notification.EventKey != "chat-turn:"+second.ID ||
		claimed.Notification.Body != "A final answer with useful detail" ||
		claimed.Notification.Urgency != "normal" || claimed.Notification.TTLSeconds != 3600 {
		t.Fatalf("final-answer notification = %#v", claimed.Notification)
	}
}

func notificationSubscriptionMaterial(t *testing.T) (string, string) {
	t.Helper()
	private, err := ecdh.P256().GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	auth := sha256.Sum256([]byte("notification-auth"))
	return base64.RawURLEncoding.EncodeToString(private.PublicKey().Bytes()),
		base64.RawURLEncoding.EncodeToString(auth[:16])
}

func addNotificationSession(t *testing.T, database *store.Store, now time.Time) [32]byte {
	t.Helper()
	old := sha256.Sum256([]byte("push-anonymous"))
	session := sha256.Sum256([]byte("push-authenticated"))
	if err := database.CreateAnonymousSession(context.Background(), old, now); err != nil {
		t.Fatal(err)
	}
	credential := store.HumanPasskey{
		CredentialID: base64.RawURLEncoding.EncodeToString([]byte("push-passkey")), CredentialJSON: `{}`,
	}
	if err := database.RegisterPasskey(context.Background(), credential, store.RegistrationInitial,
		old, session, now); err != nil {
		t.Fatal(err)
	}
	return session
}
