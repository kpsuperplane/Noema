package provider

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
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
	UpdateProviderCredential(context.Context, string, uint64, AuthMethod, bool, AccountMetadata, time.Time) (Account, error)
	DeleteProviderAccount(context.Context, string) (bool, error)
}

type authFailurePersistence interface {
	MarkProviderAuthenticationFailed(context.Context, string, uint64, time.Time) error
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
	root             string
	persistence      AccountPersistence
	runtime          *ProviderRuntime
	codexRefreshGate chan struct{}
}

// NewAccountService creates one account service for an absolute Noema home.
func NewAccountService(root string, persistence AccountPersistence) (*AccountService, error) {
	if !filepath.IsAbs(root) {
		return nil, errors.New("provider account home must be absolute")
	}
	if persistence == nil {
		return nil, errors.New("provider account persistence is required")
	}
	service := &AccountService{
		root: filepath.Clean(root), persistence: persistence, runtime: newProviderRuntime(),
		codexRefreshGate: make(chan struct{}, 1),
	}
	service.codexRefreshGate <- struct{}{}
	return service, nil
}

// Initialize creates missing permanent provider account metadata.
func (s *AccountService) Initialize(ctx context.Context, now time.Time) error {
	return s.persistence.EnsureBuiltinProviderAccounts(ctx, now.UTC())
}

// Accounts returns active provider account metadata.
func (s *AccountService) Accounts(ctx context.Context) ([]Account, error) {
	return s.persistence.ActiveProviderAccounts(ctx)
}

// LoadAccount returns one provider account by exact identifier.
func (s *AccountService) LoadAccount(ctx context.Context, id string) (Account, error) {
	return s.persistence.ProviderAccount(ctx, id)
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
		// A persistence adapter may report an error after committing its write.
		// Probe the durable boundary before compensating so a failed transaction
		// cannot leave an account row without its protected credential.
		persisted, lookupErr := s.persistence.ProviderAccount(ctx, account.ID)
		if lookupErr == nil && persisted.ID == account.ID {
			compensated := compensateFailedCreateTransaction(
				func() error { return restorePrivateFile(path, snapshot) },
				func() (bool, error) { return s.persistence.DeleteProviderAccount(ctx, account.ID) },
			)
			if !compensated {
				return Account{}, ErrCompensationFailed
			}
		} else if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	return created, nil
}

// compensateFailedCreateTransaction restores credential bytes before removing
// a durable account row. It is shared by account creation and its failure
// contract so a failed rollback never erases durable evidence first.
func compensateFailedCreateTransaction(rollback func() error, deleteDurable func() (bool, error)) bool {
	if rollback == nil || deleteDurable == nil {
		return false
	}
	if err := rollback(); err != nil {
		return false
	}
	deleted, err := deleteDurable()
	return err == nil && deleted
}

// CreateSecretAccountRequest applies one validated account-operation request.
// Callers that accept pathless API input should use this boundary so secret
// validation and account persistence share the same production flow.
func (s *AccountService) CreateSecretAccountRequest(
	ctx context.Context,
	request CreateSecretProviderAccountRequest,
	now time.Time,
) (Account, error) {
	name := ""
	if request.DisplayName != nil {
		name = *request.DisplayName
	}
	return s.CreateSecretAccount(ctx, request.ProviderKind, name, request.secret, now)
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
	if id == openAIDefaultAccountID {
		if _, err := s.ensureOpenAIAccount(ctx, now); err != nil {
			return Account{}, err
		}
	}
	return s.mutateSecret(ctx, id, 0, AuthSecretInput, true, nil, now, func(path string) error {
		return writeSecret(path, secret)
	})
}

// SaveSecretRequest applies one validated secret-replacement request.
func (s *AccountService) SaveSecretRequest(
	ctx context.Context,
	request SaveProviderAccountSecretRequest,
	now time.Time,
) (Account, error) {
	return s.SaveSecret(ctx, request.ProviderAccountID, request.secret, now)
}

