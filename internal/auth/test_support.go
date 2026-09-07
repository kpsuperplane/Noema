package auth

// This file exposes the same test-only authentication boundary that the Rust
// router adds under cfg(test). Production code does not call TestHandler.

import (
	"encoding/json"
	"net/http"
	"time"

	webauthnlib "github.com/go-webauthn/webauthn/webauthn"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/web"
)

// TestHandler adds /__test/authenticate for black-box server parity tests.
// The route uses the normal session and store authorities before forwarding
// every other request to Handler.
func (s *Server) TestHandler(application http.Handler) http.Handler {
	public := s.Handler(application)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/__test/authenticate" {
			public.ServeHTTP(w, r)
			return
		}
		setResponsePolicy(w.Header())
		if r.Method != http.MethodPost || r.Host != s.config.Authority {
			web.WriteNotFound(w, r)
			return
		}
		passkeys, err := s.sessions.store.Passkeys(r.Context())
		if err != nil {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
			return
		}
		var testPasskey *store.HumanPasskey
		for index := range passkeys {
			if passkeys[index].CredentialID == "test-passkey" {
				value := passkeys[index]
				testPasskey = &value
				break
			}
		}
		if testPasskey == nil {
			credential := testSessionCredential()
			browser, exists, currentErr := s.sessions.current(r, false)
			if currentErr != nil {
				writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
				return
			}
			if !exists {
				_, browser.digest, currentErr = s.sessions.newToken()
				if currentErr == nil {
					currentErr = s.sessions.store.CreateAnonymousSession(r.Context(), browser.digest, time.Now())
				}
			}
			newToken, newDigest, tokenErr := s.sessions.newToken()
			if currentErr == nil {
				currentErr = tokenErr
			}
			authority := store.RegistrationBypass
			if len(passkeys) == 0 {
				authority = store.RegistrationInitial
			}
			if currentErr == nil {
				currentErr = s.sessions.store.RegisterPasskey(r.Context(), credential, authority, browser.digest, newDigest, time.Now())
			}
			if currentErr != nil {
				writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
				return
			}
			s.sessions.revoke(browser.digest)
			s.sessions.setCookie(w, newToken)
			w.WriteHeader(http.StatusNoContent)
			return
		}
		browser, exists, err := s.sessions.current(r, false)
		if err != nil || !exists {
			_, browser.digest, err = s.sessions.newToken()
			if err == nil {
				err = s.sessions.store.CreateAnonymousSession(r.Context(), browser.digest, time.Now())
			}
		}
		if err != nil {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
			return
		}
		token, digest, err := s.sessions.newToken()
		if err == nil {
			err = s.sessions.store.AuthenticatePasskey(r.Context(), testPasskey.CredentialID, testPasskey.CredentialJSON, testPasskey.CredentialJSON, browser.digest, digest, time.Now())
		}
		if err != nil {
			writeAuthError(w, http.StatusInternalServerError, "authentication_unavailable")
			return
		}
		s.sessions.revoke(browser.digest)
		s.sessions.setCookie(w, token)
		w.WriteHeader(http.StatusNoContent)
	})
}

func testSessionCredential() store.HumanPasskey {
	encoded, _ := json.Marshal(&webauthnlib.Credential{ID: []byte("test-passkey")})
	return store.HumanPasskey{CredentialID: "test-passkey", CredentialJSON: string(encoded)}
}
