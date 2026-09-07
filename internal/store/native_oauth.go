package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/go-oauth2/oauth2/v4"

	"github.com/uptrace/bun"
)

const nativeOAuthLegacyRetry = 60

var (
	// ErrOAuthGrant means an authorization code or refresh credential is invalid.
	ErrOAuthGrant = errors.New("native OAuth grant is invalid")
	// ErrOAuthClientRevoked means a retained client identity cannot register again.
	ErrOAuthClientRevoked = errors.New("native OAuth client is revoked")
)

// NativeOAuthClient is retained client metadata without credential material.
type NativeOAuthClient struct {
	ClientID    string
	DisplayName string
	CreatedAt   time.Time
	RevokedAt   *time.Time
}

// NativeOAuthAccess is one active native request authority.
type NativeOAuthAccess struct {
	ClientID  string
	ExpiresAt time.Time
}

// NativeOAuthRetryProof binds one cached response to its direct successor.
type NativeOAuthRetryProof struct {
	AccessHash   [32]byte
	RefreshHash  [32]byte
	IssuedAt     int64
	RequestBound bool
}

// NativeOAuthRefresh describes an active, retryable, invalid, or replayed refresh.
type NativeOAuthRefresh struct {
	State             string
	FamilyID          string
	ClientID          string
	AbsoluteExpiresAt int64
	Sequence          int64
	AccessExpiresAt   int64
	RevokedClientID   string
}

// NativeOAuthRotation contains the next credential pair.
type NativeOAuthRotation struct {
	AccessHash    [32]byte
	RefreshHash   [32]byte
	IssuedAt      int64
	AccessExpires int64
	IdleExpiresAt int64
}

// NewNativeOAuthFamily contains one already-issued native OAuth credential family.
type NewNativeOAuthFamily struct {
	FamilyID          string
	ClientID          string
	AccessHash        [32]byte
	RefreshHash       [32]byte
	IssuedAt          int64
	AccessExpiresAt   int64
	IdleExpiresAt     int64
	AbsoluteExpiresAt int64
}

// SaveNativeOAuthBrowserRequest replaces one browser's resume or consent state.
func (s *Store) SaveNativeOAuthBrowserRequest(
	ctx context.Context,
	sessionHash [32]byte,
	query string,
	csrf *string,
) error {
	if query == "" || len(query) > 8192 || csrf != nil && len(*csrf) != 43 {
		return errors.New("native OAuth browser request is outside its bounds")
	}
	_, err := s.db.ExecContext(ctx, `
INSERT INTO native_oauth_browser_requests (session_hash, query, csrf)
VALUES (?, ?, ?)
ON CONFLICT(session_hash) DO UPDATE SET query = excluded.query, csrf = excluded.csrf`,
		sessionHash[:], query, csrf)
	if err != nil {
		return fmt.Errorf("save native OAuth browser request: %w", err)
	}
	return nil
}

// NativeOAuthBrowserRequest loads one browser request without consuming it.
func (s *Store) NativeOAuthBrowserRequest(
	ctx context.Context,
	sessionHash [32]byte,
) (string, *string, bool, error) {
	var query string
	var csrf sql.NullString
	err := s.db.QueryRowContext(ctx, `
SELECT query, csrf FROM native_oauth_browser_requests WHERE session_hash = ?`, sessionHash[:]).Scan(&query, &csrf)
	if errors.Is(err, sql.ErrNoRows) {
		return "", nil, false, nil
	}
	if err != nil {
		return "", nil, false, fmt.Errorf("load native OAuth browser request: %w", err)
	}
	if csrf.Valid {
		return query, &csrf.String, true, nil
	}
	return query, nil, true, nil
}

