package notification

import (
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/sha256"
	"crypto/x509"
	"encoding/hex"
	"encoding/json"
	"encoding/pem"
	"errors"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
)

const (
	apnsTopic       = "dev.noema.app.ios"
	apnsConfigLimit = 64 * 1024
)

// APNSProviderStatus is safe to expose to clients. It does not contain the private key.
type APNSProviderStatus struct {
	Configured     bool
	TeamID         *string
	KeyID          *string
	Topic          string
	KeyFingerprint *string
	Revision       int
	UpdatedAt      *string
	LastErrorCode  *string
	LastErrorAt    *string
}

type apnsCredential struct {
	Configured     bool    `json:"configured"`
	TeamID         *string `json:"team_id"`
	KeyID          *string `json:"key_id"`
	Topic          string  `json:"topic"`
	KeyFingerprint *string `json:"key_fingerprint"`
	PrivateKeyPEM  *string `json:"private_key_pem"`
	Revision       int     `json:"revision"`
	UpdatedAt      *string `json:"updated_at"`
	LastErrorCode  *string `json:"last_error_code"`
	LastErrorAt    *string `json:"last_error_at"`
}

func (c apnsCredential) String() string {
	return fmt.Sprintf("apnsCredential{Configured:%t, Topic:%q, Revision:%d}", c.Configured, c.Topic, c.Revision)
}

func (c apnsCredential) GoString() string { return c.String() }

func (c apnsCredential) status() APNSProviderStatus {
	return APNSProviderStatus{Configured: c.Configured, TeamID: c.TeamID, KeyID: c.KeyID,
		Topic: c.Topic, KeyFingerprint: c.KeyFingerprint, Revision: c.Revision,
		UpdatedAt: c.UpdatedAt, LastErrorCode: c.LastErrorCode, LastErrorAt: c.LastErrorAt}
}

// APNSProviderStatus returns provider metadata without key material.
func (s *Service) APNSProviderStatus() (APNSProviderStatus, error) {
	c, err := readAPNSCredential(s.paths.APNSProvider())
	return c.status(), err
}

// ConfigureAPNS validates and atomically replaces the protected provider configuration.
func (s *Service) ConfigureAPNS(teamID, keyID, privateKeyPEM string, expected int) (APNSProviderStatus, error) {
	teamID, keyID, privateKeyPEM = strings.TrimSpace(teamID), strings.TrimSpace(keyID), strings.TrimSpace(privateKeyPEM)
	if expected < 0 || !validAPNSID(teamID) || !validAPNSID(keyID) {
		return APNSProviderStatus{}, errors.New("APNs provider input is invalid")
	}
	private, err := parseAPNSKey(privateKeyPEM)
	if err != nil {
		return APNSProviderStatus{}, err
	}
	fingerprint := apnsFingerprint(&private.PublicKey)
	s.mu.Lock()
	defer s.mu.Unlock()
	current, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil {
		return APNSProviderStatus{}, err
	}
	if current.Revision != expected {
		return APNSProviderStatus{}, errors.New("APNs provider revision conflict")
	}
	now := time.Now().UTC().Format(time.RFC3339Nano)
	next := apnsCredential{Configured: true, TeamID: &teamID, KeyID: &keyID,
		Topic: apnsTopic, KeyFingerprint: &fingerprint, PrivateKeyPEM: &privateKeyPEM,
		Revision: current.Revision + 1, UpdatedAt: &now}
	if err := writeAPNSCredential(s.paths.APNSProvider(), next); err != nil {
		return APNSProviderStatus{}, err
	}
	return next.status(), nil
}

