package auth

import (
	"context"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"errors"
	"fmt"
	"net/http"
	"os"
	"sync"
	"sync/atomic"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

const sessionKeySize = 64

type browserSession struct {
	digest [32]byte
	record store.BrowserSession
}

type setupGrant struct {
	expiresAt       time.Time
	remainingStarts int
}

type sessionSecurity struct {
	store      *store.Store
	key        []byte
	secure     bool
	cookieName string

	setupMu sync.Mutex
	setups  map[[32]byte]setupGrant

	connectionsMu sync.Mutex
	connectionID  atomic.Uint64
	connections   map[[32]byte]map[uint64]context.CancelFunc
}

func newSessionSecurity(paths home.Paths, taskStore *store.Store, secure bool) (*sessionSecurity, error) {
	key, err := loadOrCreateSessionKey(paths.BrowserSessionKey())
	if err != nil {
		return nil, err
	}
	cookieName := "noema.sid"
	if secure {
		cookieName = "__Host-noema.sid"
	}
	return &sessionSecurity{
		store:       taskStore,
		key:         key,
		secure:      secure,
		cookieName:  cookieName,
		setups:      make(map[[32]byte]setupGrant),
		connections: make(map[[32]byte]map[uint64]context.CancelFunc),
	}, nil
}

func (s *sessionSecurity) current(r *http.Request, touch bool) (browserSession, bool, error) {
	cookie, err := r.Cookie(s.cookieName)
	if errors.Is(err, http.ErrNoCookie) {
		return browserSession{}, false, nil
	}
	if err != nil {
		return browserSession{}, false, err
	}
	digest, ok := s.verifyCookie(cookie.Value)
	if !ok {
		return browserSession{}, false, nil
	}
	record, exists, err := s.store.BrowserSession(r.Context(), digest, time.Now(), touch)
	if err != nil || !exists {
		return browserSession{}, false, err
	}
	return browserSession{digest: digest, record: record}, true, nil
}

func (s *sessionSecurity) ensureAnonymous(w http.ResponseWriter, r *http.Request) (browserSession, error) {
	if current, exists, err := s.current(r, false); err != nil {
		return browserSession{}, err
	} else if exists {
		return current, nil
	}
	token, digest, err := s.newToken()
	if err != nil {
		return browserSession{}, err
	}
	if err := s.store.CreateAnonymousSession(r.Context(), digest, time.Now()); err != nil {
		return browserSession{}, err
	}
	s.setCookie(w, token)
	record, exists, err := s.store.BrowserSession(r.Context(), digest, time.Now(), false)
	if err != nil || !exists {
		return browserSession{}, errors.New("created browser session is unavailable")
	}
	return browserSession{digest: digest, record: record}, nil
}

func (s *sessionSecurity) rotateAnonymous(
	w http.ResponseWriter,
	r *http.Request,
	current browserSession,
) (browserSession, error) {
	token, digest, err := s.newToken()
	if err != nil {
		return browserSession{}, err
	}
	if err := s.store.RotateAnonymousSession(r.Context(), current.digest, digest, time.Now()); err != nil {
		return browserSession{}, err
	}
	s.revoke(current.digest)
	s.setCookie(w, token)
	record, exists, err := s.store.BrowserSession(r.Context(), digest, time.Now(), false)
	if err != nil || !exists {
		return browserSession{}, errors.New("rotated browser session is unavailable")
	}
	return browserSession{digest: digest, record: record}, nil
}

func (s *sessionSecurity) clearCookie(w http.ResponseWriter) {
	http.SetCookie(w, &http.Cookie{
		Name:     s.cookieName,
		Path:     "/",
		MaxAge:   -1,
		HttpOnly: true,
		Secure:   s.secure,
		SameSite: http.SameSiteStrictMode,
	})
}

func (s *sessionSecurity) setCookie(w http.ResponseWriter, value string) {
	http.SetCookie(w, &http.Cookie{
		Name:     s.cookieName,
		Value:    value,
		Path:     "/",
		HttpOnly: true,
		Secure:   s.secure,
		SameSite: http.SameSiteStrictMode,
		MaxAge:   int((30 * 24 * time.Hour).Seconds()),
	})
}

func (s *sessionSecurity) newToken() (string, [32]byte, error) {
	var identifier [32]byte
	if _, err := rand.Read(identifier[:]); err != nil {
		return "", [32]byte{}, fmt.Errorf("generate browser session: %w", err)
	}
	digest := s.keyedValue("session digest", identifier[:])
	signature := s.keyedValue("cookie signature", identifier[:])
	return base64.RawURLEncoding.EncodeToString(identifier[:]) + "." +
		base64.RawURLEncoding.EncodeToString(signature[:]), digest, nil
}

func (s *sessionSecurity) verifyCookie(value string) ([32]byte, bool) {
	if len(value) != 87 || value[43] != '.' {
		return [32]byte{}, false
	}
	identifier, err := base64.RawURLEncoding.DecodeString(value[:43])
	if err != nil || len(identifier) != 32 {
		return [32]byte{}, false
	}
	signature, err := base64.RawURLEncoding.DecodeString(value[44:])
	if err != nil || len(signature) != sha256.Size {
		return [32]byte{}, false
	}
	expected := s.keyedValue("cookie signature", identifier)
	if !hmac.Equal(signature, expected[:]) {
		return [32]byte{}, false
	}
	return s.keyedValue("session digest", identifier), true
}

func (s *sessionSecurity) keyedValue(purpose string, value []byte) [32]byte {
	mac := hmac.New(sha256.New, s.key)
	_, _ = mac.Write([]byte(purpose))
	_, _ = mac.Write(value)
	var output [32]byte
	copy(output[:], mac.Sum(nil))
	return output
}

func (s *sessionSecurity) authorizeSetup(digest [32]byte) {
	s.setupMu.Lock()
	defer s.setupMu.Unlock()
	clear(s.setups)
	s.setups[digest] = setupGrant{
		expiresAt:       time.Now().Add(5 * time.Minute),
		remainingStarts: 8,
	}
}

func (s *sessionSecurity) claimSetupStart(digest [32]byte) bool {
	s.setupMu.Lock()
	defer s.setupMu.Unlock()
	now := time.Now()
	for key, grant := range s.setups {
		if !grant.expiresAt.After(now) {
			delete(s.setups, key)
		}
	}
	grant, exists := s.setups[digest]
	if !exists || grant.remainingStarts == 0 {
		return false
	}
	grant.remainingStarts--
	s.setups[digest] = grant
	return true
}

func (s *sessionSecurity) consumeSetup(digest [32]byte) {
	s.setupMu.Lock()
	delete(s.setups, digest)
	s.setupMu.Unlock()
}

func (s *sessionSecurity) finishSetup(digest [32]byte, finish func() error) error {
	s.setupMu.Lock()
	defer s.setupMu.Unlock()
	grant, exists := s.setups[digest]
	if !exists || !grant.expiresAt.After(time.Now()) {
		delete(s.setups, digest)
		return store.ErrSessionUnauthorized
	}
	if err := finish(); err != nil {
		return err
	}
	delete(s.setups, digest)
	return nil
}

func (s *sessionSecurity) registerConnection(digest [32]byte, cancel context.CancelFunc) uint64 {
	s.connectionsMu.Lock()
	defer s.connectionsMu.Unlock()
	if s.connections[digest] == nil {
		s.connections[digest] = make(map[uint64]context.CancelFunc)
	}
	id := s.connectionID.Add(1)
	s.connections[digest][id] = cancel
	return id
}

func (s *sessionSecurity) unregisterConnection(digest [32]byte, id uint64) {
	s.connectionsMu.Lock()
	defer s.connectionsMu.Unlock()
	delete(s.connections[digest], id)
	if len(s.connections[digest]) == 0 {
		delete(s.connections, digest)
	}
}

func (s *sessionSecurity) revoke(digest [32]byte) {
	s.connectionsMu.Lock()
	connections := s.connections[digest]
	delete(s.connections, digest)
	s.connectionsMu.Unlock()
	for _, cancel := range connections {
		cancel()
	}
	s.consumeSetup(digest)
}

func (s *sessionSecurity) revokeAll(digests [][32]byte) {
	for _, digest := range digests {
		s.revoke(digest)
	}
}

func (s *sessionSecurity) runCleanup(ctx context.Context) {
	ticker := time.NewTicker(time.Minute)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case now := <-ticker.C:
			digests, err := s.store.DeleteExpiredBrowserSessions(ctx, now)
			if err == nil {
				s.revokeAll(digests)
			}
		}
	}
}

func loadOrCreateSessionKey(path string) ([]byte, error) {
	data, err := home.ReadPrivateFile(path, sessionKeySize)
	if err == nil {
		if len(data) != sessionKeySize {
			return nil, errors.New("browser session key has an invalid length")
		}
		return data, nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return nil, fmt.Errorf("read browser session key: %w", err)
	}
	data = make([]byte, sessionKeySize)
	if _, err := rand.Read(data); err != nil {
		return nil, errors.New("generate browser session key")
	}
	if err := home.AtomicWritePrivate(path, data); err != nil {
		return nil, fmt.Errorf("store browser session key: %w", err)
	}
	verified, err := home.ReadPrivateFile(path, sessionKeySize)
	if err != nil || !hmac.Equal(data, verified) {
		return nil, errors.New("verify browser session key")
	}
	return data, nil
}
