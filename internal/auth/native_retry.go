package auth

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"strings"
	"sync"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/store"
)

const nativeRetryLimit = 1024 * 1024

type nativeTokenResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int64  `json:"expires_in"`
	Scope        string `json:"scope"`
}

type nativeRetryEntry struct {
	IssuedAt     int64  `json:"issued_at"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	RetainUntil  int64  `json:"retain_until,omitempty"`
	FamilyID     string `json:"family_id,omitempty"`
	ClientID     string `json:"client_id,omitempty"`
}

type nativeRetryStore struct {
	path string
	mu   sync.Mutex
}

func (s *nativeRetryStore) load(
	refreshHash [32]byte,
	requestID string,
	now int64,
) (*nativeRetryEntry, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	entries, err := s.read()
	if err != nil {
		return nil, err
	}
	entry, exists := entries[nativeRetryKey(refreshHash, requestID)]
	if !exists || !entry.available(now) {
		return nil, nil
	}
	return &entry, nil
}

func (s *nativeRetryStore) save(
	refreshHash [32]byte,
	requestID string,
	entry nativeRetryEntry,
	now int64,
) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	entries, err := s.read()
	if err != nil {
		return err
	}
	predecessor := hex.EncodeToString(refreshHash[:])
	for key, existing := range entries {
		existingHash := sha256.Sum256([]byte(existing.RefreshToken))
		if !existing.available(now) || key == predecessor ||
			strings.HasPrefix(key, predecessor+":") || existingHash == refreshHash {
			delete(entries, key)
		}
	}
	entries[nativeRetryKey(refreshHash, requestID)] = entry
	return s.write(entries)
}

func (s *nativeRetryStore) prune(now int64) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	entries, err := s.read()
	if err != nil {
		return err
	}
	changed := false
	for key, entry := range entries {
		if !entry.available(now) {
			delete(entries, key)
			changed = true
		}
	}
	if !changed {
		return nil
	}
	return s.write(entries)
}

func (s *nativeRetryStore) remove(familyID string, clientID string, all bool) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	entries, err := s.read()
	if err != nil {
		return err
	}
	changed := false
	for key, entry := range entries {
		if all || familyID != "" && entry.FamilyID == familyID ||
			clientID != "" && entry.ClientID == clientID {
			delete(entries, key)
			changed = true
		}
	}
	if !changed {
		return nil
	}
	return s.write(entries)
}

func (s *nativeRetryStore) write(entries map[string]nativeRetryEntry) error {
	data, err := json.Marshal(entries)
	if err != nil {
		return errors.New("encode native OAuth retry store")
	}
	if len(data) > nativeRetryLimit {
		return errors.New("native OAuth retry store exceeds its size limit")
	}
	return home.AtomicWritePrivate(s.path, data)
}

func (s *nativeRetryStore) read() (map[string]nativeRetryEntry, error) {
	data, err := home.ReadPrivateFile(s.path, nativeRetryLimit)
	if errors.Is(err, os.ErrNotExist) {
		return make(map[string]nativeRetryEntry), nil
	}
	if err != nil {
		return nil, errors.New("read native OAuth retry store")
	}
	entries := make(map[string]nativeRetryEntry)
	if err := json.Unmarshal(data, &entries); err != nil || entries == nil {
		return nil, errors.New("native OAuth retry store is invalid")
	}
	return entries, nil
}

func (e nativeRetryEntry) available(now int64) bool {
	until := e.RetainUntil
	if until == 0 {
		until = e.IssuedAt + 60
	}
	return now >= e.IssuedAt && now <= until
}

func (e nativeRetryEntry) proof(requestBound bool) store.NativeOAuthRetryProof {
	return store.NativeOAuthRetryProof{
		AccessHash:   sha256.Sum256([]byte(e.AccessToken)),
		RefreshHash:  sha256.Sum256([]byte(e.RefreshToken)),
		IssuedAt:     e.IssuedAt,
		RequestBound: requestBound,
	}
}

func (e nativeRetryEntry) response(accessExpiresAt int64, now int64) nativeTokenResponse {
	expiresIn := accessExpiresAt - now
	if expiresIn < 0 {
		expiresIn = 0
	}
	return nativeTokenResponse{
		AccessToken: e.AccessToken, RefreshToken: e.RefreshToken,
		TokenType: "bearer", ExpiresIn: expiresIn, Scope: "noema",
	}
}

func nativeRetryKey(refreshHash [32]byte, requestID string) string {
	key := hex.EncodeToString(refreshHash[:])
	if requestID != "" {
		digest := sha256.Sum256([]byte(requestID))
		key += ":" + hex.EncodeToString(digest[:])
	}
	return key
}
