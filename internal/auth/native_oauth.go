package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"errors"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/web"
)

const (
	nativeOAuthLimit = 8 * 1024
	accessLifetime   = 15 * time.Minute
	codeLifetime     = 10 * time.Minute
)

type nativeOAuth struct {
	store    *store.Store
	retries  nativeRetryStore
	protocol nativeOAuthProtocol

	refreshMu sync.Mutex
	mu        sync.Mutex
	nextID    atomic.Uint64
	sockets   map[string]map[uint64]context.CancelFunc
}

type nativePrincipal struct {
	clientID   string
	expiresAt  time.Time
	accessHash [32]byte
}

type nativePrincipalKey struct{}

type authorizationRequest struct {
	clientID  string
	redirect  string
	state     string
	challenge string
	ios       bool
}

func newNativeOAuth(paths home.Paths, taskStore *store.Store) *nativeOAuth {
	return &nativeOAuth{
		store:    taskStore,
		retries:  nativeRetryStore{path: paths.NativeOAuthRetries()},
		protocol: newNativeOAuthProtocol(),
		sockets:  make(map[string]map[uint64]context.CancelFunc),
	}
}

// ClientID returns the native client that owns the request.
func ClientID(ctx context.Context) string {
	principal, _ := ctx.Value(nativePrincipalKey{}).(nativePrincipal)
	return principal.clientID
}

func (n *nativeOAuth) authenticate(r *http.Request) (*http.Request, *nativePrincipal, error) {
	raw := r.Header.Get("Authorization")
	if raw == "" {
		return r, nil, nil
	}
	token, ok := strings.CutPrefix(raw, "Bearer ")
	if !ok || token == "" || len(token) > 128 || strings.Contains(token, ".") {
		return r, nil, store.ErrOAuthGrant
	}
	hash := sha256.Sum256([]byte(token))
	access, exists, err := n.store.ActiveNativeOAuthAccess(r.Context(), hash, time.Now().Unix())
	if err != nil {
		return r, nil, err
	}
	if !exists {
		return r, nil, store.ErrOAuthGrant
	}
	principal := &nativePrincipal{clientID: access.ClientID, expiresAt: access.ExpiresAt, accessHash: hash}
	ctx := context.WithValue(r.Context(), nativePrincipalKey{}, *principal)
	return r.WithContext(ctx), principal, nil
}