// ImportOpenAISecret stores a startup credential only when OpenAI has no credential.
func (s *AccountService) ImportOpenAISecret(
	ctx context.Context,
	secret Secret,
	organizationID string,
	projectID string,
	now time.Time,
) (Account, error) {
	account, err := s.ensureOpenAIAccount(ctx, now)
	if err != nil {
		return Account{}, err
	}
	if account.Metadata.SecretConfigured() {
		return account, nil
	}
	metadata := cloneMetadata(account.Metadata)
	for key, value := range map[string]string{
		"organization_id": strings.TrimSpace(organizationID),
		"project_id":      strings.TrimSpace(projectID),
	} {
		if value != "" {
			metadata[key], _ = json.Marshal(value)
		}
	}
	return s.mutateSecret(ctx, account.ID, 0, AuthExternalManual, true, metadata, now, func(path string) error {
		return writeSecret(path, secret)
	})
}

func (s *AccountService) ensureOpenAIAccount(ctx context.Context, now time.Time) (Account, error) {
	gate := s.gate(openAIDefaultAccountID)
	gate.Lock()
	defer gate.Unlock()
	account, err := s.persistence.ProviderAccount(ctx, openAIDefaultAccountID)
	if errors.Is(err, ErrAccountNotFound) {
		return s.persistence.CreateProviderAccount(ctx, builtinAccount("openai", "OpenAI", AuthExternalManual, now))
	}
	return account, err
}

// ClearSecret atomically removes one account credential and changes its safe metadata.
func (s *AccountService) ClearSecret(ctx context.Context, id string, now time.Time) (Account, error) {
	return s.mutateSecret(ctx, id, 0, "", false, nil, now, home.RemovePrivateFile)
}

// RecordAuthFailure marks one credential revision unauthenticated after a
// provider rejects it. A replacement credential cannot be clobbered.
func (s *AccountService) RecordAuthFailure(
	ctx context.Context,
	id string,
	expectedRevision uint64,
	now time.Time,
) (Account, error) {
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()
	account, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return Account{}, err
	}
	if account.Metadata.CredentialRevision() != expectedRevision {
		return Account{}, ErrAccountConflict
	}
	marker, ok := s.persistence.(authFailurePersistence)
	if !ok {
		return Account{}, errors.New("provider authentication failure persistence is unavailable")
	}
	if err := marker.MarkProviderAuthenticationFailed(ctx, id, expectedRevision, now.UTC()); err != nil {
		return Account{}, err
	}
	updated, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return Account{}, err
	}
	if updated.Metadata.CredentialRevision() != expectedRevision {
		return Account{}, ErrAccountConflict
	}
	return updated, nil
}

// PublishVerifiedSecret saves a remotely verified credential against its starting revision.
func (s *AccountService) PublishVerifiedSecret(
	ctx context.Context,
	id string,
	expectedRevision uint64,
	method AuthMethod,
	secret Secret,
	profiles []ModelProfile,
	now time.Time,
) (Account, error) {
	if id != "provider_account:openrouter:default" || secret.value == "" ||
		(method != AuthOAuthPKCE && method != AuthSecretInput) || len(profiles) == 0 {
		return Account{}, ErrAuthMethodMismatch
	}
	metadata, err := metadataWithProfiles(nil, profiles, now)
	if err != nil {
		return Account{}, err
	}
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()

	account, err := s.persistence.ProviderAccount(ctx, id)
	if errors.Is(err, ErrAccountNotFound) {
		if expectedRevision != 0 {
			return Account{}, ErrAccountConflict
		}
		return s.createVerifiedOpenRouter(ctx, method, secret, metadata, now)
	}
	if err != nil {
		return Account{}, err
	}
	if account.ProviderKind != "openrouter" || account.AccountKey != "default" ||
		account.Metadata.CredentialRevision() != expectedRevision {
		return Account{}, ErrAccountConflict
	}
	metadata, err = metadataWithProfiles(account.Metadata, profiles, now)
	if err != nil {
		return Account{}, err
	}
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
	updated, err := s.persistence.UpdateProviderCredential(
		ctx, id, expectedRevision, method, true, metadata, now.UTC(),
	)
	if err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	return updated, nil
}

