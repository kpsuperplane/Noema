package store

import (
	"context"
	"crypto/sha256"
	"errors"
	"sync"
	"testing"
)

const (
	testNativeClient    = "noema-desktop:abcdefghijklmnop"
	testNativeRedirect  = "http://127.0.0.1:49152/oauth/callback"
	testNativeVerifier  = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
	testNativeChallenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
)

func TestNativeOAuthCodeExchangeIsDigestOnlyAndAtomic(t *testing.T) {
	taskStore := openTestStore(t)
	ctx := context.Background()
	now := int64(1_788_523_200)
	rawCode := "authorization-code"
	hash := sha256.Sum256([]byte(rawCode))
	if err := taskStore.InsertNativeOAuthCode(
		ctx, hash, testNativeClient, "Noema Desktop", testNativeRedirect,
		testNativeChallenge, now, now+600,
	); err != nil {
		t.Fatal(err)
	}
	var stored []byte
	if err := taskStore.db.QueryRow("SELECT code_hash FROM native_oauth_codes").Scan(&stored); err != nil {
		t.Fatal(err)
	}
	if string(stored) == rawCode || string(stored) != string(hash[:]) {
		t.Fatal("authorization code storage is not digest-only")
	}
	if err := taskStore.ExchangeNativeOAuthCode(
		ctx, hash, testNativeClient, testNativeRedirect, "F"+testNativeVerifier[1:],
		"00000000000000000000000000000000", testDigest("access"), testDigest("refresh"), now,
	); !errors.Is(err, ErrOAuthGrant) {
		t.Fatalf("bad verifier exchange = %v", err)
	}
	if err := taskStore.ExchangeNativeOAuthCode(
		ctx, hash, testNativeClient, testNativeRedirect, testNativeVerifier,
		"00000000000000000000000000000000", testDigest("access"), testDigest("refresh"), now,
	); !errors.Is(err, ErrOAuthGrant) {
		t.Fatalf("burned code exchange = %v", err)
	}

	secondCode := sha256.Sum256([]byte("second-code"))
	if err := taskStore.InsertNativeOAuthCode(
		ctx, secondCode, testNativeClient, "Noema Desktop", testNativeRedirect,
		testNativeChallenge, now, now+600,
	); err != nil {
		t.Fatal(err)
	}
	start := make(chan struct{})
	results := make(chan error, 2)
	var group sync.WaitGroup
	for index := range 2 {
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			family := "0000000000000000000000000000000" + string(rune('1'+index))
			results <- taskStore.ExchangeNativeOAuthCode(
				ctx, secondCode, testNativeClient, testNativeRedirect, testNativeVerifier,
				family, testDigest("access"+family), testDigest("refresh"+family), now,
			)
		}()
	}
	close(start)
	group.Wait()
	close(results)
	var successes, rejected int
	for err := range results {
		if err == nil {
			successes++
		} else if errors.Is(err, ErrOAuthGrant) {
			rejected++
		} else {
			t.Fatal(err)
		}
	}
	if successes != 1 || rejected != 1 {
		t.Fatalf("concurrent exchanges = %d success, %d rejected", successes, rejected)
	}
}

