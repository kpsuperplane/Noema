package provider

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
)

const credentialFileLimit = 1 << 20

// AccountPersistence stores safe account metadata. It never receives credentials.
type AccountPersistence interface {
	EnsureBuiltinProviderAccounts(context.Context, time.Time) error
	ProviderAccount(context.Context, string) (Account, error)
	ActiveProviderAccounts(context.Context) ([]Account, error)
	CreateProviderAccount(context.Context, Account) (Account, error)
	UpdateProviderCredential(context.Context, string, uint64, AuthMethod, bool, time.Time) (Account, error)
	DeleteProviderAccount(context.Context, string) (bool, error)
}

// Secret is credential material that does not support string formatting.
type Secret struct {
	value string
}

// NewSecret validates credential input without exposing it through formatting.
func NewSecret(value string) (Secret, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return Secret{}, errors.New("provider secret cannot be empty")
	}
	return Secret{value: value}, nil
}

// Use supplies the secret only to one explicit secure binding.
func (s Secret) Use(binding func(string) error) error {
	if binding == nil || s.value == "" {
		return errors.New("provider secret is unavailable")
	}
	return binding(s.value)
}

// GoString prevents diagnostic formatting from exposing credential material.
func (Secret) GoString() string { return "provider.Secret{[REDACTED]}" }

// String prevents diagnostic formatting from exposing credential material.
func (Secret) String() string { return "[REDACTED]" }

// AccountService coordinates account metadata and protected credential files.
type AccountService struct {
	root        string
	persistence AccountPersistence
	gatesMu     sync.Mutex
	gates       map[string]*sync.Mutex
}

// NewAccountService creates one account service for an absolute Noema home.
func NewAccountService(root string, persistence AccountPersistence) (*AccountService, error) {
	if !filepath.IsAbs(root) {
		return nil, errors.New("provider account home must be absolute")
	}
	if persistence == nil {
		return nil, errors.New("provider account persistence is required")
	}
	return &AccountService{
		root: filepath.Clean(root), persistence: persistence, gates: make(map[string]*sync.Mutex),
	}, nil
}

// Initialize creates missing permanent provider account metadata.
func (s *AccountService) Initialize(ctx context.Context, now time.Time) error {
	return s.persistence.EnsureBuiltinProviderAccounts(ctx, now.UTC())
}

// Accounts returns active provider account metadata.
func (s *AccountService) Accounts(ctx context.Context) ([]Account, error) {
	return s.persistence.ActiveProviderAccounts(ctx)
}

// CreateSecretAccount creates a user-managed account with one protected secret.
func (s *AccountService) CreateSecretAccount(
	ctx context.Context,
	providerKind string,
	displayName string,
	secret Secret,
	now time.Time,
) (Account, error) {
	entry, ok := CatalogEntryFor(providerKind)
	if !ok {
		return Account{}, ErrUnsupportedProvider
	}
	if !supportsAuth(entry, AuthSecretInput) {
		return Account{}, ErrAuthMethodMismatch
	}
	if secret.value == "" {
		return Account{}, errors.New("provider secret cannot be empty")
	}
	var id, accountKey string
	var err error
	if entry.GeneratesAccountKey {
		id, accountKey, err = newAccountIdentity(providerKind)
		if err != nil {
			return Account{}, err
		}
	} else {
		id, accountKey = "provider_account:"+providerKind+":default", "default"
	}
	displayName = strings.TrimSpace(displayName)
	if displayName == "" {
		displayName = entry.DisplayName
	}
	now = now.UTC()
	account := Account{
		ID: id, ProviderKind: providerKind, AccountKey: accountKey, DisplayName: displayName,
		AuthMethod: AuthSecretInput, IsActive: true, IsDefault: !entry.GeneratesAccountKey,
		Status: StatusAuthenticated, Metadata: credentialMetadata(1, true),
		CreatedAt: now, UpdatedAt: now,
	}
	gate := s.gate(account.ID)
	gate.Lock()
	defer gate.Unlock()
	path, err := s.credentialPath(account)
	if err != nil {
		return Account{}, err
	}
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		return Account{}, err
	}
	if err := writeSecret(path, secret); err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	created, err := s.persistence.CreateProviderAccount(ctx, account)
	if err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	return created, nil
}

// SaveSecret atomically changes one account credential and its safe metadata.
func (s *AccountService) SaveSecret(
	ctx context.Context,
	id string,
	secret Secret,
	now time.Time,
) (Account, error) {
	if secret.value == "" {
		return Account{}, errors.New("provider secret cannot be empty")
	}
	return s.mutateSecret(ctx, id, AuthSecretInput, true, now, func(path string) error {
		return writeSecret(path, secret)
	})
}

// ClearSecret atomically removes one account credential and changes its safe metadata.
func (s *AccountService) ClearSecret(ctx context.Context, id string, now time.Time) (Account, error) {
	return s.mutateSecret(ctx, id, "", false, now, home.RemovePrivateFile)
}