func (n *nativeOAuth) serve(w http.ResponseWriter, r *http.Request, sessions *sessionSecurity, devNoAuth bool) {
	switch {
	case r.Method == http.MethodGet && r.URL.Path == "/oauth/authorize":
		n.authorize(w, r, sessions, devNoAuth)
	case r.Method == http.MethodPost && r.URL.Path == "/oauth/authorize":
		n.approve(w, r, sessions, devNoAuth)
	case r.Method == http.MethodPost && r.URL.Path == "/oauth/token":
		n.token(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/oauth/revoke":
		n.revoke(w, r)
	default:
		web.WriteNotFound(w, r)
	}
}

func (n *nativeOAuth) authorize(w http.ResponseWriter, r *http.Request, sessions *sessionSecurity, devNoAuth bool) {
	query := r.URL.RawQuery
	browser, exists, err := sessions.current(r, false)
	if err != nil {
		writeAuthorizationError(w, http.StatusInternalServerError, "service")
		return
	}
	if query == "" {
		if !exists {
			writeAuthorizationError(w, http.StatusBadRequest, "invalid")
			return
		}
		var csrf *string
		var found bool
		query, csrf, found, err = n.store.NativeOAuthBrowserRequest(r.Context(), browser.digest)
		if err != nil {
			writeAuthorizationError(w, http.StatusInternalServerError, "service")
			return
		}
		if !found || csrf != nil {
			writeAuthorizationError(w, http.StatusBadRequest, "invalid")
			return
		}
	}
	request, err := n.protocol.authorization(query)
	if err != nil {
		writeAuthorizationError(w, http.StatusBadRequest, "invalid")
		return
	}
	if !devNoAuth && (!exists || !recentPasskey(browser.record, time.Now())) {
		browser, err = sessions.ensureAnonymous(w, r)
		if err != nil || n.store.SaveNativeOAuthBrowserRequest(r.Context(), browser.digest, query, nil) != nil {
			writeAuthorizationError(w, http.StatusInternalServerError, "service")
			return
		}
		http.Redirect(w, r, "/?native_authorization=resume", http.StatusSeeOther)
		return
	}
	if !exists {
		browser, err = sessions.ensureAnonymous(w, r)
		if err != nil {
			writeAuthorizationError(w, http.StatusInternalServerError, "service")
			return
		}
	}
	csrf, err := randomBase64URL(32)
	if err != nil || n.store.SaveNativeOAuthBrowserRequest(r.Context(), browser.digest, query, &csrf) != nil {
		writeAuthorizationError(w, http.StatusInternalServerError, "service")
		return
	}
	writeConsentPage(w, request, csrf)
}

func (n *nativeOAuth) approve(w http.ResponseWriter, r *http.Request, sessions *sessionSecurity, devNoAuth bool) {
	if !formContentType(r.Header.Get("Content-Type")) {
		writeAuthorizationError(w, http.StatusForbidden, "invalid")
		return
	}
	browser, exists, err := sessions.current(r, false)
	if err != nil || !devNoAuth && (!exists || !recentPasskey(browser.record, time.Now())) {
		writeAuthorizationError(w, http.StatusForbidden, "expired")
		return
	}
	parameters, err := readUniqueForm(w, r, 64*1024)
	if err != nil {
		writeAuthorizationError(w, http.StatusBadRequest, "invalid")
		return
	}
	query, csrf, found, err := n.store.ConsumeNativeOAuthBrowserRequest(r.Context(), browser.digest)
	if err != nil {
		writeAuthorizationError(w, http.StatusInternalServerError, "service")
		return
	}
	if !found {
		writeAuthorizationError(w, http.StatusBadRequest, "expired")
		return
	}
	provided, hasCSRF := parameters["csrf"]
	if !hasCSRF || !secretTextEqual(provided, csrf) {
		writeAuthorizationError(w, http.StatusBadRequest, "unverified")
		return
	}
	request, err := n.protocol.authorization(query)
	if err != nil {
		writeAuthorizationError(w, http.StatusBadRequest, "invalid")
		return
	}
	switch parameters["decision"] {
	case "deny":
		n.protocol.redirect(w, request, "", "access_denied")
	case "approve":
		code, err := randomBase64URL(32)
		if err != nil {
			writeAuthorizationError(w, http.StatusInternalServerError, "service")
			return
		}
		hash := sha256.Sum256([]byte(code))
		now := time.Now().Unix()
		if err := n.store.InsertNativeOAuthCode(
			r.Context(), hash, request.clientID, displayName(request.clientID),
			request.redirect, request.challenge, now, now+int64(codeLifetime/time.Second),
		); err != nil {
			writeAuthorizationError(w, http.StatusInternalServerError, "service")
			return
		}
		n.protocol.redirect(w, request, code, "")
	default:
		writeAuthorizationError(w, http.StatusBadRequest, "invalid")
	}
}

func (n *nativeOAuth) token(w http.ResponseWriter, r *http.Request) {
	if !formContentType(r.Header.Get("Content-Type")) {
		n.protocol.writeError(w, errInvalidOAuthRequest)
		return
	}
	parameters, err := readUniqueForm(w, r, nativeOAuthLimit)
	if err != nil {
		n.protocol.writeError(w, errInvalidOAuthRequest)
		return
	}
	grant, request, err := n.protocol.token(r, parameters)
	if err != nil {
		n.protocol.writeError(w, err)
		return
	}
	switch grant {
	case oauthGrantAuthorizationCode:
		n.exchangeCode(w, r, request)
	case oauthGrantRefreshToken:
		n.refresh(w, r, request)
	default:
		n.protocol.writeError(w, errUnsupportedGrant)
	}
}

func (n *nativeOAuth) exchangeCode(w http.ResponseWriter, r *http.Request, request nativeTokenRequest) {
	code, clientID := request.code, request.clientID
	redirect, verifier := request.redirect, request.verifier
	if code == "" || len(code) > 128 || !validClientAndRedirect(clientID, redirect) || !validVerifier(verifier) {
		n.protocol.writeError(w, errInvalidOAuthRequest)
		return
	}
	access, refresh, family, err := nativeCredentialPair()
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	now := time.Now().Unix()
	err = n.store.ExchangeNativeOAuthCode(
		r.Context(), sha256.Sum256([]byte(code)), clientID, redirect, verifier, family,
		sha256.Sum256([]byte(access)), sha256.Sum256([]byte(refresh)), now,
	)
	if errors.Is(err, store.ErrOAuthGrant) {
		n.protocol.writeError(w, errInvalidOAuthGrant)
		return
	}
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	n.protocol.writeToken(w, nativeTokenResponse{
		AccessToken: access, RefreshToken: refresh, TokenType: "bearer",
		ExpiresIn: int64(accessLifetime / time.Second), Scope: "noema",
	})
}

func (n *nativeOAuth) refresh(w http.ResponseWriter, r *http.Request, request nativeTokenRequest) {
	raw := request.refresh
	requestID := request.requestID
	if raw == "" || len(raw) > 128 || requestID != "" && !validRequestID(requestID) {
		n.protocol.writeError(w, errInvalidOAuthRequest)
		return
	}
	n.refreshMu.Lock()
	defer n.refreshMu.Unlock()
	now := time.Now().Unix()
	hash := sha256.Sum256([]byte(raw))
	cached, err := n.retries.load(hash, requestID, now)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	var proof *store.NativeOAuthRetryProof
	if cached != nil {
		value := cached.proof(requestID != "")
		proof = &value
	}
	grant, err := n.store.NativeOAuthRefreshGrant(r.Context(), hash, proof, now)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	if grant.RevokedClientID != "" {
		n.revokeConnections(grant.RevokedClientID)
		if err := n.retries.remove(grant.FamilyID, "", false); err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			return
		}
	}
	if grant.State == "retryable" && cached != nil {
		n.protocol.writeToken(w, cached.response(grant.AccessExpiresAt, now))
		return
	}
	if grant.State != "active" {
		n.protocol.writeError(w, errInvalidOAuthGrant)
		return
	}
	access, refresh, _, err := nativeCredentialPair()
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	entry := nativeRetryEntry{
		IssuedAt: now, AccessToken: access, RefreshToken: refresh,
		FamilyID: grant.FamilyID, ClientID: grant.ClientID,
	}
	if requestID != "" {
		entry.RetainUntil = now + 30*24*60*60
		if entry.RetainUntil > grant.AbsoluteExpiresAt {
			entry.RetainUntil = grant.AbsoluteExpiresAt
		}
	}
	if err := n.retries.save(hash, requestID, entry, now); err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	rotation := store.NativeOAuthRotation{
		AccessHash: sha256.Sum256([]byte(access)), RefreshHash: sha256.Sum256([]byte(refresh)),
		IssuedAt: now, AccessExpires: now + int64(accessLifetime/time.Second),
		IdleExpiresAt: now + 30*24*60*60,
	}
	outcome, err := n.store.RotateNativeOAuthRefresh(r.Context(), hash, rotation, now)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		return
	}
	if outcome.RevokedClientID != "" {
		n.revokeConnections(outcome.RevokedClientID)
		if err := n.retries.remove(outcome.FamilyID, "", false); err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			return
		}
	}
	if outcome.State != "rotated" {
		n.protocol.writeError(w, errInvalidOAuthGrant)
		return
	}
	n.protocol.writeToken(w, entry.response(rotation.AccessExpires, now))
}

