package provider

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"
)

var (
	// ErrAccountNotFound means the selected provider account does not exist.
	ErrAccountNotFound = errors.New("provider account not found")
	// ErrProtectedAccount means a built-in provider account cannot be deleted.
	ErrProtectedAccount = errors.New("protected provider account")
	// ErrAccountConflict means stored account state changed during an operation.
	ErrAccountConflict = errors.New("provider account conflict")
	// ErrUnsupportedProvider means the provider catalog has no matching entry.
	ErrUnsupportedProvider = errors.New("unsupported provider")
	// ErrAuthMethodMismatch means the provider does not support the requested method.
	ErrAuthMethodMismatch = errors.New("provider authentication method mismatch")
	// ErrCompensationFailed means a failed operation could not restore prior state.
	ErrCompensationFailed = errors.New("provider account compensation failed")
)

// AuthMethod identifies one provider authentication method.
type AuthMethod string

const (
	AuthOAuthDeviceCode AuthMethod = "oauth_device_code"
	AuthOAuthPKCE       AuthMethod = "oauth_pkce"
	AuthSecretInput     AuthMethod = "secret_input"
	AuthExternalManual  AuthMethod = "external_manual"
	AuthNone            AuthMethod = "none"
)

// AccountStatus is the last known provider account readiness state.
type AccountStatus string

const (
	StatusUnknown         AccountStatus = "unknown"
	StatusChecking        AccountStatus = "checking"
	StatusAuthenticated   AccountStatus = "authenticated"
	StatusUnauthenticated AccountStatus = "unauthenticated"
	StatusUnavailable     AccountStatus = "unavailable"
)

// AccountMetadata contains durable non-secret provider metadata.
type AccountMetadata map[string]json.RawMessage

// CredentialRevision returns the stored credential generation.
func (m AccountMetadata) CredentialRevision() uint64 {
	var revision uint64
	_ = json.Unmarshal(m["credentialRevision"], &revision)
	return revision
}

// SecretConfigured reports whether a protected secret file should exist.
func (m AccountMetadata) SecretConfigured() bool {
	var configured bool
	_ = json.Unmarshal(m["secretConfigured"], &configured)
	return configured
}

// ModelProfiles returns safe provider model metadata.
func (m AccountMetadata) ModelProfiles() ([]ModelProfile, error) {
	var profiles []ModelProfile
	value := m["profiles"]
	if len(value) == 0 {
		return []ModelProfile{}, nil
	}
	if err := json.Unmarshal(value, &profiles); err != nil {
		return nil, errors.New("provider model profiles are invalid")
	}
	return profiles, nil
}

func credentialMetadata(revision uint64, configured bool) AccountMetadata {
	revisionJSON, _ := json.Marshal(revision)
	configuredJSON, _ := json.Marshal(configured)
	return AccountMetadata{"credentialRevision": revisionJSON, "secretConfigured": configuredJSON}
}

// Account contains durable metadata that is safe for ordinary storage.
type Account struct {
	ID                  string
	ProviderKind        string
	AccountKey          string
	DisplayName         string
	AuthMethod          AuthMethod
	IsActive            bool
	IsDefault           bool
	Status              AccountStatus
	LastCheckedAt       *time.Time
	LastAuthenticatedAt *time.Time
	LastErrorCode       string
	LastErrorMessage    string
	Metadata            AccountMetadata
	CreatedAt           time.Time
	UpdatedAt           time.Time
}

// CatalogEntry declares one provider account type.
type CatalogEntry struct {
	ProviderKind         string
	DisplayName          string
	PreferredAuthMethod  AuthMethod
	SupportedAuthMethods []AuthMethod
	GeneratesAccountKey  bool
}

var catalog = []CatalogEntry{
	{
		ProviderKind: "codex", DisplayName: "Codex",
		PreferredAuthMethod:  AuthOAuthDeviceCode,
		SupportedAuthMethods: []AuthMethod{AuthOAuthDeviceCode},
	},
	{
		ProviderKind: "openrouter", DisplayName: "OpenRouter",
		PreferredAuthMethod:  AuthOAuthPKCE,
		SupportedAuthMethods: []AuthMethod{AuthOAuthPKCE, AuthSecretInput},
	},
	{
		ProviderKind: "exa", DisplayName: "Exa", PreferredAuthMethod: AuthSecretInput,
		SupportedAuthMethods: []AuthMethod{AuthSecretInput}, GeneratesAccountKey: true,
	},
	{
		ProviderKind: "kernel", DisplayName: "Kernel", PreferredAuthMethod: AuthSecretInput,
		SupportedAuthMethods: []AuthMethod{AuthSecretInput}, GeneratesAccountKey: true,
	},
	{
		ProviderKind: "tinyfish", DisplayName: "TinyFish", PreferredAuthMethod: AuthSecretInput,
		SupportedAuthMethods: []AuthMethod{AuthSecretInput}, GeneratesAccountKey: true,
	},
	{
		ProviderKind: "firecrawl", DisplayName: "Firecrawl", PreferredAuthMethod: AuthSecretInput,
		SupportedAuthMethods: []AuthMethod{AuthSecretInput}, GeneratesAccountKey: true,
	},
}

