package store

import (
	"context"
	"database/sql"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"time"
)

const (
	browserSessionCapacity  = 1024
	browserIdleLifetime     = 24 * time.Hour
	browserAbsoluteLifetime = 30 * 24 * time.Hour
	shortSessionLifetime    = 5 * time.Minute
	recentPasskeyLifetime   = 5 * time.Minute
)

var (
	// ErrSessionFull means the bounded browser session store has no capacity.
	ErrSessionFull = errors.New("browser session capacity reached")
	// ErrSessionUnauthorized means current browser authority is insufficient.
	ErrSessionUnauthorized = errors.New("browser session is not authorized")
	// ErrInitialPasskeyClaimed means another browser completed initial setup.
	ErrInitialPasskeyClaimed = errors.New("initial passkey is already claimed")
	// ErrPasskeyExists means the credential is already registered.
	ErrPasskeyExists = errors.New("passkey is already registered")
	// ErrCredentialChanged means stored WebAuthn state changed during a ceremony.
	ErrCredentialChanged = errors.New("passkey credential changed")
	// ErrPasskeyNotFound means the selected credential does not exist.
	ErrPasskeyNotFound = errors.New("passkey not found")
	// ErrFinalPasskey means normal settings cannot remove the final passkey.
	ErrFinalPasskey = errors.New("final passkey cannot be removed")
)

// BrowserSession is one server-side browser authority record.
type BrowserSession struct {
	State           string
	PasskeyID       string
	RecentPasskeyAt time.Time
	CreatedAt       time.Time
	ExpiresAt       time.Time
}

// HumanPasskey is one stored WebAuthn credential.
type HumanPasskey struct {
	CredentialID   string
	CredentialJSON string
	CreatedAt      time.Time
}

// RegistrationAuthority records the authority checked when registration starts.
type RegistrationAuthority string

const (
	RegistrationInitial RegistrationAuthority = "initial"
	RegistrationCurrent RegistrationAuthority = "current"
	RegistrationSetup   RegistrationAuthority = "setup"
	RegistrationBypass  RegistrationAuthority = "bypass"
)

// HasPasskey reports whether the local human owns any credential.
func (s *Store) HasPasskey(ctx context.Context) (bool, error) {
	var exists bool
	if err := s.db.QueryRowContext(ctx, "SELECT EXISTS(SELECT 1 FROM human_passkeys)").Scan(&exists); err != nil {
		return false, fmt.Errorf("inspect passkey state: %w", err)
	}
	return exists, nil
}

// Passkeys returns all local-human credentials in stable order.
func (s *Store) Passkeys(ctx context.Context) ([]HumanPasskey, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT credential_id, credential_json, created_at_ms
FROM human_passkeys
ORDER BY created_at_ms, credential_id`)
	if err != nil {
		return nil, fmt.Errorf("query passkeys: %w", err)
	}
	defer rows.Close()
	result := make([]HumanPasskey, 0)
	for rows.Next() {
		var passkey HumanPasskey
		var created int64
		if err := rows.Scan(&passkey.CredentialID, &passkey.CredentialJSON, &created); err != nil {
			return nil, fmt.Errorf("scan passkey: %w", err)
		}
		passkey.CreatedAt = fromMillis(created)
		result = append(result, passkey)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read passkeys: %w", err)
	}
	return result, nil
}

// CreateAnonymousSession creates one short-lived ceremony binding.
func (s *Store) CreateAnonymousSession(ctx context.Context, digest [32]byte, now time.Time) error {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin browser session creation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if err := clearExpiredSessions(ctx, tx, now); err != nil {
		return err
	}
	if err := requireSessionCapacity(ctx, tx); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO browser_sessions (session_hash, state, created_at_ms, expires_at_ms)
VALUES (?, 'anonymous', ?, ?)`, digest[:], millis(now), millis(now.Add(shortSessionLifetime))); err != nil {
		return fmt.Errorf("insert browser session: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit browser session creation: %w", err)
	}
	return nil
}

