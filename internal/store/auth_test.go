package store

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"errors"
	"fmt"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

func TestInitialPasskeyClaimIsAtomic(t *testing.T) {
	taskStore := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	type attempt struct {
		old        [32]byte
		new        [32]byte
		credential HumanPasskey
	}
	attempts := []attempt{
		{testDigest("old-one"), testDigest("new-one"), testCredential(1)},
		{testDigest("old-two"), testDigest("new-two"), testCredential(2)},
	}
	for _, attempt := range attempts {
		if err := taskStore.CreateAnonymousSession(ctx, attempt.old, now); err != nil {
			t.Fatal(err)
		}
	}
	start := make(chan struct{})
	results := make(chan error, len(attempts))
	var group sync.WaitGroup
	for _, attempt := range attempts {
		attempt := attempt
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			results <- taskStore.RegisterPasskey(
				ctx,
				attempt.credential,
				RegistrationInitial,
				attempt.old,
				attempt.new,
				now,
			)
		}()
	}
	close(start)
	group.Wait()
	close(results)
	var succeeded, conflicted int
	for err := range results {
		switch {
		case err == nil:
			succeeded++
		case errors.Is(err, ErrInitialPasskeyClaimed):
			conflicted++
		default:
			t.Fatalf("unexpected claim result: %v", err)
		}
	}
	if succeeded != 1 || conflicted != 1 {
		t.Fatalf("claim results = %d success, %d conflict", succeeded, conflicted)
	}
	passkeys, err := taskStore.Passkeys(ctx)
	if err != nil || len(passkeys) != 1 {
		t.Fatalf("stored passkeys = %#v, %v", passkeys, err)
	}
}

func TestBrowserSessionsPersistExpireAndStayBounded(t *testing.T) {
	ctx := context.Background()
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	taskStore, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	digest := testDigest("persistent")
	if err := taskStore.CreateAnonymousSession(ctx, digest, now); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.Close(); err != nil {
		t.Fatal(err)
	}
	taskStore, err = Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = taskStore.Close() })
	if session, exists, err := taskStore.BrowserSession(ctx, digest, now.Add(time.Minute), false); err != nil ||
		!exists || session.State != "anonymous" {
		t.Fatalf("reopened session = %#v, %v, %v", session, exists, err)
	}
	if _, exists, err := taskStore.BrowserSession(ctx, digest, now.Add(6*time.Minute), false); err != nil || exists {
		t.Fatalf("expired session remains: %v, %v", exists, err)
	}

	tx, err := taskStore.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	for index := range browserSessionCapacity {
		value := testDigest(fmt.Sprintf("capacity-%d", index))
		if _, err := tx.ExecContext(ctx, `
INSERT INTO browser_sessions (session_hash, state, created_at_ms, expires_at_ms)
VALUES (?, 'anonymous', ?, ?)`, value[:], millis(now), millis(now.Add(time.Minute))); err != nil {
			t.Fatal(err)
		}
	}
	if err := tx.Commit(); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.CreateAnonymousSession(ctx, testDigest("overflow"), now); !errors.Is(err, ErrSessionFull) {
		t.Fatalf("capacity error = %v, want %v", err, ErrSessionFull)
	}
}

func TestPasskeyChangesRequireCurrentCredential(t *testing.T) {
	taskStore := openTestStore(t)
	ctx := context.Background()
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	firstOld := testDigest("first-old")
	firstSession := testDigest("first-session")
	if err := taskStore.CreateAnonymousSession(ctx, firstOld, now); err != nil {
		t.Fatal(err)
	}
	first := testCredential(1)
	if err := taskStore.RegisterPasskey(
		ctx, first, RegistrationInitial, firstOld, firstSession, now,
	); err != nil {
		t.Fatal(err)
	}
	secondSession := testDigest("second-session")
	second := testCredential(2)
	if err := taskStore.RegisterPasskey(
		ctx,
		second,
		RegistrationCurrent,
		firstSession,
		secondSession,
		now.Add(time.Minute),
	); err != nil {
		t.Fatal(err)
	}
	if _, err := taskStore.RemovePasskey(
		ctx,
		first.CredentialID,
		secondSession,
		false,
		now.Add(7*time.Minute),
	); !errors.Is(err, ErrSessionUnauthorized) {
		t.Fatalf("stale recent-passkey removal = %v", err)
	}
	revoked, err := taskStore.RemovePasskey(
		ctx,
		first.CredentialID,
		secondSession,
		false,
		now.Add(2*time.Minute),
	)
	if err != nil || len(revoked) != 0 {
		t.Fatalf("passkey removal = %#v, %v", revoked, err)
	}
	if _, err := taskStore.RemovePasskey(
		ctx,
		second.CredentialID,
		secondSession,
		false,
		now.Add(2*time.Minute),
	); !errors.Is(err, ErrFinalPasskey) {
		t.Fatalf("final-passkey removal = %v", err)
	}

	loginOld := testDigest("login-old")
	staleLoginOld := testDigest("stale-login-old")
	loginNew := testDigest("login-new")
	if err := taskStore.CreateAnonymousSession(ctx, loginOld, now.Add(2*time.Minute)); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.CreateAnonymousSession(ctx, staleLoginOld, now.Add(2*time.Minute)); err != nil {
		t.Fatal(err)
	}
	replacement := `{"counter":1}`
	if err := taskStore.AuthenticatePasskey(
		ctx,
		second.CredentialID,
		second.CredentialJSON,
		replacement,
		loginOld,
		loginNew,
		now.Add(2*time.Minute),
	); err != nil {
		t.Fatal(err)
	}
	if err := taskStore.AuthenticatePasskey(
		ctx,
		second.CredentialID,
		second.CredentialJSON,
		`{"counter":2}`,
		staleLoginOld,
		testDigest("late-login"),
		now.Add(2*time.Minute),
	); !errors.Is(err, ErrCredentialChanged) {
		t.Fatalf("stale credential update = %v", err)
	}
}

func testDigest(value string) [32]byte {
	return sha256.Sum256([]byte(value))
}

func testCredential(value byte) HumanPasskey {
	return HumanPasskey{
		CredentialID:   base64.RawURLEncoding.EncodeToString([]byte{value}),
		CredentialJSON: fmt.Sprintf(`{"credential":%d}`, value),
	}
}