// Catalog returns independent provider declarations in stable display order.
func Catalog() []CatalogEntry {
	entries := make([]CatalogEntry, len(catalog))
	for index, entry := range catalog {
		entry.SupportedAuthMethods = append([]AuthMethod(nil), entry.SupportedAuthMethods...)
		entries[index] = entry
	}
	return entries
}

// CatalogEntryFor returns one exact provider declaration.
func CatalogEntryFor(providerKind string) (CatalogEntry, bool) {
	for _, entry := range catalog {
		if entry.ProviderKind == providerKind {
			entry.SupportedAuthMethods = append([]AuthMethod(nil), entry.SupportedAuthMethods...)
			return entry, true
		}
	}
	return CatalogEntry{}, false
}

// BuiltinAccounts returns the permanent model-provider account declarations.
func BuiltinAccounts(now time.Time) []Account {
	now = now.UTC()
	return []Account{
		builtinAccount("codex", "Codex", AuthOAuthDeviceCode, now),
		builtinAccount("openai", "OpenAI", AuthExternalManual, now),
		builtinAccount("openrouter", "OpenRouter", AuthOAuthPKCE, now),
	}
}

// InitialBuiltinAccounts returns built-ins created before interactive setup.
func InitialBuiltinAccounts(now time.Time) []Account {
	now = now.UTC()
	accounts := BuiltinAccounts(now)[:2]
	return append(accounts,
		builtinNoAuthAccount("foundation_local", "default", "Apple Foundation Models", StatusUnknown, now),
		builtinNoAuthAccount("local_models", "default", "Local models", StatusUnknown, now),
		builtinNoAuthAccount("duckduckgo_public", "system", "DuckDuckGo public search", StatusAuthenticated, now),
		builtinNoAuthAccount("direct_http", "system", "Direct HTTP web fetch", StatusAuthenticated, now),
		builtinNoAuthAccount("obscura", "system", "Obscura interactive browser", StatusAuthenticated, now),
		builtinNoAuthAccount("firecrawl", "public", "Firecrawl Keyless", StatusAuthenticated, now),
	)
}

func builtinNoAuthAccount(kind string, key string, displayName string, status AccountStatus, now time.Time) Account {
	return Account{
		ID: "provider_account:" + kind + ":" + key, ProviderKind: kind, AccountKey: key,
		DisplayName: displayName, AuthMethod: AuthNone, IsActive: true, IsDefault: true,
		Status: status, Metadata: credentialMetadata(0, false), CreatedAt: now, UpdatedAt: now,
	}
}

func builtinAccount(kind string, displayName string, method AuthMethod, now time.Time) Account {
	return Account{
		ID:           "provider_account:" + kind + ":default",
		ProviderKind: kind,
		AccountKey:   "default",
		DisplayName:  displayName,
		AuthMethod:   method,
		IsActive:     true,
		IsDefault:    true,
		Status:       StatusUnknown,
		Metadata:     credentialMetadata(0, false),
		CreatedAt:    now,
		UpdatedAt:    now,
	}
}

// IsBuiltinAccountID reports whether an account has protected built-in identity.
func IsBuiltinAccountID(id string) bool {
	for _, builtinID := range []string{
		"provider_account:codex:default", "provider_account:openai:default",
		"provider_account:openrouter:default", "provider_account:foundation_local:default",
		"provider_account:local_models:default", "provider_account:duckduckgo_public:system",
		"provider_account:direct_http:system", "provider_account:obscura:system",
		"provider_account:firecrawl:public",
	} {
		if builtinID == id {
			return true
		}
	}
	return false
}

func supportsAuth(entry CatalogEntry, method AuthMethod) bool {
	for _, supported := range entry.SupportedAuthMethods {
		if supported == method {
			return true
		}
	}
	return false
}

func validateAccount(account Account) error {
	entry, ok := CatalogEntryFor(account.ProviderKind)
	if !ok {
		return ErrUnsupportedProvider
	}
	if !supportsAuth(entry, account.AuthMethod) {
		return ErrAuthMethodMismatch
	}
	if account.ID == "" || account.AccountKey == "" || strings.TrimSpace(account.DisplayName) == "" {
		return errors.New("provider account fields cannot be empty")
	}
	if account.ID != fmt.Sprintf("provider_account:%s:%s", account.ProviderKind, account.AccountKey) {
		return errors.New("provider account identity is inconsistent")
	}
	if !safePathPart(account.ProviderKind) || !safePathPart(account.AccountKey) {
		return errors.New("provider account identity is unsafe")
	}
	return nil
}

func safePathPart(value string) bool {
	if value == "" || len(value) > 128 {
		return false
	}
	for _, character := range value {
		if !(character >= 'a' && character <= 'z') &&
			!(character >= '0' && character <= '9') && character != '_' && character != '-' {
			return false
		}
	}
	return true
}

func newAccountIdentity(providerKind string) (string, string, error) {
	buffer := make([]byte, 16)
	if _, err := rand.Read(buffer); err != nil {
		return "", "", fmt.Errorf("create provider account id: %w", err)
	}
	accountKey := "acct_" + hex.EncodeToString(buffer)
	return "provider_account:" + providerKind + ":" + accountKey, accountKey, nil
}