// LoadSecret returns one credential through a typed wrapper.
func (s *AccountService) LoadSecret(ctx context.Context, id string) (Secret, error) {
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()
	account, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return Secret{}, err
	}
	path, err := s.credentialPath(account)
	if err != nil {
		return Secret{}, err
	}
	data, err := home.ReadPrivateFile(path, credentialFileLimit)
	if err != nil {
		return Secret{}, errors.New("provider secret is unavailable")
	}
	var file credentialFile
	if err := decodeCredentialFile(data, &file); err != nil {
		return Secret{}, errors.New("provider secret is unavailable")
	}
	return NewSecret(file.APIKey)
}

// DeleteAccount removes one user-managed account and its credential directory.
func (s *AccountService) DeleteAccount(ctx context.Context, id string) (bool, error) {
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()
	account, err := s.persistence.ProviderAccount(ctx, id)
	if errors.Is(err, ErrAccountNotFound) {
		return false, nil
	}
	if err != nil {
		return false, err
	}
	if IsBuiltinAccountID(account.ID) {
		return false, ErrProtectedAccount
	}
	original, quarantine, err := s.quarantineAccountHome(account)
	if err != nil {
		return false, err
	}
	deleted, deleteErr := s.persistence.DeleteProviderAccount(ctx, id)
	if deleteErr != nil || !deleted {
		if restoreErr := restoreQuarantine(original, quarantine); restoreErr != nil {
			return false, ErrCompensationFailed
		}
		if deleteErr != nil {
			return false, deleteErr
		}
		return false, ErrAccountConflict
	}
	if quarantine != "" {
		_ = os.RemoveAll(quarantine)
	}
	return true, nil
}

func (s *AccountService) mutateSecret(
	ctx context.Context,
	id string,
	method AuthMethod,
	configured bool,
	now time.Time,
	mutation func(string) error,
) (Account, error) {
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()
	account, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return Account{}, err
	}
	entry, ok := CatalogEntryFor(account.ProviderKind)
	if !ok || !supportsAuth(entry, AuthSecretInput) {
		return Account{}, ErrAuthMethodMismatch
	}
	if method == "" {
		method = account.AuthMethod
	}
	path, err := s.credentialPath(account)
	if err != nil {
		return Account{}, err
	}
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		return Account{}, err
	}
	if err := mutation(path); err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	updated, err := s.persistence.UpdateProviderCredential(
		ctx, id, account.Metadata.CredentialRevision(), method, configured, now.UTC(),
	)
	if err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	return updated, nil
}

func (s *AccountService) gate(id string) *sync.Mutex {
	s.gatesMu.Lock()
	defer s.gatesMu.Unlock()
	if s.gates[id] == nil {
		s.gates[id] = &sync.Mutex{}
	}
	return s.gates[id]
}

func (s *AccountService) credentialPath(account Account) (string, error) {
	if err := validateAccount(account); err != nil {
		return "", err
	}
	return filepath.Join(s.root, "providers", account.ProviderKind, account.AccountKey, "api_key.json"), nil
}

type credentialFile struct {
	APIKey string `json:"api_key"`
}

func writeSecret(path string, secret Secret) error {
	data, err := json.Marshal(credentialFile{APIKey: secret.value})
	if err != nil {
		return errors.New("encode provider secret")
	}
	if err := home.AtomicWritePrivate(path, data); err != nil {
		return fmt.Errorf("write provider secret: %w", err)
	}
	return nil
}

func decodeCredentialFile(data []byte, destination *credentialFile) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return errors.New("provider credential has trailing data")
	}
	return nil
}

type fileSnapshot struct {
	exists bool
	data   []byte
}

func snapshotPrivateFile(path string) (fileSnapshot, error) {
	data, err := home.ReadPrivateFile(path, credentialFileLimit)
	if errors.Is(err, os.ErrNotExist) {
		return fileSnapshot{}, nil
	}
	if err != nil {
		return fileSnapshot{}, fmt.Errorf("snapshot provider secret: %w", err)
	}
	return fileSnapshot{exists: true, data: data}, nil
}

func restorePrivateFile(path string, snapshot fileSnapshot) error {
	if snapshot.exists {
		return home.AtomicWritePrivate(path, snapshot.data)
	}
	return home.RemovePrivateFile(path)
}

func (s *AccountService) quarantineAccountHome(account Account) (string, string, error) {
	path, err := s.credentialPath(account)
	if err != nil {
		return "", "", err
	}
	original := filepath.Dir(path)
	if _, err := os.Lstat(original); errors.Is(err, os.ErrNotExist) {
		return original, "", nil
	} else if err != nil {
		return "", "", fmt.Errorf("inspect provider account files: %w", err)
	}
	temporary, err := os.MkdirTemp(filepath.Dir(original), ".noema-delete-*")
	if err != nil {
		return "", "", fmt.Errorf("prepare provider account quarantine: %w", err)
	}
	if err := os.Remove(temporary); err != nil {
		return "", "", fmt.Errorf("prepare provider account quarantine: %w", err)
	}
	if err := os.Rename(original, temporary); err != nil {
		return "", "", fmt.Errorf("quarantine provider account files: %w", err)
	}
	return original, temporary, nil
}

func restoreQuarantine(original string, quarantine string) error {
	if quarantine == "" {
		return nil
	}
	return os.Rename(quarantine, original)
}
