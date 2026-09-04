package auth

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	recoveryBodyLimit  = 1024
	authBodyLimit      = 256 * 1024
	httpRequestLimit   = 256
	websocketConnLimit = 64
)

// Server owns browser authentication and HTTP admission.
type Server struct {
	config    Config
	recovery  *Recovery
	sessions  *sessionSecurity
	passkeys  *passkeySecurity
	native    *nativeOAuth
	httpSlots chan struct{}
	wsSlots   chan struct{}
}

// New opens browser authentication state for one Go server.
func New(paths home.Paths, taskStore *store.Store, config Config, recovery *Recovery) (*Server, error) {
	if taskStore == nil || recovery == nil {
		return nil, errors.New("browser authentication dependencies are unavailable")
	}
	sessions, err := newSessionSecurity(paths, taskStore, config.Secure)
	if err != nil {
		return nil, err
	}
	passkeys, err := newPasskeySecurity(config, taskStore)
	if err != nil {
		return nil, fmt.Errorf("configure passkeys: %w", err)
	}
	return &Server{
		config:    config,
		recovery:  recovery,
		sessions:  sessions,
		passkeys:  passkeys,
		native:    newNativeOAuth(paths, taskStore),
		httpSlots: make(chan struct{}, httpRequestLimit),
		wsSlots:   make(chan struct{}, websocketConnLimit),
	}, nil
}

// RunCleanup removes expired sessions until the server context ends.
func (s *Server) RunCleanup(ctx context.Context) {
	go s.native.runCleanup(ctx)
	s.sessions.runCleanup(ctx)
}

// Handler applies admission and serves browser-auth routes before the application.
func (s *Server) Handler(application http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		setResponsePolicy(w.Header())
		select {
		case s.httpSlots <- struct{}{}:
			defer func() { <-s.httpSlots }()
		default:
			writeAuthError(w, http.StatusServiceUnavailable, "server_busy")
			return
		}
		if r.Host != s.config.Authority {
			writeAuthError(w, http.StatusBadRequest, "invalid_authority")
			return
		}
		if !s.config.DevNoAuth {
			hasPasskey, err := s.sessions.store.HasPasskey(r.Context())
			if err != nil {
				writeAuthError(w, http.StatusServiceUnavailable, "authentication_unavailable")
				return
			}
			if !hasPasskey && !setupPathAllowed(r.Method, r.URL.Path) {
				writeAuthError(w, http.StatusForbidden, "setup_required")
				return
			}
		}
		var native *nativePrincipal
		if nativeBearerPath(r.URL.Path) && r.Header.Get("Authorization") != "" {
			var err error
			r, native, err = s.native.authenticate(r)
			if errors.Is(err, store.ErrOAuthGrant) {
				writeAuthError(w, http.StatusUnauthorized, "authentication_required")
				return
			}
			if err != nil {
				writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
				return
			}
		}
		if s.requiresOrigin(r) && native == nil && !s.acceptsOrigin(r) {
			writeAuthError(w, http.StatusForbidden, "invalid_origin")
			return
		}
		if strings.HasPrefix(r.URL.Path, "/auth/") {
			s.serveAuth(w, r)
			return
		}
		if strings.HasPrefix(r.URL.Path, "/oauth/") {
			s.native.serve(w, r, s.sessions, s.config.DevNoAuth)
			return
		}
		if r.URL.Path == "/graphql" || r.URL.Path == "/graphql/ws" {
			if hasUpgrade(r) && !isWebSocket(r) {
				http.NotFound(w, r)
				return
			}
			if r.URL.Path == "/graphql/ws" && !isWebSocket(r) {
				w.WriteHeader(http.StatusMethodNotAllowed)
				return
			}
			s.serveGraphQL(w, r, application, native)
			return
		}
		application.ServeHTTP(w, r)
	})
}