// RemoveAPNS atomically writes a revisioned tombstone and fails pending deliveries.
func (s *Service) RemoveAPNS(ctx context.Context, expected int) (APNSProviderStatus, error) {
	if expected < 0 {
		return APNSProviderStatus{}, errors.New("APNs provider revision must be non-negative")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	current, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil {
		return APNSProviderStatus{}, err
	}
	if current.Revision != expected {
		return APNSProviderStatus{}, errors.New("APNs provider revision conflict")
	}
	now := time.Now().UTC().Format(time.RFC3339Nano)
	next := apnsCredential{Topic: apnsTopic, Revision: current.Revision + 1, UpdatedAt: &now}
	if err := writeAPNSCredential(s.paths.APNSProvider(), next); err != nil {
		return APNSProviderStatus{}, err
	}
	if err := s.database.FailPendingAPNS(ctx, "provider_unconfigured", time.Now()); err != nil {
		return APNSProviderStatus{}, err
	}
	return next.status(), nil
}

func (s *Service) recordAPNSError(code string, revision int) error {
	if revision == 0 || code == "" {
		return nil
	}
	if len(code) > 128 {
		code = code[:128]
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	current, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil || current.Revision != revision {
		return err
	}
	now := time.Now().UTC().Format(time.RFC3339Nano)
	current.LastErrorCode, current.LastErrorAt, current.UpdatedAt = &code, &now, &now
	return writeAPNSCredential(s.paths.APNSProvider(), current)
}

func readAPNSCredential(path string) (apnsCredential, error) {
	data, err := home.ReadPrivateFile(path, apnsConfigLimit)
	if errors.Is(err, os.ErrNotExist) {
		return apnsCredential{Topic: apnsTopic}, nil
	}
	if err != nil {
		return apnsCredential{}, errors.New("APNs provider credential is unavailable")
	}
	var value apnsCredential
	if json.Unmarshal(data, &value) != nil || validateAPNSCredential(value) != nil {
		return apnsCredential{}, errors.New("APNs provider credential is invalid")
	}
	return value, nil
}

func writeAPNSCredential(path string, value apnsCredential) error {
	if err := validateAPNSCredential(value); err != nil {
		return err
	}
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return errors.New("encode APNs provider credential")
	}
	if err := home.AtomicWritePrivate(path, data); err != nil {
		return fmt.Errorf("store APNs provider credential: %w", err)
	}
	return nil
}

func validateAPNSCredential(value apnsCredential) error {
	if value.Topic != apnsTopic || value.Revision < 0 ||
		(value.LastErrorCode != nil && (*value.LastErrorCode == "" || len(*value.LastErrorCode) > 128 || strings.TrimSpace(*value.LastErrorCode) != *value.LastErrorCode)) {
		return errors.New("APNs provider credential is invalid")
	}
	if !value.Configured {
		if value.TeamID != nil || value.KeyID != nil || value.KeyFingerprint != nil || value.PrivateKeyPEM != nil {
			return errors.New("APNs provider tombstone is invalid")
		}
		return nil
	}
	if value.TeamID == nil || value.KeyID == nil || value.KeyFingerprint == nil || value.PrivateKeyPEM == nil ||
		!validAPNSID(*value.TeamID) || !validAPNSID(*value.KeyID) || len(*value.KeyFingerprint) != 64 {
		return errors.New("APNs provider metadata is invalid")
	}
	private, err := parseAPNSKey(*value.PrivateKeyPEM)
	if err != nil || apnsFingerprint(&private.PublicKey) != *value.KeyFingerprint {
		return errors.New("APNs private key is invalid")
	}
	return nil
}

func validAPNSID(value string) bool {
	if len(value) != 10 {
		return false
	}
	for _, b := range []byte(value) {
		if (b < 'A' || b > 'Z') && (b < '0' || b > '9') {
			return false
		}
	}
	return true
}

func parseAPNSKey(value string) (*ecdsa.PrivateKey, error) {
	if len(value) == 0 || len(value) > 16*1024 {
		return nil, errors.New("APNs private key is invalid")
	}
	block, rest := pem.Decode([]byte(value))
	if block == nil || block.Type != "PRIVATE KEY" || strings.TrimSpace(string(rest)) != "" {
		return nil, errors.New("APNs private key is invalid")
	}
	parsed, err := x509.ParsePKCS8PrivateKey(block.Bytes)
	private, ok := parsed.(*ecdsa.PrivateKey)
	if err != nil || !ok || private.Curve != elliptic.P256() {
		return nil, errors.New("APNs private key is invalid")
	}
	return private, nil
}

func apnsFingerprint(public *ecdsa.PublicKey) string {
	digest := sha256.Sum256(elliptic.Marshal(elliptic.P256(), public.X, public.Y))
	return hex.EncodeToString(digest[:])
}