func (n *nativeOAuth) revoke(w http.ResponseWriter, r *http.Request) {
	if !formContentType(r.Header.Get("Content-Type")) {
		n.protocol.writeError(w, errInvalidOAuthRequest)
		return
	}
	parameters, err := readUniqueForm(w, r, nativeOAuthLimit)
	if err != nil {
		n.protocol.writeError(w, errInvalidOAuthRequest)
		return
	}
	if token := parameters["token"]; token != "" {
		clientID, familyID, err := n.store.RevokeNativeOAuthFamily(
			r.Context(), sha256.Sum256([]byte(token)), time.Now().Unix(),
		)
		if err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			return
		}
		if clientID != "" {
			n.revokeConnections(clientID)
			if err := n.retries.remove(familyID, "", false); err != nil {
				w.WriteHeader(http.StatusInternalServerError)
				return
			}
		}
	}
	w.WriteHeader(http.StatusOK)
}

func (n *nativeOAuth) registerConnection(clientID string, cancel context.CancelFunc) uint64 {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.sockets[clientID] == nil {
		n.sockets[clientID] = make(map[uint64]context.CancelFunc)
	}
	id := n.nextID.Add(1)
	n.sockets[clientID][id] = cancel
	return id
}

func (n *nativeOAuth) unregisterConnection(clientID string, id uint64) {
	n.mu.Lock()
	defer n.mu.Unlock()
	delete(n.sockets[clientID], id)
	if len(n.sockets[clientID]) == 0 {
		delete(n.sockets, clientID)
	}
}

func (n *nativeOAuth) revokeConnections(clientID string) {
	n.mu.Lock()
	connections := n.sockets[clientID]
	delete(n.sockets, clientID)
	n.mu.Unlock()
	for _, cancel := range connections {
		cancel()
	}
}

func (n *nativeOAuth) runCleanup(ctx context.Context) {
	ticker := time.NewTicker(time.Minute)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case now := <-ticker.C:
			clientIDs, err := n.store.ExpireNativeOAuthFamilies(ctx, now.Unix())
			if err == nil {
				for _, clientID := range clientIDs {
					n.revokeConnections(clientID)
				}
			}
			_ = n.retries.prune(now.Unix())
		}
	}
}