// BrowserSession returns an active session and optionally records human activity.
func (s *Store) BrowserSession(
	ctx context.Context,
	digest [32]byte,
	now time.Time,
	touch bool,
) (BrowserSession, bool, error) {
	now = now.UTC()
	var session BrowserSession
	var passkey sql.NullString
	var recent sql.NullInt64
	var created, expires int64
	err := s.db.QueryRowContext(ctx, `
SELECT state, passkey_id, recent_passkey_at_ms, created_at_ms, expires_at_ms
FROM browser_sessions
WHERE session_hash = ? AND expires_at_ms > ?`, digest[:], millis(now)).Scan(
		&session.State,
		&passkey,
		&recent,
		&created,
		&expires,
	)
	if errors.Is(err, sql.ErrNoRows) {
		_, _ = s.db.ExecContext(ctx, "DELETE FROM browser_sessions WHERE session_hash = ?", digest[:])
		return BrowserSession{}, false, nil
	}
	if err != nil {
		return BrowserSession{}, false, fmt.Errorf("load browser session: %w", err)
	}
	session.PasskeyID = passkey.String
	if recent.Valid {
		session.RecentPasskeyAt = fromMillis(recent.Int64)
	}
	session.CreatedAt = fromMillis(created)
	session.ExpiresAt = fromMillis(expires)
	if touch && session.State == "authenticated" {
		next := now.Add(browserIdleLifetime)
		absolute := session.CreatedAt.Add(browserAbsoluteLifetime)
		if next.After(absolute) {
			next = absolute
		}
		if next.After(session.ExpiresAt) {
			if _, err := s.db.ExecContext(ctx, `
UPDATE browser_sessions SET expires_at_ms = ?
WHERE session_hash = ? AND expires_at_ms > ?`, millis(next), digest[:], millis(now)); err != nil {
				return BrowserSession{}, false, fmt.Errorf("touch browser session: %w", err)
			}
			session.ExpiresAt = next
		}
	}
	return session, true, nil
}