func TestNativeOAuthRefreshProofAndReplayRevokeTheFamily(t *testing.T) {
	taskStore := openTestStore(t)
	ctx := context.Background()
	now := int64(1_788_523_200)
	oldAccess, oldRefresh := seedNativeFamily(t, taskStore, testNativeClient, "1", now)
	lookup, err := taskStore.NativeOAuthRefreshGrant(ctx, oldRefresh, nil, now+1)
	if err != nil || lookup.State != "active" || lookup.Sequence != 0 {
		t.Fatalf("active refresh = %#v, %v", lookup, err)
	}
	newAccess, newRefresh := testDigest("new-access"), testDigest("new-refresh")
	rotation := NativeOAuthRotation{
		AccessHash: newAccess, RefreshHash: newRefresh, IssuedAt: now + 1,
		AccessExpires: now + 901, IdleExpiresAt: now + 30*24*60*60,
	}
	if result, err := taskStore.RotateNativeOAuthRefresh(ctx, oldRefresh, rotation, now+1); err != nil || result.State != "rotated" {
		t.Fatalf("rotation = %#v, %v", result, err)
	}
	proof := &NativeOAuthRetryProof{
		AccessHash: newAccess, RefreshHash: newRefresh, IssuedAt: now + 1, RequestBound: true,
	}
	if result, err := taskStore.NativeOAuthRefreshGrant(ctx, oldRefresh, proof, now+902); err != nil || result.State != "retryable" {
		t.Fatalf("request-bound retry = %#v, %v", result, err)
	}
	result, err := taskStore.NativeOAuthRefreshGrant(ctx, oldRefresh, nil, now+2)
	if err != nil || result.State != "replay" || result.RevokedClientID != testNativeClient {
		t.Fatalf("replay = %#v, %v", result, err)
	}
	if _, exists, err := taskStore.ActiveNativeOAuthAccess(ctx, oldAccess, now+2); err != nil || exists {
		t.Fatalf("revoked old access = %v, %v", exists, err)
	}
	if _, exists, err := taskStore.ActiveNativeOAuthAccess(ctx, newAccess, now+2); err != nil || exists {
		t.Fatalf("revoked successor access = %v, %v", exists, err)
	}

	_, legacyOld := seedNativeFamily(t, taskStore, testNativeClient, "5", now+10)
	legacyAccess, legacyRefresh := testDigest("legacy-access"), testDigest("legacy-refresh")
	legacyRotation := NativeOAuthRotation{
		AccessHash: legacyAccess, RefreshHash: legacyRefresh, IssuedAt: now + 11,
		AccessExpires: now + 911, IdleExpiresAt: now + 30*24*60*60,
	}
	if result, err := taskStore.RotateNativeOAuthRefresh(ctx, legacyOld, legacyRotation, now+11); err != nil || result.State != "rotated" {
		t.Fatalf("legacy rotation = %#v, %v", result, err)
	}
	legacyProof := &NativeOAuthRetryProof{
		AccessHash: legacyAccess, RefreshHash: legacyRefresh, IssuedAt: now + 11,
	}
	if result, err := taskStore.NativeOAuthRefreshGrant(ctx, legacyOld, legacyProof, now+71); err != nil || result.State != "retryable" {
		t.Fatalf("60-second desktop retry = %#v, %v", result, err)
	}
	if result, err := taskStore.NativeOAuthRefreshGrant(ctx, legacyOld, legacyProof, now+72); err != nil || result.State != "replay" {
		t.Fatalf("expired desktop retry = %#v, %v", result, err)
	}
}

func TestNativeOAuthClientsRetainRowsAcrossRevocationAndExpiry(t *testing.T) {
	taskStore := openTestStore(t)
	ctx := context.Background()
	now := int64(1_788_523_200)
	firstAccess, _ := seedNativeFamily(t, taskStore, testNativeClient, "2", now)
	secondClient := "noema-ios:abcdefghijklmnop"
	_, _ = seedNativeFamily(t, taskStore, secondClient, "3", now+1)
	client, changed, err := taskStore.RevokeNativeOAuthClient(ctx, testNativeClient, now+2)
	if err != nil || !changed || client.RevokedAt == nil {
		t.Fatalf("client revocation = %#v, %v, %v", client, changed, err)
	}
	if _, exists, err := taskStore.ActiveNativeOAuthAccess(ctx, firstAccess, now+2); err != nil || exists {
		t.Fatalf("revoked client access = %v, %v", exists, err)
	}
	revoked, err := taskStore.RevokeAllNativeOAuthClients(ctx, now+3)
	if err != nil || len(revoked) != 1 || revoked[0] != secondClient {
		t.Fatalf("global revocation = %#v, %v", revoked, err)
	}
	expiringClient := "noema-desktop:qrstuvwxyzABCDEF"
	_, _ = seedNativeFamily(t, taskStore, expiringClient, "4", now+4)
	expired, err := taskStore.ExpireNativeOAuthFamilies(ctx, now+4+30*24*60*60)
	if err != nil || len(expired) != 1 || expired[0] != expiringClient {
		t.Fatalf("family expiry = %#v, %v", expired, err)
	}
	clients, err := taskStore.NativeOAuthClients(ctx)
	if err != nil || len(clients) != 3 || clients[0].RevokedAt == nil || clients[1].RevokedAt == nil {
		t.Fatalf("retained clients = %#v, %v", clients, err)
	}
}

func seedNativeFamily(
	t *testing.T,
	taskStore *Store,
	clientID string,
	suffix string,
	now int64,
) ([32]byte, [32]byte) {
	t.Helper()
	code := sha256.Sum256([]byte("code-" + suffix))
	if err := taskStore.InsertNativeOAuthCode(
		context.Background(), code, clientID, "Native client", nativeRedirect(clientID),
		testNativeChallenge, now, now+600,
	); err != nil {
		t.Fatal(err)
	}
	access, refresh := testDigest("access-"+suffix), testDigest("refresh-"+suffix)
	family := suffix + "0000000000000000000000000000000"
	if err := taskStore.ExchangeNativeOAuthCode(
		context.Background(), code, clientID, nativeRedirect(clientID), testNativeVerifier,
		family, access, refresh, now,
	); err != nil {
		t.Fatal(err)
	}
	return access, refresh
}

func nativeRedirect(clientID string) string {
	if clientID == testNativeClient {
		return testNativeRedirect
	}
	return "noema://oauth/callback"
}