func validClientAndRedirect(clientID string, redirect string) bool {
	prefix := "noema-desktop:"
	ios := strings.HasPrefix(clientID, "noema-ios:")
	if ios {
		prefix = "noema-ios:"
	} else if !strings.HasPrefix(clientID, prefix) {
		return false
	}
	if !validURLText(strings.TrimPrefix(clientID, prefix), 16, 96) {
		return false
	}
	if ios {
		return redirect == "noema://oauth/callback"
	}
	parsed, err := url.Parse(redirect)
	return err == nil && parsed.Scheme == "http" && parsed.User == nil &&
		(parsed.Hostname() == "127.0.0.1" || parsed.Hostname() == "::1") &&
		parsed.Port() != "" && parsed.Path == "/oauth/callback" &&
		parsed.RawQuery == "" && !parsed.ForceQuery && !strings.Contains(redirect, "#")
}

func validVerifier(value string) bool {
	if len(value) < 43 || len(value) > 128 {
		return false
	}
	for _, char := range []byte(value) {
		if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' ||
			char >= '0' && char <= '9' || strings.ContainsRune("-._~", rune(char))) {
			return false
		}
	}
	return true
}

func validRequestID(value string) bool { return validURLText(value, 32, 128) }

func validURLText(value string, minimum int, maximum int) bool {
	if len(value) < minimum || len(value) > maximum {
		return false
	}
	for _, char := range []byte(value) {
		if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' ||
			char >= '0' && char <= '9' || char == '-' || char == '_') {
			return false
		}
	}
	return true
}

func recentPasskey(record store.BrowserSession, now time.Time) bool {
	return record.State == "authenticated" && !record.RecentPasskeyAt.IsZero() &&
		!record.RecentPasskeyAt.After(now) && now.Sub(record.RecentPasskeyAt) <= 5*time.Minute
}

func nativeCredentialPair() (string, string, string, error) {
	access, err := randomBase64URL(32)
	if err != nil {
		return "", "", "", err
	}
	refresh, err := randomBase64URL(32)
	if err != nil {
		return "", "", "", err
	}
	value := make([]byte, 16)
	if _, err := rand.Read(value); err != nil {
		return "", "", "", err
	}
	return access, refresh, hex.EncodeToString(value), nil
}

func secretTextEqual(left string, right string) bool {
	leftHash, rightHash := sha256.Sum256([]byte(left)), sha256.Sum256([]byte(right))
	return subtle.ConstantTimeCompare(leftHash[:], rightHash[:]) == 1
}

func displayName(clientID string) string {
	if strings.HasPrefix(clientID, "noema-ios:") {
		return "Noema iOS"
	}
	return "Noema Desktop"
}

func (n *nativeOAuth) clients(ctx context.Context) ([]store.NativeOAuthClient, error) {
	return n.store.NativeOAuthClients(ctx)
}

func (n *nativeOAuth) revokeClient(ctx context.Context, clientID string) (store.NativeOAuthClient, error) {
	client, changed, err := n.store.RevokeNativeOAuthClient(ctx, strings.TrimSpace(clientID), time.Now().Unix())
	if err == nil && changed {
		n.revokeConnections(client.ClientID)
		if cleanupErr := n.retries.remove("", client.ClientID, false); cleanupErr != nil {
			return client, cleanupErr
		}
	}
	return client, err
}

func (n *nativeOAuth) revokeAllClients(ctx context.Context) (int, error) {
	clientIDs, err := n.store.RevokeAllNativeOAuthClients(ctx, time.Now().Unix())
	if err != nil {
		return 0, err
	}
	for _, clientID := range clientIDs {
		n.revokeConnections(clientID)
	}
	if err := n.retries.remove("", "", true); err != nil {
		return 0, err
	}
	return len(clientIDs), nil
}

// Clients returns retained native client metadata.
func (s *Server) Clients(ctx context.Context) ([]store.NativeOAuthClient, error) {
	return s.native.clients(ctx)
}

// RevokeClient revokes one native client and its active connections.
func (s *Server) RevokeClient(ctx context.Context, clientID string) (store.NativeOAuthClient, error) {
	client, err := s.native.revokeClient(ctx, clientID)
	if err == nil && client.ClientID == "" {
		return store.NativeOAuthClient{}, errors.New("client is unavailable")
	}
	return client, err
}

// RevokeAllClients revokes all active native clients.
func (s *Server) RevokeAllClients(ctx context.Context) (int, error) {
	return s.native.revokeAllClients(ctx)
}