// publishCodexTokens saves verified Codex tokens and models against their starting revision.
func (s *AccountService) publishCodexTokens(
	ctx context.Context,
	expectedRevision uint64,
	tokens CodexTokens,
	catalog codexModelCatalog,
	now time.Time,
) (Account, error) {
	if !tokens.valid() || len(catalog.Profiles) == 0 || !validCodexClientVersion(catalog.ClientVersion) {
		return Account{}, ErrAuthMethodMismatch
	}
	const id = "provider_account:codex:default"
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()

	account, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return Account{}, err
	}
	if account.ProviderKind != "codex" || account.AccountKey != "default" || !account.IsActive ||
		account.Metadata.CredentialRevision() != expectedRevision {
		return Account{}, ErrAccountConflict
	}
	metadata, err := codexMetadataWithCatalog(account.Metadata, catalog, now)
	if err != nil {
		return Account{}, err
	}
	path, err := s.codexTokenPath(account)
	if err != nil {
		return Account{}, err
	}
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		return Account{}, err
	}
	if err := writeCodexTokens(path, tokens); err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	updated, err := s.persistence.UpdateProviderCredential(
		ctx, id, expectedRevision, AuthOAuthDeviceCode, true, metadata, now.UTC(),
	)
	if err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	return updated, nil
}

// LoadCodexTokens returns the protected Codex token set.
func (s *AccountService) LoadCodexTokens(ctx context.Context) (CodexTokens, error) {
	_, tokens, err := s.loadCodexTokenSnapshot(ctx)
	return tokens, err
}

// RefreshCodexTokens refreshes a Codex token that is close to expiry or was rejected.
// It does not hold the account gate while the remote request is in progress.
func (s *AccountService) RefreshCodexTokens(
	ctx context.Context,
	forcedAccessDigest *codexAccessTokenDigest,
	now time.Time,
	refresh func(context.Context, CodexTokens) (CodexTokens, error),
) (CodexTokens, error) {
	if refresh == nil {
		return CodexTokens{}, errors.New("Codex token refresh is unavailable")
	}
	select {
	case <-ctx.Done():
		return CodexTokens{}, ctx.Err()
	case <-s.codexRefreshGate:
	}
	defer func() { s.codexRefreshGate <- struct{}{} }()

	account, tokens, err := s.loadCodexTokenSnapshot(ctx)
	if err != nil {
		return CodexTokens{}, err
	}
	var startingDigest codexAccessTokenDigest
	var refreshNeeded bool
	err = tokens.Use(func(accessToken string, _ string, _ uint64) error {
		startingDigest = codexAccessTokenDigestFor(accessToken)
		if forcedAccessDigest != nil {
			refreshNeeded = startingDigest == *forcedAccessDigest
			return nil
		}
		refreshNeeded = codexTokenNeedsRefresh(accessToken, now)
		return nil
	})
	if err != nil {
		return CodexTokens{}, err
	}
	if !refreshNeeded {
		return tokens, nil
	}

	refreshed, err := refresh(ctx, tokens)
	if err != nil {
		return CodexTokens{}, err
	}
	if err := refreshed.Use(func(string, string, uint64) error { return nil }); err != nil {
		return CodexTokens{}, errors.New("Codex token refresh is invalid")
	}
	refreshed.lastRefresh = uint64(now.UTC().Unix())
	return s.publishRefreshedCodexTokens(
		ctx, account, startingDigest, refreshed, now,
	)
}

func (s *AccountService) loadCodexTokenSnapshot(ctx context.Context) (Account, CodexTokens, error) {
	const id = "provider_account:codex:default"
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()
	account, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return Account{}, CodexTokens{}, err
	}
	if !validCodexTokenAccount(account) {
		return Account{}, CodexTokens{}, ErrAccountConflict
	}
	path, err := s.codexTokenPath(account)
	if err != nil {
		return Account{}, CodexTokens{}, err
	}
	data, err := home.ReadPrivateFile(path, credentialFileLimit)
	if err != nil {
		return Account{}, CodexTokens{}, errors.New("Codex tokens are unavailable")
	}
	var file codexTokenFile
	if err := decodeProtectedJSON(data, &file); err != nil {
		return Account{}, CodexTokens{}, errors.New("Codex tokens are unavailable")
	}
	tokens := file.tokens()
	if !tokens.valid() {
		return Account{}, CodexTokens{}, errors.New("Codex tokens are unavailable")
	}
	return account, tokens, nil
}