func (s *Server) serveAuth(w http.ResponseWriter, r *http.Request) {
	switch {
	case r.Method == http.MethodGet && r.URL.Path == "/auth/status":
		s.status(w, r)
	case r.Method == http.MethodGet && r.URL.Path == "/auth/passkeys":
		s.listPasskeys(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/recovery":
		s.authorizeRecovery(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/passkey/register/start":
		s.startRegistration(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/passkey/register/finish":
		s.finishRegistration(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/passkey/login/start":
		s.startAuthentication(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/passkey/login/finish":
		s.finishAuthentication(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/passkey/remove":
		s.removePasskey(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/logout":
		s.logout(w, r)
	case r.Method == http.MethodPost && r.URL.Path == "/auth/logout/all":
		s.logoutAll(w, r)
	default:
		http.NotFound(w, r)
	}
}

func (s *Server) status(w http.ResponseWriter, r *http.Request) {
	mode := "required"
	state := "login_required"
	if s.config.DevNoAuth {
		mode = "disabled"
		state = "authenticated"
	} else if browser, exists, err := s.sessions.current(r, false); err != nil {
		writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		return
	} else if exists && browser.record.State == "authenticated" {
		state = "authenticated"
	} else if exists, err := s.sessions.store.HasPasskey(r.Context()); err != nil {
		writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		return
	} else if !exists {
		state = "setup_ready"
	}
	writeJSON(w, http.StatusOK, map[string]string{"state": state, "mode": mode})
}

func (s *Server) startRegistration(w http.ResponseWriter, r *http.Request) {
	browser, err := s.sessions.ensureAnonymous(w, r)
	if err != nil {
		writeStoreError(w, err)
		return
	}
	setup := s.sessions.claimSetupStart(browser.digest)
	options, id, err := s.passkeys.beginRegistration(r, browser, s.config.DevNoAuth, setup)
	if err != nil {
		if errors.Is(err, store.ErrSessionUnauthorized) {
			writeAuthError(w, http.StatusForbidden, "setup_not_authorized")
		} else if errors.Is(err, errCeremonyFull) || errors.Is(err, store.ErrSessionFull) {
			writeAuthError(w, http.StatusServiceUnavailable, "ceremony_unavailable")
		} else {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		}
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"ceremonyId": id, "options": options})
}

func (s *Server) finishRegistration(w http.ResponseWriter, r *http.Request) {
	browser, exists, err := s.sessions.current(r, false)
	if err != nil || !exists {
		writeAuthError(w, http.StatusBadRequest, "invalid_ceremony")
		return
	}
	var input finishRequest
	if err := decodeJSON(w, r, authBodyLimit, &input); err != nil {
		writeAuthError(w, http.StatusBadRequest, "invalid_ceremony")
		return
	}
	credential, authority, err := s.passkeys.finishRegistration(
		r,
		browser,
		input.CeremonyID,
		input.Credential,
	)
	if err != nil {
		if errors.Is(err, errInvalidCeremony) {
			writeAuthError(w, http.StatusBadRequest, "invalid_ceremony")
		} else if errors.Is(err, errPasskeyState) {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		} else {
			writeAuthError(w, http.StatusUnauthorized, "passkey_rejected")
		}
		return
	}
	token, digest, err := s.sessions.newToken()
	if err == nil {
		finish := func() error {
			return s.sessions.store.RegisterPasskey(
				r.Context(), credential, authority, browser.digest, digest, time.Now(),
			)
		}
		if authority == store.RegistrationSetup {
			err = s.sessions.finishSetup(browser.digest, finish)
		} else {
			err = finish()
		}
	}
	if err != nil {
		switch {
		case errors.Is(err, store.ErrInitialPasskeyClaimed):
			writeAuthError(w, http.StatusConflict, "initial_passkey_claimed")
		case errors.Is(err, store.ErrPasskeyExists):
			writeAuthError(w, http.StatusConflict, "passkey_already_registered")
		case errors.Is(err, store.ErrSessionUnauthorized):
			writeAuthError(w, http.StatusForbidden, "setup_not_authorized")
		default:
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		}
		return
	}
	s.sessions.revoke(browser.digest)
	s.sessions.setCookie(w, token)
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) startAuthentication(w http.ResponseWriter, r *http.Request) {
	browser, err := s.sessions.ensureAnonymous(w, r)
	if err != nil {
		writeStoreError(w, err)
		return
	}
	options, id, err := s.passkeys.beginAuthentication(r, browser)
	if err != nil {
		if errors.Is(err, store.ErrInitialPasskeyClaimed) {
			writeAuthError(w, http.StatusConflict, "setup_required")
		} else if errors.Is(err, errCeremonyFull) {
			writeAuthError(w, http.StatusServiceUnavailable, "ceremony_unavailable")
		} else {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		}
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"ceremonyId": id, "options": options})
}

func (s *Server) finishAuthentication(w http.ResponseWriter, r *http.Request) {
	browser, exists, err := s.sessions.current(r, false)
	if err != nil || !exists {
		writeAuthError(w, http.StatusBadRequest, "invalid_ceremony")
		return
	}
	var input finishRequest
	if err := decodeJSON(w, r, authBodyLimit, &input); err != nil {
		writeAuthError(w, http.StatusBadRequest, "invalid_ceremony")
		return
	}
	credential, expected, err := s.passkeys.finishAuthentication(
		r,
		browser,
		input.CeremonyID,
		input.Credential,
	)
	if err != nil {
		if errors.Is(err, errInvalidCeremony) {
			writeAuthError(w, http.StatusBadRequest, "invalid_ceremony")
		} else if errors.Is(err, errPasskeyState) {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		} else {
			writeAuthError(w, http.StatusUnauthorized, "passkey_rejected")
		}
		return
	}
	token, digest, err := s.sessions.newToken()
	if err == nil {
		err = s.sessions.store.AuthenticatePasskey(
			r.Context(),
			credential.CredentialID,
			expected,
			credential.CredentialJSON,
			browser.digest,
			digest,
			time.Now(),
		)
	}
	if err != nil {
		if errors.Is(err, store.ErrCredentialChanged) {
			writeAuthError(w, http.StatusConflict, "credential_changed")
		} else {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		}
		return
	}
	s.sessions.revoke(browser.digest)
	s.sessions.setCookie(w, token)
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) authorizeRecovery(w http.ResponseWriter, r *http.Request) {
	if s.config.DevNoAuth {
		http.NotFound(w, r)
		return
	}
	var input struct {
		Code string `json:"code"`
	}
	if err := decodeJSON(w, r, recoveryBodyLimit, &input); err != nil {
		http.Error(w, "recovery rejected", http.StatusUnauthorized)
		return
	}
	matched, err := s.recovery.Attempt(input.Code)
	if err != nil {
		http.Error(w, "recovery unavailable", http.StatusServiceUnavailable)
		return
	}
	if !matched {
		http.Error(w, "recovery rejected", http.StatusUnauthorized)
		return
	}
	browser, err := s.sessions.ensureAnonymous(w, r)
	if err != nil {
		writeStoreError(w, err)
		return
	}
	rotated, err := s.sessions.rotateAnonymous(w, r, browser)
	if err != nil {
		writeStoreError(w, err)
		return
	}
	s.sessions.authorizeSetup(rotated.digest)
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) listPasskeys(w http.ResponseWriter, r *http.Request) {
	browser, authenticated := s.requireAuthenticated(w, r, false)
	if !authenticated {
		return
	}
	passkeys, err := s.sessions.store.Passkeys(r.Context())
	if err != nil {
		writeAuthError(w, http.StatusServiceUnavailable, "authentication_unavailable")
		return
	}
	result := make([]map[string]any, 0, len(passkeys))
	for _, passkey := range passkeys {
		result = append(result, map[string]any{
			"credentialId": passkey.CredentialID,
			"current":      browser.record.PasskeyID == passkey.CredentialID,
		})
	}
	writeJSON(w, http.StatusOK, result)
}

func (s *Server) removePasskey(w http.ResponseWriter, r *http.Request) {
	browser, authenticated := s.requireAuthenticated(w, r, false)
	if !authenticated {
		return
	}
	var input struct {
		CredentialID string `json:"credentialId"`
	}
	if err := decodeJSON(w, r, authBodyLimit, &input); err != nil {
		writeAuthError(w, http.StatusBadRequest, "invalid_request")
		return
	}
	digests, err := s.sessions.store.RemovePasskey(
		r.Context(), input.CredentialID, browser.digest, s.config.DevNoAuth, time.Now(),
	)
	if err != nil {
		switch {
		case errors.Is(err, store.ErrSessionUnauthorized):
			writeAuthError(w, http.StatusForbidden, "recent_passkey_required")
		case errors.Is(err, store.ErrPasskeyNotFound):
			writeAuthError(w, http.StatusNotFound, "passkey_not_found")
		case errors.Is(err, store.ErrFinalPasskey):
			writeAuthError(w, http.StatusConflict, "final_passkey_required")
		default:
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
		}
		return
	}
	s.passkeys.clear()
	s.sessions.revokeAll(digests)
	if digestIncluded(digests, browser.digest) {
		s.sessions.clearCookie(w)
	}
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) logout(w http.ResponseWriter, r *http.Request) {
	if browser, exists, err := s.sessions.current(r, false); err != nil {
		writeAuthError(w, http.StatusInternalServerError, "session_unavailable")
		return
	} else if exists {
		if err := s.sessions.store.DeleteBrowserSession(r.Context(), browser.digest); err != nil {
			writeAuthError(w, http.StatusInternalServerError, "session_unavailable")
			return
		}
		s.sessions.revoke(browser.digest)
	}
	s.sessions.clearCookie(w)
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) logoutAll(w http.ResponseWriter, r *http.Request) {
	if _, authenticated := s.requireAuthenticated(w, r, false); !authenticated {
		return
	}
	digests, err := s.sessions.store.DeleteAllBrowserSessions(r.Context())
	if err != nil {
		writeAuthError(w, http.StatusInternalServerError, "session_unavailable")
		return
	}
	s.sessions.revokeAll(digests)
	s.sessions.clearCookie(w)
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) serveGraphQL(
	w http.ResponseWriter,
	r *http.Request,
	application http.Handler,
	native *nativePrincipal,
) {
	if r.Method == http.MethodPost {
		r.Body = http.MaxBytesReader(w, r.Body, 64*1024)
	}
	if native != nil {
		if !isWebSocket(r) {
			application.ServeHTTP(w, r)
			return
		}
		select {
		case s.wsSlots <- struct{}{}:
			defer func() { <-s.wsSlots }()
		default:
			writeAuthError(w, http.StatusServiceUnavailable, "server_busy")
			return
		}
		s.serveNativeWebSocket(w, r, application, *native)
		return
	}
	if s.config.DevNoAuth {
		application.ServeHTTP(w, r)
		return
	}
	touch := r.Method == http.MethodPost
	browser, exists, err := s.sessions.current(r, touch)
	if err != nil {
		writeAuthError(w, http.StatusInternalServerError, "session_unavailable")
		return
	}
	if !exists || browser.record.State != "authenticated" {
		writeAuthError(w, http.StatusUnauthorized, "authentication_required")
		return
	}
	if !isWebSocket(r) {
		application.ServeHTTP(w, r)
		return
	}
	select {
	case s.wsSlots <- struct{}{}:
		defer func() { <-s.wsSlots }()
	default:
		writeAuthError(w, http.StatusServiceUnavailable, "server_busy")
		return
	}
	ctx, cancel := context.WithCancel(r.Context())
	id := s.sessions.registerConnection(browser.digest, cancel)
	defer func() {
		s.sessions.unregisterConnection(browser.digest, id)
		cancel()
	}()
	if _, stillActive, err := s.sessions.store.BrowserSession(ctx, browser.digest, time.Now(), false); err != nil || !stillActive {
		writeAuthError(w, http.StatusUnauthorized, "authentication_required")
		return
	}
	application.ServeHTTP(w, r.WithContext(ctx))
}

func (s *Server) serveNativeWebSocket(
	w http.ResponseWriter,
	r *http.Request,
	application http.Handler,
	principal nativePrincipal,
) {
	ctx, cancel := context.WithDeadline(r.Context(), principal.expiresAt)
	id := s.native.registerConnection(principal.clientID, cancel)
	defer func() {
		s.native.unregisterConnection(principal.clientID, id)
		cancel()
	}()
	access, exists, err := s.sessions.store.ActiveNativeOAuthAccess(ctx, principal.accessHash, time.Now().Unix())
	if err != nil || !exists || access.ClientID != principal.clientID {
		writeAuthError(w, http.StatusUnauthorized, "authentication_required")
		return
	}
	application.ServeHTTP(w, r.WithContext(ctx))
}

func (s *Server) requireAuthenticated(
	w http.ResponseWriter,
	r *http.Request,
	touch bool,
) (browserSession, bool) {
	if s.config.DevNoAuth {
		browser, _, err := s.sessions.current(r, touch)
		if err != nil {
			writeAuthError(w, http.StatusInternalServerError, "session_unavailable")
			return browserSession{}, false
		}
		return browser, true
	}
	browser, exists, err := s.sessions.current(r, touch)
	if err != nil {
		writeAuthError(w, http.StatusInternalServerError, "session_unavailable")
		return browserSession{}, false
	}
	if !exists || browser.record.State != "authenticated" {
		writeAuthError(w, http.StatusUnauthorized, "authentication_required")
		return browserSession{}, false
	}
	return browser, true
}

func (s *Server) requiresOrigin(r *http.Request) bool {
	needsOrigin := r.Method == http.MethodPost &&
		(strings.HasPrefix(r.URL.Path, "/graphql") || strings.HasPrefix(r.URL.Path, "/auth/")) ||
		(r.Method == http.MethodPost && r.URL.Path == "/oauth/authorize") || hasUpgrade(r)
	return needsOrigin
}

func (s *Server) acceptsOrigin(r *http.Request) bool {
	origin := r.Header.Get("Origin")
	if r.Method == http.MethodPost && r.URL.Path == "/oauth/authorize" &&
		(origin == "" || origin == "null") {
		return true
	}
	return origin == s.config.Origin
}

func isWebSocket(r *http.Request) bool {
	return r.Method == http.MethodGet && r.URL.Path == "/graphql/ws" &&
		strings.EqualFold(r.Header.Get("Upgrade"), "websocket")
}

func hasUpgrade(r *http.Request) bool { return r.Header.Get("Upgrade") != "" }

func nativeBearerPath(path string) bool {
	return path == "/graphql" || path == "/graphql/ws" ||
		strings.HasPrefix(path, "/artifacts/versions/") || strings.HasPrefix(path, "/favicons/")
}

func setupPathAllowed(method string, path string) bool {
	if method == http.MethodGet && (path == "/" || path == "/auth/status") {
		return true
	}
	if method == http.MethodPost && (path == "/auth/recovery" ||
		path == "/auth/passkey/register/start" ||
		path == "/auth/passkey/register/finish") {
		return true
	}
	return (method == http.MethodGet || method == http.MethodHead) && strings.HasPrefix(path, "/assets/")
}

type finishRequest struct {
	CeremonyID string          `json:"ceremonyId"`
	Credential json.RawMessage `json:"credential"`
}

func decodeJSON(w http.ResponseWriter, r *http.Request, limit int64, target any) error {
	r.Body = http.MaxBytesReader(w, r.Body, limit)
	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); errors.Is(err, io.EOF) {
		return nil
	} else if err != nil {
		return err
	}
	return errors.New("request contains multiple JSON values")
}

func writeStoreError(w http.ResponseWriter, err error) {
	if errors.Is(err, store.ErrSessionFull) {
		writeAuthError(w, http.StatusServiceUnavailable, "session_unavailable")
		return
	}
	writeAuthError(w, http.StatusInternalServerError, "session_unavailable")
}

func writeAuthError(w http.ResponseWriter, status int, code string) {
	writeJSON(w, status, map[string]string{"error": code})
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}

func setResponsePolicy(header http.Header) {
	header.Set("Cache-Control", "no-store")
	header.Set("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'")
	header.Set("Permissions-Policy", "publickey-credentials-create=(self), publickey-credentials-get=(self)")
	header.Set("Referrer-Policy", "no-referrer")
	header.Set("X-Content-Type-Options", "nosniff")
}

func digestIncluded(digests [][32]byte, selected [32]byte) bool {
	for _, digest := range digests {
		if digest == selected {
			return true
		}
	}
	return false
}
