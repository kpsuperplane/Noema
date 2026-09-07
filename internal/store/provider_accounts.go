package store

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
)

const providerAccountSelect = `
SELECT provider_account_id, provider_kind, account_key, display_name, auth_method,
       is_active, is_default, status, last_checked_at_ms, last_authenticated_at_ms,
       COALESCE(last_error_code, ''), COALESCE(last_error_message, ''), metadata_json,
       created_at_ms, updated_at_ms
FROM provider_accounts`

// EnsureBuiltinProviderAccounts creates missing permanent provider accounts.
func (s *Store) EnsureBuiltinProviderAccounts(ctx context.Context, now time.Time) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin built-in provider account setup: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	for _, account := range provider.InitialBuiltinAccounts(now) {
		metadata, err := json.Marshal(account.Metadata)
		if err != nil {
			return fmt.Errorf("encode built-in provider metadata: %w", err)
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO provider_accounts (
    provider_account_id, provider_kind, account_key, display_name, auth_method,
    is_active, is_default, status, metadata_json, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, 1, 1, ?, ?, ?, ?)
ON CONFLICT(provider_account_id) DO NOTHING`,
			account.ID, account.ProviderKind, account.AccountKey, account.DisplayName,
			account.AuthMethod, account.Status, string(metadata), millis(account.CreatedAt),
			millis(account.UpdatedAt)); err != nil {
			return fmt.Errorf("store built-in provider account: %w", err)
		}
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit built-in provider account setup: %w", err)
	}
	return nil
}

// ProviderAccount returns one provider account by exact identifier.
func (s *Store) ProviderAccount(ctx context.Context, id string) (provider.Account, error) {
	return scanProviderAccount(s.db.QueryRowContext(ctx, providerAccountSelect+`
WHERE provider_account_id = ?`, id))
}

// ActiveProviderAccounts returns active accounts in stable display order.
func (s *Store) ActiveProviderAccounts(ctx context.Context) ([]provider.Account, error) {
	rows, err := s.db.QueryContext(ctx, providerAccountSelect+`
WHERE is_active = 1 ORDER BY provider_kind, display_name, account_key`)
	if err != nil {
		return nil, fmt.Errorf("query provider accounts: %w", err)
	}
	defer rows.Close()

	accounts := make([]provider.Account, 0)
	for rows.Next() {
		account, err := scanProviderAccount(rows)
		if err != nil {
			return nil, err
		}
		accounts = append(accounts, account)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read provider accounts: %w", err)
	}
	return accounts, nil
}

// CreateProviderAccount stores one user-managed provider account.
func (s *Store) CreateProviderAccount(ctx context.Context, account provider.Account) (provider.Account, error) {
	if err := validateProviderAccount(account); err != nil {
		return provider.Account{}, err
	}
	metadata, err := json.Marshal(account.Metadata)
	if err != nil {
		return provider.Account{}, fmt.Errorf("encode provider account metadata: %w", err)
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return provider.Account{}, fmt.Errorf("begin provider account creation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	_, err = tx.ExecContext(ctx, `
INSERT INTO provider_accounts (
    provider_account_id, provider_kind, account_key, display_name, auth_method,
    is_active, is_default, status, last_checked_at_ms, last_authenticated_at_ms,
    last_error_code, last_error_message, metadata_json, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULLIF(?, ''), NULLIF(?, ''), ?, ?, ?)`,
		account.ID, account.ProviderKind, account.AccountKey, strings.TrimSpace(account.DisplayName),
		account.AuthMethod, account.IsActive, account.IsDefault, account.Status,
		nullTimeMillis(account.LastCheckedAt), nullTimeMillis(account.LastAuthenticatedAt),
		account.LastErrorCode, account.LastErrorMessage, string(metadata),
		millis(account.CreatedAt), millis(account.UpdatedAt))
	if err != nil {
		return provider.Account{}, fmt.Errorf("create provider account: %w", err)
	}
	created, err := scanProviderAccount(tx.QueryRowContext(ctx, providerAccountSelect+`
WHERE provider_account_id = ?`, account.ID))
	if err != nil {
		return provider.Account{}, err
	}
	if err := tx.Commit(); err != nil {
		return provider.Account{}, fmt.Errorf("commit provider account creation: %w", err)
	}
	return created, nil
}

// UpdateProviderCredential records one file credential change with a revision check.
func (s *Store) UpdateProviderCredential(
	ctx context.Context,
	id string,
	expectedRevision uint64,
	method provider.AuthMethod,
	configured bool,
	metadata provider.AccountMetadata,
	now time.Time,
) (provider.Account, error) {
	status := provider.StatusUnauthenticated
	if configured {
		status = provider.StatusAuthenticated
	}
	now = now.UTC()
	if metadata == nil {
		current, err := s.ProviderAccount(ctx, id)
		if err != nil {
			return provider.Account{}, err
		}
		metadata = current.Metadata
	}
	metadata = cloneProviderMetadata(metadata)
	revisionJSON, _ := json.Marshal(expectedRevision + 1)
	configuredJSON, _ := json.Marshal(configured)
	metadata["credentialRevision"] = revisionJSON
	metadata["secretConfigured"] = configuredJSON
	encodedMetadata, err := json.Marshal(metadata)
	if err != nil {
		return provider.Account{}, fmt.Errorf("encode provider credential metadata: %w", err)
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return provider.Account{}, fmt.Errorf("begin provider credential update: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `
UPDATE provider_accounts
SET auth_method = ?, status = ?, last_checked_at_ms = ?,
    last_authenticated_at_ms = CASE WHEN ? THEN ? ELSE last_authenticated_at_ms END,
    last_error_code = NULL, last_error_message = NULL,
    metadata_json = ?, updated_at_ms = ?
WHERE provider_account_id = ?
  AND COALESCE(CAST(json_extract(metadata_json, '$.credentialRevision') AS INTEGER), 0) = ?`,
		method, status, millis(now), configured, millis(now), string(encodedMetadata),
		millis(now), id, expectedRevision)
	if err != nil {
		return provider.Account{}, fmt.Errorf("update provider credential: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return provider.Account{}, fmt.Errorf("inspect provider credential update: %w", err)
	}
	if changed != 1 {
		if _, err := scanProviderAccount(tx.QueryRowContext(ctx, providerAccountSelect+`
WHERE provider_account_id = ?`, id)); errors.Is(err, provider.ErrAccountNotFound) {
			return provider.Account{}, err
		} else if err != nil {
			return provider.Account{}, err
		}
		return provider.Account{}, provider.ErrAccountConflict
	}
	updated, err := scanProviderAccount(tx.QueryRowContext(ctx, providerAccountSelect+`
WHERE provider_account_id = ?`, id))
	if err != nil {
		return provider.Account{}, err
	}
	if err := tx.Commit(); err != nil {
		return provider.Account{}, fmt.Errorf("commit provider credential update: %w", err)
	}
	return updated, nil
}

// MarkProviderAuthenticationFailed records a remote rejection only for the attempted credential revision.
func (s *Store) MarkProviderAuthenticationFailed(ctx context.Context, id string, revision uint64, now time.Time) error {
	_, err := s.db.ExecContext(ctx, `UPDATE provider_accounts SET status='unauthenticated',
last_checked_at_ms=?,last_error_code='auth_failed',last_error_message='Provider rejected the configured credentials',updated_at_ms=?
WHERE provider_account_id=? AND COALESCE(CAST(json_extract(metadata_json,'$.credentialRevision') AS INTEGER),0)=?`,
		millis(now.UTC()), millis(now.UTC()), id, revision)
	return err
}

// SetProviderAccountStatus records an externally observed readiness state
// without changing the protected credential or its revision.
func (s *Store) SetProviderAccountStatus(
	ctx context.Context,
	id string,
	status provider.AccountStatus,
	code, message string,
	now time.Time,
) error {
	switch status {
	case provider.StatusUnknown, provider.StatusChecking, provider.StatusAuthenticated,
		provider.StatusUnauthenticated, provider.StatusUnavailable:
	default:
		return errors.New("provider account status is unsupported")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE provider_accounts
SET status=?, last_checked_at_ms=?,
    last_authenticated_at_ms=CASE WHEN ?='authenticated' THEN ? ELSE last_authenticated_at_ms END,
    last_error_code=NULLIF(?,''), last_error_message=NULLIF(?,''), updated_at_ms=?
WHERE provider_account_id=?`, status, millis(now.UTC()), status, millis(now.UTC()), code, message, millis(now.UTC()), id)
	if err != nil {
		return fmt.Errorf("set provider account status: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("inspect provider account status: %w", err)
	}
	if changed != 1 {
		return provider.ErrAccountNotFound
	}
	return nil
}

func cloneProviderMetadata(source provider.AccountMetadata) provider.AccountMetadata {
	result := make(provider.AccountMetadata, len(source)+2)
	for key, value := range source {
		result[key] = append(json.RawMessage(nil), value...)
	}
	return result
}

// DeleteProviderAccount hard-deletes one user-managed provider account.
func (s *Store) DeleteProviderAccount(ctx context.Context, id string) (bool, error) {
	if provider.IsBuiltinAccountID(id) {
		return false, provider.ErrProtectedAccount
	}
	result, err := s.db.ExecContext(ctx, "DELETE FROM provider_accounts WHERE provider_account_id = ?", id)
	if err != nil {
		return false, fmt.Errorf("delete provider account: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return false, fmt.Errorf("inspect provider account deletion: %w", err)
	}
	return changed == 1, nil
}

func scanProviderAccount(scanner rowScanner) (provider.Account, error) {
	var account provider.Account
	var checkedAt, authenticatedAt sql.NullInt64
	var metadata []byte
	var createdAt, updatedAt int64
	if err := scanner.Scan(
		&account.ID, &account.ProviderKind, &account.AccountKey, &account.DisplayName,
		&account.AuthMethod, &account.IsActive, &account.IsDefault, &account.Status,
		&checkedAt, &authenticatedAt, &account.LastErrorCode, &account.LastErrorMessage,
		&metadata, &createdAt, &updatedAt,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return provider.Account{}, provider.ErrAccountNotFound
		}
		return provider.Account{}, fmt.Errorf("scan provider account: %w", err)
	}
	if err := decodeProviderMetadata(metadata, &account.Metadata); err != nil {
		return provider.Account{}, err
	}
	account.LastCheckedAt = nullableMillis(checkedAt)
	account.LastAuthenticatedAt = nullableMillis(authenticatedAt)
	account.CreatedAt = fromMillis(createdAt)
	account.UpdatedAt = fromMillis(updatedAt)
	return account, nil
}

func decodeProviderMetadata(data []byte, metadata *provider.AccountMetadata) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := decoder.Decode(metadata); err != nil {
		return fmt.Errorf("decode provider account metadata: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return errors.New("decode provider account metadata: trailing data")
	}
	return nil
}

func validateProviderAccount(account provider.Account) error {
	entry, ok := provider.CatalogEntryFor(account.ProviderKind)
	if !ok {
		return provider.ErrUnsupportedProvider
	}
	supported := false
	for _, method := range entry.SupportedAuthMethods {
		if account.AuthMethod == method {
			supported = true
		}
	}
	if !supported {
		return provider.ErrAuthMethodMismatch
	}
	if account.ID != "provider_account:"+account.ProviderKind+":"+account.AccountKey {
		return errors.New("provider account identity is inconsistent")
	}
	if account.ID == "" || account.AccountKey == "" || strings.TrimSpace(account.DisplayName) == "" {
		return errors.New("provider account fields cannot be empty")
	}
	if account.CreatedAt.IsZero() || account.UpdatedAt.IsZero() {
		return errors.New("provider account timestamps cannot be empty")
	}
	return nil
}

func nullTimeMillis(value *time.Time) any {
	if value == nil {
		return nil
	}
	return millis(value.UTC())
}

func nullableMillis(value sql.NullInt64) *time.Time {
	if !value.Valid {
		return nil
	}
	converted := fromMillis(value.Int64)
	return &converted
}