// RotateAnonymousSession gives one browser a fresh short-lived binding.
func (s *Store) RotateAnonymousSession(
	ctx context.Context,
	oldDigest [32]byte,
	newDigest [32]byte,
	now time.Time,
) error {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin anonymous session rotation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if err := clearExpiredSessions(ctx, tx, now); err != nil {
		return err
	}
	result, err := tx.ExecContext(ctx, "DELETE FROM browser_sessions WHERE session_hash = ? AND expires_at_ms > ?", oldDigest[:], millis(now))
	if err != nil {
		return fmt.Errorf("replace anonymous session: %w", err)
	}
	if changed, err := result.RowsAffected(); err != nil || changed != 1 {
		return ErrSessionUnauthorized
	}
	if err := requireSessionCapacity(ctx, tx); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO browser_sessions (session_hash, state, created_at_ms, expires_at_ms)
VALUES (?, 'anonymous', ?, ?)`,
		newDigest[:], millis(now), millis(now.Add(shortSessionLifetime))); err != nil {
		return fmt.Errorf("insert anonymous session: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit anonymous session rotation: %w", err)
	}
	return nil
}

// RegisterPasskey inserts one credential and rotates into an authenticated session.
func (s *Store) RegisterPasskey(
	ctx context.Context,
	credential HumanPasskey,
	authority RegistrationAuthority,
	oldDigest [32]byte,
	newDigest [32]byte,
	now time.Time,
) error {
	if !validCredential(credential) {
		return errors.New("invalid passkey credential")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin passkey registration: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if err := requireRegistrationAuthority(ctx, tx, authority, oldDigest, now); err != nil {
		return err
	}
	pending, err := nativeBrowserRequestForRotation(ctx, tx, oldDigest)
	if err != nil {
		return err
	}
	result, err := tx.ExecContext(ctx, `
INSERT INTO human_passkeys (credential_id, credential_json, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, ?)
ON CONFLICT(credential_id) DO NOTHING`, credential.CredentialID, credential.CredentialJSON, millis(now), millis(now))
	if err != nil {
		return fmt.Errorf("insert passkey: %w", err)
	}
	inserted, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("inspect passkey insert: %w", err)
	}
	if inserted != 1 {
		return ErrPasskeyExists
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM browser_sessions WHERE session_hash = ?", oldDigest[:]); err != nil {
		return fmt.Errorf("rotate browser session: %w", err)
	}
	if err := insertAuthenticatedSession(ctx, tx, newDigest, credential.CredentialID, now); err != nil {
		return err
	}
	if err := restoreNativeBrowserRequest(ctx, tx, newDigest, pending); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit passkey registration: %w", err)
	}
	return nil
}

// AuthenticatePasskey updates one credential and rotates the browser session.
func (s *Store) AuthenticatePasskey(
	ctx context.Context,
	credentialID string,
	expectedJSON string,
	replacementJSON string,
	oldDigest [32]byte,
	newDigest [32]byte,
	now time.Time,
) error {
	if !validCredential(HumanPasskey{CredentialID: credentialID, CredentialJSON: replacementJSON}) {
		return errors.New("invalid passkey credential")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin passkey authentication: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if err := requireBoundSession(ctx, tx, oldDigest, now, ""); err != nil {
		return err
	}
	pending, err := nativeBrowserRequestForRotation(ctx, tx, oldDigest)
	if err != nil {
		return err
	}
	result, err := tx.ExecContext(ctx, `
UPDATE human_passkeys SET credential_json = ?, updated_at_ms = ?
WHERE credential_id = ? AND credential_json = ?`, replacementJSON, millis(now), credentialID, expectedJSON)
	if err != nil {
		return fmt.Errorf("update passkey: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("inspect passkey update: %w", err)
	}
	if changed != 1 {
		return ErrCredentialChanged
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM browser_sessions WHERE session_hash = ?", oldDigest[:]); err != nil {
		return fmt.Errorf("rotate browser session: %w", err)
	}
	if err := insertAuthenticatedSession(ctx, tx, newDigest, credentialID, now); err != nil {
		return err
	}
	if err := restoreNativeBrowserRequest(ctx, tx, newDigest, pending); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit passkey authentication: %w", err)
	}
	return nil
}

// RemovePasskey deletes one non-final credential and its browser sessions.
func (s *Store) RemovePasskey(
	ctx context.Context,
	credentialID string,
	currentDigest [32]byte,
	bypass bool,
	now time.Time,
) ([][32]byte, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin passkey removal: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if !bypass {
		if err := requireCurrentSession(ctx, tx, currentDigest, now, true); err != nil {
			return nil, err
		}
	}
	var exists bool
	if err := tx.QueryRowContext(ctx, "SELECT EXISTS(SELECT 1 FROM human_passkeys WHERE credential_id = ?)", credentialID).Scan(&exists); err != nil {
		return nil, fmt.Errorf("inspect passkey removal: %w", err)
	}
	if !exists {
		return nil, ErrPasskeyNotFound
	}
	var count int
	if err := tx.QueryRowContext(ctx, "SELECT count(*) FROM human_passkeys").Scan(&count); err != nil {
		return nil, fmt.Errorf("count passkeys: %w", err)
	}
	if count <= 1 {
		return nil, ErrFinalPasskey
	}
	hashes, err := sessionHashes(ctx, tx, "SELECT session_hash FROM browser_sessions WHERE passkey_id = ?", credentialID)
	if err != nil {
		return nil, err
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM human_passkeys WHERE credential_id = ?", credentialID); err != nil {
		return nil, fmt.Errorf("delete passkey: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit passkey removal: %w", err)
	}
	return hashes, nil
}

// DeleteBrowserSession revokes one browser authority.
func (s *Store) DeleteBrowserSession(ctx context.Context, digest [32]byte) error {
	if _, err := s.db.ExecContext(ctx, "DELETE FROM browser_sessions WHERE session_hash = ?", digest[:]); err != nil {
		return fmt.Errorf("delete browser session: %w", err)
	}
	return nil
}

// DeleteAllBrowserSessions revokes all browser authority and returns its digests.
func (s *Store) DeleteAllBrowserSessions(ctx context.Context) ([][32]byte, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin browser logout: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	hashes, err := sessionHashes(ctx, tx, "SELECT session_hash FROM browser_sessions")
	if err != nil {
		return nil, err
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM browser_sessions"); err != nil {
		return nil, fmt.Errorf("delete browser sessions: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit browser logout: %w", err)
	}
	return hashes, nil
}

// DeleteExpiredBrowserSessions removes expired authority and returns its digests.
func (s *Store) DeleteExpiredBrowserSessions(ctx context.Context, now time.Time) ([][32]byte, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin session expiry: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	hashes, err := sessionHashes(ctx, tx, "SELECT session_hash FROM browser_sessions WHERE expires_at_ms <= ?", millis(now))
	if err != nil {
		return nil, err
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM browser_sessions WHERE expires_at_ms <= ?", millis(now)); err != nil {
		return nil, fmt.Errorf("delete expired sessions: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit session expiry: %w", err)
	}
	return hashes, nil
}

func requireRegistrationAuthority(
	ctx context.Context,
	tx *sql.Tx,
	authority RegistrationAuthority,
	digest [32]byte,
	now time.Time,
) error {
	switch authority {
	case RegistrationInitial:
		if err := requireBoundSession(ctx, tx, digest, now, ""); err != nil {
			return err
		}
		var exists bool
		if err := tx.QueryRowContext(ctx, "SELECT EXISTS(SELECT 1 FROM human_passkeys)").Scan(&exists); err != nil {
			return fmt.Errorf("inspect initial passkey: %w", err)
		}
		if exists {
			return ErrInitialPasskeyClaimed
		}
	case RegistrationCurrent:
		return requireCurrentSession(ctx, tx, digest, now, true)
	case RegistrationSetup:
		return requireBoundSession(ctx, tx, digest, now, "anonymous")
	case RegistrationBypass:
		return requireBoundSession(ctx, tx, digest, now, "")
	default:
		return ErrSessionUnauthorized
	}
	return nil
}

func requireBoundSession(
	ctx context.Context,
	tx *sql.Tx,
	digest [32]byte,
	now time.Time,
	state string,
) error {
	query := "SELECT EXISTS(SELECT 1 FROM browser_sessions WHERE session_hash = ? AND expires_at_ms > ?"
	arguments := []any{digest[:], millis(now)}
	if state != "" {
		query += " AND state = ?"
		arguments = append(arguments, state)
	}
	query += ")"
	var exists bool
	if err := tx.QueryRowContext(ctx, query, arguments...).Scan(&exists); err != nil {
		return fmt.Errorf("inspect browser session: %w", err)
	}
	if !exists {
		return ErrSessionUnauthorized
	}
	return nil
}

func requireCurrentSession(
	ctx context.Context,
	tx *sql.Tx,
	digest [32]byte,
	now time.Time,
	recent bool,
) error {
	query := `SELECT EXISTS(
    SELECT 1 FROM browser_sessions
    WHERE session_hash = ? AND state = 'authenticated' AND expires_at_ms > ?`
	arguments := []any{digest[:], millis(now)}
	if recent {
		query += " AND recent_passkey_at_ms BETWEEN ? AND ?"
		arguments = append(arguments, millis(now.Add(-recentPasskeyLifetime)))
		arguments = append(arguments, millis(now))
	}
	query += ")"
	var exists bool
	if err := tx.QueryRowContext(ctx, query, arguments...).Scan(&exists); err != nil {
		return fmt.Errorf("inspect browser authority: %w", err)
	}
	if !exists {
		return ErrSessionUnauthorized
	}
	return nil
}

func insertAuthenticatedSession(
	ctx context.Context,
	tx *sql.Tx,
	digest [32]byte,
	credentialID string,
	now time.Time,
) error {
	if err := clearExpiredSessions(ctx, tx, now); err != nil {
		return err
	}
	if err := requireSessionCapacity(ctx, tx); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO browser_sessions (
    session_hash, state, passkey_id, recent_passkey_at_ms,
    created_at_ms, expires_at_ms
) VALUES (?, 'authenticated', ?, ?, ?, ?)`,
		digest[:], credentialID, millis(now), millis(now), millis(now.Add(browserIdleLifetime))); err != nil {
		return fmt.Errorf("insert authenticated session: %w", err)
	}
	return nil
}