func (s *AccountService) publishRefreshedCodexTokens(
	ctx context.Context,
	starting Account,
	startingAccessDigest codexAccessTokenDigest,
	tokens CodexTokens,
	now time.Time,
) (CodexTokens, error) {
	const id = "provider_account:codex:default"
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()

	account, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return CodexTokens{}, err
	}
	if !validCodexTokenAccount(account) || account.Metadata.CredentialRevision() != starting.Metadata.CredentialRevision() {
		return CodexTokens{}, ErrAccountConflict
	}
	path, err := s.codexTokenPath(account)
	if err != nil {
		return CodexTokens{}, err
	}
	snapshot, err := snapshotPrivateFile(path)
	if err != nil {
		return CodexTokens{}, err
	}
	var current codexTokenFile
	if err := decodeProtectedJSON(snapshot.data, &current); err != nil {
		return CodexTokens{}, ErrAccountConflict
	}
	currentTokens := current.tokens()
	var currentAccessDigest codexAccessTokenDigest
	if err := currentTokens.Use(func(accessToken string, _ string, _ uint64) error {
		currentAccessDigest = codexAccessTokenDigestFor(accessToken)
		return nil
	}); err != nil || currentAccessDigest != startingAccessDigest {
		return CodexTokens{}, ErrAccountConflict
	}
	if err := writeCodexTokens(path, tokens); err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return CodexTokens{}, ErrCompensationFailed
		}
		return CodexTokens{}, err
	}
	if _, err := s.persistence.UpdateProviderCredential(
		ctx, id, starting.Metadata.CredentialRevision(), AuthOAuthDeviceCode, true, account.Metadata, now.UTC(),
	); err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return CodexTokens{}, ErrCompensationFailed
		}
		return CodexTokens{}, err
	}
	return tokens, nil
}

type codexAccessTokenDigest [sha256.Size]byte

func codexAccessTokenDigestFor(accessToken string) codexAccessTokenDigest {
	return sha256.Sum256([]byte(accessToken))
}

func validCodexTokenAccount(account Account) bool {
	return account.ID == "provider_account:codex:default" && account.ProviderKind == "codex" &&
		account.AccountKey == "default" && account.AuthMethod == AuthOAuthDeviceCode && account.IsActive
}

func codexTokenNeedsRefresh(accessToken string, now time.Time) bool {
	parts := strings.Split(accessToken, ".")
	if len(parts) != 3 {
		return false
	}
	payload, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return false
	}
	var claims struct {
		ExpiresAt json.Number `json:"exp"`
	}
	decoder := json.NewDecoder(bytes.NewReader(payload))
	decoder.UseNumber()
	if decoder.Decode(&claims) != nil {
		return false
	}
	expiresAt, err := claims.ExpiresAt.Int64()
	if err != nil || expiresAt <= 0 {
		return false
	}
	return time.Unix(expiresAt, 0).UTC().Sub(now.UTC()) <= 120*time.Second
}

// LoadSecret returns one credential through a typed wrapper.
func (s *AccountService) LoadSecret(ctx context.Context, id string) (Secret, error) {
	return s.loadSecret(ctx, id, nil)
}

// LoadSecretAtRevision returns the credential only when its revision is current.
func (s *AccountService) LoadSecretAtRevision(ctx context.Context, id string, expectedRevision uint64) (Secret, error) {
	return s.loadSecret(ctx, id, &expectedRevision)
}