// ConsumeNativeOAuthBrowserRequest removes and returns one consent request.
func (s *Store) ConsumeNativeOAuthBrowserRequest(
	ctx context.Context,
	sessionHash [32]byte,
) (string, string, bool, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return "", "", false, fmt.Errorf("begin native OAuth consent: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var query, csrf string
	err = tx.QueryRowContext(ctx, `
SELECT query, csrf FROM native_oauth_browser_requests
WHERE session_hash = ? AND csrf IS NOT NULL`, sessionHash[:]).Scan(&query, &csrf)
	if errors.Is(err, sql.ErrNoRows) {
		return "", "", false, nil
	}
	if err != nil {
		return "", "", false, fmt.Errorf("load native OAuth consent: %w", err)
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM native_oauth_browser_requests WHERE session_hash = ?", sessionHash[:]); err != nil {
		return "", "", false, fmt.Errorf("consume native OAuth consent: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return "", "", false, fmt.Errorf("commit native OAuth consent: %w", err)
	}
	return query, csrf, true, nil
}

// InsertNativeOAuthCode retains one digest-only authorization code.
func (s *Store) InsertNativeOAuthCode(
	ctx context.Context,
	codeHash [32]byte,
	clientID string,
	displayName string,
	redirectURI string,
	challenge string,
	now int64,
	expiresAt int64,
) error {
	if clientID == "" || len(clientID) > 128 || displayName == "" || len(displayName) > 128 ||
		redirectURI == "" || len(redirectURI) > 2048 || len(challenge) != 43 || expiresAt <= now {
		return errors.New("native OAuth code is outside its bounds")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin native OAuth code: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if _, err := tx.ExecContext(ctx, "DELETE FROM native_oauth_codes WHERE expires_at <= ?", now); err != nil {
		return fmt.Errorf("expire native OAuth codes: %w", err)
	}
	var revoked sql.NullInt64
	err = tx.QueryRowContext(ctx, "SELECT revoked_at FROM clients WHERE client_id = ?", clientID).Scan(&revoked)
	switch {
	case errors.Is(err, sql.ErrNoRows):
		if _, err := tx.ExecContext(ctx, `
INSERT INTO clients (client_id, display_name, created_at) VALUES (?, ?, ?)`, clientID, displayName, now); err != nil {
			return fmt.Errorf("insert native OAuth client: %w", err)
		}
	case err != nil:
		return fmt.Errorf("inspect native OAuth client: %w", err)
	case revoked.Valid:
		return ErrOAuthClientRevoked
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_codes
    (code_hash, client_id, redirect_uri, pkce_challenge, expires_at, created_at)
VALUES (?, ?, ?, ?, ?, ?)`, codeHash[:], clientID, redirectURI, challenge, expiresAt, now); err != nil {
		return fmt.Errorf("insert native OAuth code: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit native OAuth code: %w", err)
	}
	return nil
}

// ExchangeNativeOAuthCode consumes one exact code and creates its first family.
func (s *Store) ExchangeNativeOAuthCode(
	ctx context.Context,
	codeHash [32]byte,
	clientID string,
	redirectURI string,
	verifier string,
	familyID string,
	accessHash [32]byte,
	refreshHash [32]byte,
	now int64,
) error {
	if len(familyID) != 32 || len(verifier) < 43 || len(verifier) > 128 {
		return ErrOAuthGrant
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin native OAuth exchange: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var storedClient, storedRedirect, storedChallenge string
	var expiresAt int64
	err = tx.QueryRowContext(ctx, `
SELECT client_id, redirect_uri, pkce_challenge, expires_at
FROM native_oauth_codes WHERE code_hash = ?`, codeHash[:]).Scan(
		&storedClient, &storedRedirect, &storedChallenge, &expiresAt,
	)
	if errors.Is(err, sql.ErrNoRows) {
		return ErrOAuthGrant
	}
	if err != nil {
		return fmt.Errorf("load native OAuth code: %w", err)
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM native_oauth_codes WHERE code_hash = ?", codeHash[:]); err != nil {
		return fmt.Errorf("consume native OAuth code: %w", err)
	}
	var clientActive bool
	if err := tx.QueryRowContext(ctx, `
SELECT EXISTS(SELECT 1 FROM clients WHERE client_id = ? AND revoked_at IS NULL)`, storedClient).Scan(&clientActive); err != nil {
		return fmt.Errorf("inspect native OAuth client: %w", err)
	}
	if storedClient != clientID || storedRedirect != redirectURI ||
		!oauth2.CodeChallengeS256.Validate(storedChallenge, verifier) ||
		expiresAt <= now || !clientActive {
		if err := tx.Commit(); err != nil {
			return fmt.Errorf("commit rejected native OAuth exchange: %w", err)
		}
		return ErrOAuthGrant
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_families
    (family_id, client_id, created_at, last_used_at, idle_expires_at, absolute_expires_at)
VALUES (?, ?, ?, ?, ?, ?)`, familyID, clientID, now, now, now+30*24*60*60, now+180*24*60*60); err != nil {
		return fmt.Errorf("insert native OAuth family: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_refresh_tokens
    (token_hash, family_id, sequence, status, issued_at)
VALUES (?, ?, 0, 'active', ?)`, refreshHash[:], familyID, now); err != nil {
		return fmt.Errorf("insert native OAuth refresh credential: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_access_tokens (token_hash, family_id, issued_at, expires_at)
VALUES (?, ?, ?, ?)`, accessHash[:], familyID, now, now+15*60); err != nil {
		return fmt.Errorf("insert native OAuth access credential: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit native OAuth exchange: %w", err)
	}
	return nil
}

// InsertNativeOAuthFamily stores one issued native OAuth credential family.
func (s *Store) InsertNativeOAuthFamily(ctx context.Context, input NewNativeOAuthFamily) error {
	if len(input.FamilyID) != 32 || input.FamilyID != strings.ToLower(input.FamilyID) ||
		input.ClientID == "" || len(input.ClientID) > 128 ||
		input.AccessExpiresAt <= input.IssuedAt || input.IdleExpiresAt <= input.IssuedAt ||
		input.AbsoluteExpiresAt < input.IdleExpiresAt {
		return ErrOAuthGrant
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin native OAuth family: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var active bool
	if err := tx.QueryRowContext(ctx, `
SELECT EXISTS(SELECT 1 FROM clients WHERE client_id = ? AND revoked_at IS NULL)`, input.ClientID).Scan(&active); err != nil {
		return fmt.Errorf("inspect native OAuth client: %w", err)
	}
	if !active {
		return ErrOAuthGrant
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_families
    (family_id, client_id, created_at, last_used_at, idle_expires_at, absolute_expires_at)
VALUES (?, ?, ?, ?, ?, ?)`, input.FamilyID, input.ClientID, input.IssuedAt, input.IssuedAt,
		input.IdleExpiresAt, input.AbsoluteExpiresAt); err != nil {
		return fmt.Errorf("insert native OAuth family: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_refresh_tokens
    (token_hash, family_id, sequence, status, issued_at)
VALUES (?, ?, 0, 'active', ?)`, input.RefreshHash[:], input.FamilyID, input.IssuedAt); err != nil {
		return fmt.Errorf("insert native OAuth refresh credential: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_access_tokens (token_hash, family_id, issued_at, expires_at)
VALUES (?, ?, ?, ?)`, input.AccessHash[:], input.FamilyID, input.IssuedAt, input.AccessExpiresAt); err != nil {
		return fmt.Errorf("insert native OAuth access credential: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit native OAuth family: %w", err)
	}
	return nil
}

// NativeOAuthRefreshGrant resolves active or replayed refresh authority.
func (s *Store) NativeOAuthRefreshGrant(
	ctx context.Context,
	refreshHash [32]byte,
	proof *NativeOAuthRetryProof,
	now int64,
) (NativeOAuthRefresh, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return NativeOAuthRefresh{}, fmt.Errorf("begin native OAuth refresh: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	row, exists, err := loadNativeRefresh(ctx, tx, refreshHash)
	if err != nil {
		return NativeOAuthRefresh{}, err
	}
	if !exists {
		return NativeOAuthRefresh{State: "invalid"}, tx.Commit()
	}
	if row.revokedAt.Valid || row.idleExpiresAt <= now || row.absoluteExpiresAt <= now {
		if !row.revokedAt.Valid {
			if err := revokeNativeFamily(ctx, tx, row.familyID, now, "expired"); err != nil {
				return NativeOAuthRefresh{}, err
			}
		}
		if err := tx.Commit(); err != nil {
			return NativeOAuthRefresh{}, err
		}
		return NativeOAuthRefresh{
			State: "invalid", FamilyID: row.familyID, RevokedClientID: row.clientID,
		}, nil
	}
	if row.status == "used" {
		if proof != nil {
			expiresAt, retryable, err := nativeRetryable(ctx, tx, row, *proof, now)
			if err != nil {
				return NativeOAuthRefresh{}, err
			}
			if retryable {
				if err := tx.Commit(); err != nil {
					return NativeOAuthRefresh{}, err
				}
				return NativeOAuthRefresh{
					State: "retryable", FamilyID: row.familyID, AccessExpiresAt: expiresAt,
				}, nil
			}
		}
		if err := revokeNativeFamily(ctx, tx, row.familyID, now, "replay"); err != nil {
			return NativeOAuthRefresh{}, err
		}
		if err := tx.Commit(); err != nil {
			return NativeOAuthRefresh{}, err
		}
		return NativeOAuthRefresh{
			State: "replay", FamilyID: row.familyID, RevokedClientID: row.clientID,
		}, nil
	}
	if err := tx.Commit(); err != nil {
		return NativeOAuthRefresh{}, err
	}
	return NativeOAuthRefresh{
		State: "active", FamilyID: row.familyID, ClientID: row.clientID,
		AbsoluteExpiresAt: row.absoluteExpiresAt, Sequence: row.sequence,
	}, nil
}

// RotateNativeOAuthRefresh atomically replaces both native credentials.
func (s *Store) RotateNativeOAuthRefresh(
	ctx context.Context,
	oldHash [32]byte,
	rotation NativeOAuthRotation,
	now int64,
) (NativeOAuthRefresh, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return NativeOAuthRefresh{}, fmt.Errorf("begin native OAuth rotation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	row, exists, err := loadNativeRefresh(ctx, tx, oldHash)
	if err != nil {
		return NativeOAuthRefresh{}, err
	}
	if !exists {
		return NativeOAuthRefresh{State: "invalid"}, tx.Commit()
	}
	if row.status != "active" {
		if err := revokeNativeFamily(ctx, tx, row.familyID, now, "replay"); err != nil {
			return NativeOAuthRefresh{}, err
		}
		if err := tx.Commit(); err != nil {
			return NativeOAuthRefresh{}, err
		}
		return NativeOAuthRefresh{
			State: "replay", FamilyID: row.familyID, RevokedClientID: row.clientID,
		}, nil
	}
	if row.revokedAt.Valid || row.idleExpiresAt <= now || row.absoluteExpiresAt <= now {
		if !row.revokedAt.Valid {
			if err := revokeNativeFamily(ctx, tx, row.familyID, now, "expired"); err != nil {
				return NativeOAuthRefresh{}, err
			}
		}
		if err := tx.Commit(); err != nil {
			return NativeOAuthRefresh{}, err
		}
		return NativeOAuthRefresh{
			State: "invalid", FamilyID: row.familyID, RevokedClientID: row.clientID,
		}, nil
	}
	result, err := tx.ExecContext(ctx, `
UPDATE native_oauth_refresh_tokens SET status = 'used', used_at = ?
WHERE token_hash = ? AND status = 'active'`, now, oldHash[:])
	if err != nil {
		return NativeOAuthRefresh{}, fmt.Errorf("consume native OAuth refresh credential: %w", err)
	}
	if changed, err := result.RowsAffected(); err != nil || changed != 1 {
		return NativeOAuthRefresh{}, ErrOAuthGrant
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_refresh_tokens
    (token_hash, family_id, sequence, status, issued_at)
VALUES (?, ?, ?, 'active', ?)`, rotation.RefreshHash[:], row.familyID, row.sequence+1, rotation.IssuedAt); err != nil {
		return NativeOAuthRefresh{}, fmt.Errorf("insert rotated refresh credential: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_access_tokens (token_hash, family_id, issued_at, expires_at)
VALUES (?, ?, ?, ?)`, rotation.AccessHash[:], row.familyID, rotation.IssuedAt, rotation.AccessExpires); err != nil {
		return NativeOAuthRefresh{}, fmt.Errorf("insert rotated access credential: %w", err)
	}
	idle := rotation.IdleExpiresAt
	if idle > row.absoluteExpiresAt {
		idle = row.absoluteExpiresAt
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE native_oauth_families SET last_used_at = ?, idle_expires_at = ? WHERE family_id = ?`,
		now, idle, row.familyID); err != nil {
		return NativeOAuthRefresh{}, fmt.Errorf("update native OAuth family: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return NativeOAuthRefresh{}, fmt.Errorf("commit native OAuth rotation: %w", err)
	}
	return NativeOAuthRefresh{
		State: "rotated", FamilyID: row.familyID, ClientID: row.clientID,
	}, nil
}

// ActiveNativeOAuthAccess validates one digest-only bearer credential.
func (s *Store) ActiveNativeOAuthAccess(ctx context.Context, hash [32]byte, now int64) (NativeOAuthAccess, bool, error) {
	var access NativeOAuthAccess
	var expiresAt int64
	err := s.db.QueryRowContext(ctx, `
SELECT family.client_id, min(access.expires_at, family.idle_expires_at, family.absolute_expires_at)
FROM native_oauth_access_tokens AS access
JOIN native_oauth_families AS family ON family.family_id = access.family_id
JOIN clients ON clients.client_id = family.client_id
WHERE access.token_hash = ? AND access.revoked_at IS NULL AND access.expires_at > ?
  AND family.revoked_at IS NULL AND family.idle_expires_at > ? AND family.absolute_expires_at > ?
  AND clients.revoked_at IS NULL`, hash[:], now, now, now).Scan(&access.ClientID, &expiresAt)
	if errors.Is(err, sql.ErrNoRows) {
		return NativeOAuthAccess{}, false, nil
	}
	if err != nil {
		return NativeOAuthAccess{}, false, fmt.Errorf("validate native OAuth access: %w", err)
	}
	access.ExpiresAt = time.Unix(expiresAt, 0).UTC()
	return access, true, nil
}

// RevokeNativeOAuthFamily revokes the family selected by one refresh credential.
func (s *Store) RevokeNativeOAuthFamily(ctx context.Context, hash [32]byte, now int64) (string, string, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return "", "", fmt.Errorf("begin native OAuth revocation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	row, exists, err := loadNativeRefresh(ctx, tx, hash)
	if err != nil {
		return "", "", err
	}
	if exists {
		if err := revokeNativeFamily(ctx, tx, row.familyID, now, "client"); err != nil {
			return "", "", err
		}
	}
	if err := tx.Commit(); err != nil {
		return "", "", fmt.Errorf("commit native OAuth revocation: %w", err)
	}
	if exists {
		return row.clientID, row.familyID, nil
	}
	return "", "", nil
}

// ExpireNativeOAuthFamilies revokes every idle or absolute expiry.
func (s *Store) ExpireNativeOAuthFamilies(ctx context.Context, now int64) ([]string, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin native OAuth expiry: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	rows, err := tx.QueryContext(ctx, `
SELECT family_id, client_id FROM native_oauth_families
WHERE revoked_at IS NULL AND (idle_expires_at <= ? OR absolute_expires_at <= ?)`, now, now)
	if err != nil {
		return nil, fmt.Errorf("query expired native OAuth families: %w", err)
	}
	type expired struct{ familyID, clientID string }
	values := make([]expired, 0)
	for rows.Next() {
		var value expired
		if err := rows.Scan(&value.familyID, &value.clientID); err != nil {
			_ = rows.Close()
			return nil, err
		}
		values = append(values, value)
	}
	if err := rows.Close(); err != nil {
		return nil, err
	}
	clients := make(map[string]struct{})
	for _, value := range values {
		if err := revokeNativeFamily(ctx, tx, value.familyID, now, "expired"); err != nil {
			return nil, err
		}
		clients[value.clientID] = struct{}{}
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit native OAuth expiry: %w", err)
	}
	result := make([]string, 0, len(clients))
	for clientID := range clients {
		result = append(result, clientID)
	}
	return result, nil
}

// NativeOAuthClients lists retained native client identities.
func (s *Store) NativeOAuthClients(ctx context.Context) ([]NativeOAuthClient, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT client_id, display_name, created_at, revoked_at FROM clients ORDER BY created_at, client_id`)
	if err != nil {
		return nil, fmt.Errorf("query native OAuth clients: %w", err)
	}
	defer rows.Close()
	clients := make([]NativeOAuthClient, 0)
	for rows.Next() {
		client, err := scanNativeClient(rows)
		if err != nil {
			return nil, err
		}
		clients = append(clients, client)
	}
	return clients, rows.Err()
}

// RevokeNativeOAuthClient revokes one retained client identity and every family.
func (s *Store) RevokeNativeOAuthClient(ctx context.Context, clientID string, now int64) (NativeOAuthClient, bool, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return NativeOAuthClient{}, false, fmt.Errorf("begin native OAuth client revocation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, "UPDATE clients SET revoked_at = ? WHERE client_id = ? AND revoked_at IS NULL", now, clientID)
	if err != nil {
		return NativeOAuthClient{}, false, err
	}
	changed, _ := result.RowsAffected()
	if changed == 1 {
		if err := revokeNativeClientAuthority(ctx, tx, clientID, now, "client"); err != nil {
			return NativeOAuthClient{}, false, err
		}
	}
	client, exists, err := nativeClient(ctx, tx, clientID)
	if err != nil || !exists {
		return NativeOAuthClient{}, false, err
	}
	if err := tx.Commit(); err != nil {
		return NativeOAuthClient{}, false, err
	}
	return client, changed == 1, nil
}

// RevokeAllNativeOAuthClients revokes every active native client.
func (s *Store) RevokeAllNativeOAuthClients(ctx context.Context, now int64) ([]string, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin global native OAuth revocation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	rows, err := tx.QueryContext(ctx, "SELECT client_id FROM clients WHERE revoked_at IS NULL")
	if err != nil {
		return nil, err
	}
	clientIDs := make([]string, 0)
	for rows.Next() {
		var clientID string
		if err := rows.Scan(&clientID); err != nil {
			_ = rows.Close()
			return nil, err
		}
		clientIDs = append(clientIDs, clientID)
	}
	if err := rows.Close(); err != nil {
		return nil, err
	}
	for _, clientID := range clientIDs {
		if _, err := tx.ExecContext(ctx, "UPDATE clients SET revoked_at = ? WHERE client_id = ?", now, clientID); err != nil {
			return nil, err
		}
		if err := revokeNativeClientAuthority(ctx, tx, clientID, now, "global"); err != nil {
			return nil, err
		}
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit global native OAuth revocation: %w", err)
	}
	return clientIDs, nil
}

type nativeRefreshRow struct {
	familyID, clientID               string
	absoluteExpiresAt, idleExpiresAt int64
	revokedAt                        sql.NullInt64
	sequence                         int64
	status                           string
	usedAt                           sql.NullInt64
}

func loadNativeRefresh(ctx context.Context, tx bun.Tx, hash [32]byte) (nativeRefreshRow, bool, error) {
	var row nativeRefreshRow
	err := tx.QueryRowContext(ctx, `
SELECT family.family_id, family.client_id, family.absolute_expires_at,
       family.idle_expires_at, family.revoked_at, refresh.sequence, refresh.status, refresh.used_at
FROM native_oauth_refresh_tokens AS refresh
JOIN native_oauth_families AS family ON family.family_id = refresh.family_id
WHERE refresh.token_hash = ?`, hash[:]).Scan(
		&row.familyID, &row.clientID, &row.absoluteExpiresAt, &row.idleExpiresAt,
		&row.revokedAt, &row.sequence, &row.status, &row.usedAt,
	)
	if errors.Is(err, sql.ErrNoRows) {
		return nativeRefreshRow{}, false, nil
	}
	if err != nil {
		return nativeRefreshRow{}, false, fmt.Errorf("load native OAuth refresh credential: %w", err)
	}
	return row, true, nil
}

func nativeRetryable(
	ctx context.Context,
	tx bun.Tx,
	used nativeRefreshRow,
	proof NativeOAuthRetryProof,
	now int64,
) (int64, bool, error) {
	if !used.usedAt.Valid || now < used.usedAt.Int64 ||
		(!proof.RequestBound && now-used.usedAt.Int64 > nativeOAuthLegacyRetry) {
		return 0, false, nil
	}
	var expiresAt int64
	err := tx.QueryRowContext(ctx, `
SELECT access.expires_at
FROM native_oauth_refresh_tokens AS active
JOIN native_oauth_access_tokens AS access
  ON access.family_id = active.family_id AND access.issued_at = active.issued_at
WHERE active.family_id = ? AND active.sequence = ? AND active.status = 'active'
  AND active.token_hash = ? AND active.issued_at = ?
  AND access.token_hash = ? AND access.revoked_at IS NULL`,
		used.familyID, used.sequence+1, proof.RefreshHash[:], proof.IssuedAt, proof.AccessHash[:]).Scan(&expiresAt)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, false, nil
	}
	if err != nil {
		return 0, false, err
	}
	return expiresAt, proof.RequestBound || expiresAt > now, nil
}

func revokeNativeFamily(ctx context.Context, tx bun.Tx, familyID string, now int64, reason string) error {
	if _, err := tx.ExecContext(ctx, `
UPDATE native_oauth_families SET revoked_at = ?, revoke_reason = ?
WHERE family_id = ? AND revoked_at IS NULL`, now, reason, familyID); err != nil {
		return fmt.Errorf("revoke native OAuth family: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE native_oauth_access_tokens SET revoked_at = ?
WHERE family_id = ? AND revoked_at IS NULL`, now, familyID); err != nil {
		return fmt.Errorf("revoke native OAuth access credentials: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM client_notification_registrations
WHERE client_id = (SELECT client_id FROM native_oauth_families WHERE family_id = ?)`, familyID); err != nil {
		return fmt.Errorf("remove native notification registration: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM client_live_activity_registrations
WHERE client_id = (SELECT client_id FROM native_oauth_families WHERE family_id = ?)`, familyID); err != nil {
		return fmt.Errorf("remove Live Activity registration: %w", err)
	}
	return nil
}

func revokeNativeClientAuthority(ctx context.Context, tx bun.Tx, clientID string, now int64, reason string) error {
	if _, err := tx.ExecContext(ctx, `
UPDATE native_oauth_families SET revoked_at = ?, revoke_reason = ?
WHERE client_id = ? AND revoked_at IS NULL`, now, reason, clientID); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE native_oauth_access_tokens SET revoked_at = ?
WHERE family_id IN (SELECT family_id FROM native_oauth_families WHERE client_id = ?)
  AND revoked_at IS NULL`, now, clientID); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM client_notification_registrations WHERE client_id = ?", clientID); err != nil {
		return err
	}
	_, err := tx.ExecContext(ctx, "DELETE FROM client_live_activity_registrations WHERE client_id = ?", clientID)
	return err
}

func scanNativeClient(row rowScanner) (NativeOAuthClient, error) {
	var client NativeOAuthClient
	var created int64
	var revoked sql.NullInt64
	if err := row.Scan(&client.ClientID, &client.DisplayName, &created, &revoked); err != nil {
		return NativeOAuthClient{}, err
	}
	client.CreatedAt = time.Unix(created, 0).UTC()
	if revoked.Valid {
		value := time.Unix(revoked.Int64, 0).UTC()
		client.RevokedAt = &value
	}
	return client, nil
}

func nativeClient(ctx context.Context, tx bun.Tx, clientID string) (NativeOAuthClient, bool, error) {
	client, err := scanNativeClient(tx.QueryRowContext(ctx, `
SELECT client_id, display_name, created_at, revoked_at FROM clients WHERE client_id = ?`, clientID))
	if errors.Is(err, sql.ErrNoRows) {
		return NativeOAuthClient{}, false, nil
	}
	return client, err == nil, err
}