func clearExpiredSessions(ctx context.Context, tx *sql.Tx, now time.Time) error {
	if _, err := tx.ExecContext(ctx, "DELETE FROM browser_sessions WHERE expires_at_ms <= ?", millis(now)); err != nil {
		return fmt.Errorf("delete expired browser sessions: %w", err)
	}
	return nil
}

func requireSessionCapacity(ctx context.Context, tx *sql.Tx) error {
	var count int
	if err := tx.QueryRowContext(ctx, "SELECT count(*) FROM browser_sessions").Scan(&count); err != nil {
		return fmt.Errorf("count browser sessions: %w", err)
	}
	if count >= browserSessionCapacity {
		return ErrSessionFull
	}
	return nil
}

func sessionHashes(ctx context.Context, tx *sql.Tx, query string, arguments ...any) ([][32]byte, error) {
	rows, err := tx.QueryContext(ctx, query, arguments...)
	if err != nil {
		return nil, fmt.Errorf("query browser session digests: %w", err)
	}
	defer rows.Close()
	hashes := make([][32]byte, 0)
	for rows.Next() {
		var value []byte
		if err := rows.Scan(&value); err != nil {
			return nil, fmt.Errorf("scan browser session digest: %w", err)
		}
		if len(value) != 32 {
			return nil, errors.New("stored browser session digest has an invalid length")
		}
		var digest [32]byte
		copy(digest[:], value)
		hashes = append(hashes, digest)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read browser session digests: %w", err)
	}
	return hashes, nil
}