func (s *AccountService) loadSecret(ctx context.Context, id string, expectedRevision *uint64) (Secret, error) {
	gate := s.gate(id)
	gate.Lock()
	defer gate.Unlock()
	account, err := s.persistence.ProviderAccount(ctx, id)
	if err != nil {
		return Secret{}, err
	}
	if expectedRevision != nil && account.Metadata.CredentialRevision() != *expectedRevision {
		return Secret{}, ErrAccountConflict
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
	if IsBuiltinAccountID(id) {
		return false, ErrProtectedAccount
	}
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
	expectedRevision uint64,
	method AuthMethod,
	configured bool,
	metadata AccountMetadata,
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
	openAIAccount := isExternalOpenAIAccount(account)
	if (!ok || !supportsAuth(entry, AuthSecretInput)) && !openAIAccount {
		return Account{}, ErrAuthMethodMismatch
	}
	if openAIAccount {
		method = AuthExternalManual
		if configured {
			if metadata == nil {
				metadata = account.Metadata
			}
			metadata, err = metadataWithProfiles(metadata, openAIModelProfiles(), now)
			if err != nil {
				return Account{}, err
			}
		}
	}
	if method == "" {
		method = account.AuthMethod
	}
	if metadata == nil {
		metadata = cloneMetadata(account.Metadata)
	}
	if expectedRevision == 0 {
		expectedRevision = account.Metadata.CredentialRevision()
	} else if account.Metadata.CredentialRevision() != expectedRevision {
		return Account{}, ErrAccountConflict
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
		ctx, id, expectedRevision, method, configured, metadata, now.UTC(),
	)
	if err != nil {
		if restoreErr := restorePrivateFile(path, snapshot); restoreErr != nil {
			return Account{}, ErrCompensationFailed
		}
		return Account{}, err
	}
	return updated, nil
}

func (s *AccountService) createVerifiedOpenRouter(
	ctx context.Context,
	method AuthMethod,
	secret Secret,
	metadata AccountMetadata,
	now time.Time,
) (Account, error) {
	now = now.UTC()
	metadata = cloneMetadata(metadata)
	metadata["credentialRevision"] = json.RawMessage("1")
	metadata["secretConfigured"] = json.RawMessage("true")
	account := Account{
		ID: "provider_account:openrouter:default", ProviderKind: "openrouter", AccountKey: "default",
		DisplayName: "OpenRouter", AuthMethod: method, IsActive: true, IsDefault: true,
		Status: StatusAuthenticated, Metadata: metadata, CreatedAt: now, UpdatedAt: now,
		LastCheckedAt: &now, LastAuthenticatedAt: &now,
	}
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

func cloneMetadata(source AccountMetadata) AccountMetadata {
	result := make(AccountMetadata, len(source)+2)
	for key, value := range source {
		result[key] = append(json.RawMessage(nil), value...)
	}
	return result
}

func (s *AccountService) gate(id string) *sync.Mutex {
	if s.runtime == nil {
		s.runtime = newProviderRuntime()
	}
	return s.runtime.accountGate(id)
}

func (s *AccountService) admitGeneration(ctx context.Context, priority int) (func(), error) {
	if s == nil || s.runtime == nil {
		return func() {}, nil
	}
	return s.runtime.admitGeneration(ctx, priority)
}

func (s *AccountService) credentialPath(account Account) (string, error) {
	if err := validateAccount(account); err != nil {
		return "", err
	}
	return filepath.Join(s.root, "providers", account.ProviderKind, account.AccountKey, "api_key.json"), nil
}

func (s *AccountService) codexTokenPath(account Account) (string, error) {
	if err := validateAccount(account); err != nil {
		return "", err
	}
	if account.ProviderKind != "codex" || account.AccountKey != "default" {
		return "", ErrAuthMethodMismatch
	}
	return filepath.Join(s.root, "providers", "codex", "default", "codex_tokens.json"), nil
}

type credentialFile struct {
	APIKey string `json:"api_key"`
}

func writeSecret(path string, secret Secret) error {
	data, err := json.Marshal(credentialFile{APIKey: secret.value})
	if err != nil {
		return errors.New("encode provider secret")
	}
	if len(data) > credentialFileLimit {
		return errors.New("provider secret exceeds the protected file limit")
	}
	if err := home.AtomicWritePrivate(path, data); err != nil {
		return fmt.Errorf("write provider secret: %w", err)
	}
	return nil
}

func writeCodexTokens(path string, tokens CodexTokens) error {
	data, err := json.Marshal(codexTokenFile{
		AccessToken: tokens.accessToken, RefreshToken: tokens.refreshToken, LastRefresh: tokens.lastRefresh,
	})
	if err != nil {
		return errors.New("encode Codex tokens")
	}
	if len(data) > credentialFileLimit {
		return errors.New("Codex tokens exceed the protected file limit")
	}
	if err := home.AtomicWritePrivate(path, data); err != nil {
		return fmt.Errorf("write Codex tokens: %w", err)
	}
	return nil
}

func decodeCredentialFile(data []byte, destination *credentialFile) error {
	return decodeProtectedJSON(data, destination)
}

func decodeProtectedJSON(data []byte, destination any) error {
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

// String prevents a temporary credential snapshot from exposing its bytes in
// diagnostics while retaining whether a prior file existed.
func (s fileSnapshot) String() string {
	if !s.exists {
		return "provider.fileSnapshot{exists:false}"
	}
	return "provider.fileSnapshot{exists:true,data:[REDACTED]}"
}

// GoString prevents %#v diagnostics from exposing temporary credential bytes.
func (s fileSnapshot) GoString() string { return s.String() }

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