func validCredential(credential HumanPasskey) bool {
	decoded, err := base64.RawURLEncoding.DecodeString(credential.CredentialID)
	return err == nil && len(decoded) > 0 && len(decoded) <= 1024 &&
		base64.RawURLEncoding.EncodeToString(decoded) == credential.CredentialID &&
		json.Valid([]byte(credential.CredentialJSON))
}

type nativeBrowserRequest struct {
	query string
	csrf  sql.NullString
	found bool
}

func nativeBrowserRequestForRotation(
	ctx context.Context,
	tx *sql.Tx,
	digest [32]byte,
) (nativeBrowserRequest, error) {
	var value nativeBrowserRequest
	err := tx.QueryRowContext(ctx, `
SELECT query, csrf FROM native_oauth_browser_requests WHERE session_hash = ?`, digest[:]).Scan(
		&value.query, &value.csrf,
	)
	if errors.Is(err, sql.ErrNoRows) {
		return value, nil
	}
	if err != nil {
		return value, fmt.Errorf("load native OAuth browser request for rotation: %w", err)
	}
	value.found = true
	return value, nil
}

func restoreNativeBrowserRequest(
	ctx context.Context,
	tx *sql.Tx,
	digest [32]byte,
	value nativeBrowserRequest,
) error {
	if !value.found {
		return nil
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO native_oauth_browser_requests (session_hash, query, csrf)
VALUES (?, ?, ?)`, digest[:], value.query, value.csrf); err != nil {
		return fmt.Errorf("restore native OAuth browser request after rotation: %w", err)
	}
	return nil
}
