package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"

	"github.com/uptrace/bun"
)

var (
	ErrLocalModelNotFound = errors.New("local model installation not found")
	ErrLocalModelState    = errors.New("local model installation state is invalid")
)

type LocalModelInstallation struct {
	ID, ModelID, Name, File, SourceKind       string
	Repo, Revision, License, SHA256, BlobPath string
	CompletedBytes, TotalBytes, DiskBytes     int64
	Backend, Status, ErrorCode, ErrorMessage  string
	Active                                    bool
	CreatedAt, UpdatedAt                      time.Time
}

type LocalModelEvent struct {
	Cursor                        int64
	Kind, InstallationID, ModelID string
	CreatedAt                     time.Time
}

const localModelSelect = `SELECT installation_id,model_id,name,file,source_kind,
COALESCE(repo,''),COALESCE(revision,''),COALESCE(license,''),COALESCE(sha256,''),completed_bytes,
COALESCE(total_bytes,0),disk_bytes,COALESCE(blob_path,''),COALESCE(backend,''),status,
is_active,COALESCE(error_code,''),COALESCE(error_message,''),created_at_ms,updated_at_ms
FROM local_model_installations`

func (s *Store) QueueLocalModel(ctx context.Context, value LocalModelInstallation) (LocalModelInstallation, error) {
	if value.ID == "" || value.ModelID == "" || value.Name == "" || value.File == "" {
		return LocalModelInstallation{}, ErrLocalModelState
	}
	now := value.CreatedAt.UTC()
	if now.IsZero() {
		now = time.Now().UTC()
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return LocalModelInstallation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	_, err = tx.ExecContext(ctx, `INSERT INTO local_model_installations
(installation_id,model_id,name,file,source_kind,repo,revision,license,sha256,total_bytes,backend,status,created_at_ms,updated_at_ms)
VALUES(?,?,?,?,?,NULLIF(?,''),NULLIF(?,''),NULLIF(?,''),NULLIF(?,''),NULLIF(?,0),NULLIF(?,''),'queued',?,?)
ON CONFLICT(installation_id) DO UPDATE SET status='queued',completed_bytes=0,
error_code=NULL,error_message=NULL,updated_at_ms=excluded.updated_at_ms
WHERE local_model_installations.status IN ('failed','cancelled')`, value.ID, value.ModelID, value.Name, value.File, value.SourceKind,
		value.Repo, value.Revision, value.License, value.SHA256, value.TotalBytes, value.Backend, millis(now), millis(now))
	if err != nil {
		return LocalModelInstallation{}, fmt.Errorf("queue local model: %w", err)
	}
	var created int
	if err = tx.QueryRowContext(ctx, `SELECT changes()`).Scan(&created); err != nil {
		return LocalModelInstallation{}, err
	}
	if created == 1 {
		if err = appendLocalModelEvent(ctx, tx, "installation_updated", value.ID, value.ModelID, now); err != nil {
			return LocalModelInstallation{}, err
		}
	}
	result, err := scanLocalModel(tx.QueryRowContext(ctx, localModelSelect+` WHERE installation_id=?`, value.ID))
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if err = tx.Commit(); err != nil {
		return LocalModelInstallation{}, err
	}
	return result, nil
}

func (s *Store) LocalModelInstallation(ctx context.Context, id string) (LocalModelInstallation, error) {
	return scanLocalModel(s.db.QueryRowContext(ctx, localModelSelect+` WHERE installation_id=?`, id))
}

func (s *Store) LocalModelInstallations(ctx context.Context) ([]LocalModelInstallation, error) {
	rows, err := s.db.QueryContext(ctx, localModelSelect+` ORDER BY is_active DESC,updated_at_ms DESC,installation_id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []LocalModelInstallation
	for rows.Next() {
		value, scanErr := scanLocalModel(rows)
		if scanErr != nil {
			return nil, scanErr
		}
		result = append(result, value)
	}
	return result, rows.Err()
}

func (s *Store) UpdateLocalModel(ctx context.Context, id, status string, completed, total, disk int64,
	sha, blob, code, message string, now time.Time) (LocalModelInstallation, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return LocalModelInstallation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	current, err := scanLocalModel(tx.QueryRowContext(ctx, localModelSelect+` WHERE installation_id=?`, id))
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if !localModelTransition(current.Status, status) || completed < 0 || total < 0 || (total > 0 && completed > total) {
		return LocalModelInstallation{}, ErrLocalModelState
	}
	result, err := tx.ExecContext(ctx, `UPDATE local_model_installations SET status=?,completed_bytes=?,
total_bytes=COALESCE(NULLIF(?,0),total_bytes),disk_bytes=CASE WHEN ?>0 THEN ? ELSE disk_bytes END,
sha256=COALESCE(NULLIF(?,''),sha256),blob_path=COALESCE(NULLIF(?,''),blob_path),
error_code=NULLIF(?,''),error_message=NULLIF(?,''),updated_at_ms=? WHERE installation_id=?`,
		status, completed, total, disk, disk, sha, blob, code, message, millis(now.UTC()), id)
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return LocalModelInstallation{}, ErrLocalModelNotFound
	}
	kind := "installation_updated"
	if status == "downloading" {
		kind = "transfer_progress"
	}
	if err = appendLocalModelEvent(ctx, tx, kind, id, current.ModelID, now); err != nil {
		return LocalModelInstallation{}, err
	}
	updated, err := scanLocalModel(tx.QueryRowContext(ctx, localModelSelect+` WHERE installation_id=?`, id))
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if err = tx.Commit(); err != nil {
		return LocalModelInstallation{}, err
	}
	return updated, nil
}

func (s *Store) CancelLocalModel(ctx context.Context, id string, now time.Time) (LocalModelInstallation, error) {
	value, err := s.LocalModelInstallation(ctx, id)
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if value.Status == "cancelled" {
		return value, nil
	}
	return s.UpdateLocalModel(ctx, id, "cancelled", value.CompletedBytes, value.TotalBytes, 0, "", "", "", "", now)
}

func (s *Store) RemoveLocalModel(ctx context.Context, id string, now time.Time) (LocalModelInstallation, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return LocalModelInstallation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	value, err := scanLocalModel(tx.QueryRowContext(ctx, localModelSelect+` WHERE installation_id=?`, id))
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if value.Active {
		return LocalModelInstallation{}, ErrLocalModelState
	}
	var referenced int
	err = tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM hosted_model_assignments
WHERE provider_kind='local_models' AND model_profile=?)`, value.ModelID).Scan(&referenced)
	if err != nil || referenced != 0 {
		return LocalModelInstallation{}, ErrLocalModelState
	}
	if _, err = tx.ExecContext(ctx, `DELETE FROM local_model_installations WHERE installation_id=?`, id); err != nil {
		return LocalModelInstallation{}, err
	}
	if err = appendLocalModelEvent(ctx, tx, "installation_updated", id, value.ModelID, now); err != nil {
		return LocalModelInstallation{}, err
	}
	if err = tx.Commit(); err != nil {
		return LocalModelInstallation{}, err
	}
	return value, nil
}

func (s *Store) ActivateLocalModel(ctx context.Context, id string, assign bool, now time.Time) (LocalModelInstallation, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return LocalModelInstallation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	value, err := scanLocalModel(tx.QueryRowContext(ctx, localModelSelect+` WHERE installation_id=?`, id))
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if value.Status != "installed" {
		return LocalModelInstallation{}, ErrLocalModelState
	}
	if _, err = tx.ExecContext(ctx, `UPDATE local_model_installations SET is_active=CASE WHEN installation_id=? THEN 1 ELSE 0 END,updated_at_ms=?`, id, millis(now.UTC())); err != nil {
		return LocalModelInstallation{}, err
	}
	if _, err = tx.ExecContext(ctx, `UPDATE provider_accounts SET status='authenticated',last_checked_at_ms=?,last_authenticated_at_ms=?,last_error_code=NULL,last_error_message=NULL,updated_at_ms=? WHERE provider_account_id='provider_account:local_models:default'`, millis(now.UTC()), millis(now.UTC()), millis(now.UTC())); err != nil {
		return LocalModelInstallation{}, err
	}
	if assign {
		for _, role := range []HostedModelRole{HostedModelNoema, HostedModelSimpleTasks, HostedModelMediumTasks, HostedModelDifficultTasks, HostedModelTaskReviewer, HostedModelWebFetchSummarizer, HostedModelToolProgressAudit, HostedModelMemoryConsolidation} {
			_, err = tx.ExecContext(ctx, `INSERT INTO hosted_model_assignments(role,provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode)
VALUES(?,'local_models','provider_account:local_models:default','explicit_profile',?,NULL,0)
ON CONFLICT(role) DO UPDATE SET provider_kind=excluded.provider_kind,provider_account_id=excluded.provider_account_id,
selection_mode=excluded.selection_mode,model_profile=excluded.model_profile,reasoning_effort=NULL,fast_mode=0`, role, value.ModelID)
			if err != nil {
				return LocalModelInstallation{}, err
			}
		}
		if err = saveDefaultModelPreferenceTx(ctx, tx, ModelAssignment{ProviderKind: "local_models", ProviderAccountID: "provider_account:local_models:default", SelectionMode: ModelSelectionExplicitProfile, ModelProfile: value.ModelID}, now); err != nil {
			return LocalModelInstallation{}, err
		}
	}
	if err = appendLocalModelEvent(ctx, tx, "active_model_changed", id, value.ModelID, now); err != nil {
		return LocalModelInstallation{}, err
	}
	updated, err := scanLocalModel(tx.QueryRowContext(ctx, localModelSelect+` WHERE installation_id=?`, id))
	if err != nil {
		return LocalModelInstallation{}, err
	}
	if err = tx.Commit(); err != nil {
		return LocalModelInstallation{}, err
	}
	return updated, nil
}

func (s *Store) DefaultModelPreference(ctx context.Context) (*ModelAssignment, error) {
	var value ModelAssignment
	var profile, effort sql.NullString
	var fast int
	err := s.db.QueryRowContext(ctx, `SELECT provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode FROM default_model_preference WHERE preference_id='default'`).Scan(&value.ProviderKind, &value.ProviderAccountID, &value.SelectionMode, &profile, &effort, &fast)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	value.ModelProfile, value.ReasoningEffort, value.FastMode = profile.String, ModelReasoningEffort(effort.String), fast == 1
	return &value, nil
}

func (s *Store) SaveDefaultModelPreference(ctx context.Context, value ModelAssignment, now time.Time) (ModelAssignment, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ModelAssignment{}, err
	}
	defer func() { _ = tx.Rollback() }()
	if _, err = modelAssignmentAccount(ctx, tx, value); err != nil {
		return ModelAssignment{}, err
	}
	if err = saveDefaultModelPreferenceTx(ctx, tx, value, now); err != nil {
		return ModelAssignment{}, err
	}
	if err = appendLocalModelEvent(ctx, tx, "default_preference_changed", "", "", now); err != nil {
		return ModelAssignment{}, err
	}
	if err = tx.Commit(); err != nil {
		return ModelAssignment{}, err
	}
	return value, nil
}

func saveDefaultModelPreferenceTx(ctx context.Context, tx bun.Tx, value ModelAssignment, now time.Time) error {
	_, err := tx.ExecContext(ctx, `INSERT INTO default_model_preference(preference_id,provider_kind,provider_account_id,selection_mode,model_profile,reasoning_effort,fast_mode,updated_at_ms)
VALUES('default',?,?,?,NULLIF(?,''),NULLIF(?,''),?,?) ON CONFLICT(preference_id) DO UPDATE SET provider_kind=excluded.provider_kind,
provider_account_id=excluded.provider_account_id,selection_mode=excluded.selection_mode,model_profile=excluded.model_profile,
reasoning_effort=excluded.reasoning_effort,fast_mode=excluded.fast_mode,updated_at_ms=excluded.updated_at_ms`, value.ProviderKind, value.ProviderAccountID, value.SelectionMode, value.ModelProfile, value.ReasoningEffort, value.FastMode, millis(now.UTC()))
	return err
}

func (s *Store) LocalModelEvents(ctx context.Context, after int64, limit int) ([]LocalModelEvent, error) {
	if limit < 1 || limit > 256 {
		limit = 256
	}
	rows, err := s.db.QueryContext(ctx, `SELECT cursor,kind,COALESCE(installation_id,''),COALESCE(model_id,''),created_at_ms FROM local_model_events WHERE cursor>? ORDER BY cursor LIMIT ?`, after, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []LocalModelEvent
	for rows.Next() {
		var v LocalModelEvent
		var at int64
		if err = rows.Scan(&v.Cursor, &v.Kind, &v.InstallationID, &v.ModelID, &at); err != nil {
			return nil, err
		}
		v.CreatedAt = fromMillis(at)
		result = append(result, v)
	}
	return result, rows.Err()
}

func (s *Store) LatestLocalModelEvent(ctx context.Context) (LocalModelEvent, error) {
	var value LocalModelEvent
	var created int64
	err := s.db.QueryRowContext(ctx, `SELECT cursor,kind,COALESCE(installation_id,''),
COALESCE(model_id,''),created_at_ms FROM local_model_events ORDER BY cursor DESC LIMIT 1`).Scan(
		&value.Cursor, &value.Kind, &value.InstallationID, &value.ModelID, &created,
	)
	if err != nil {
		return value, err
	}
	value.CreatedAt = fromMillis(created)
	return value, nil
}

func appendLocalModelEvent(ctx context.Context, tx bun.Tx, kind, id, model string, now time.Time) error {
	_, err := tx.ExecContext(ctx, `INSERT INTO local_model_events(kind,installation_id,model_id,created_at_ms) VALUES(?,NULLIF(?,''),NULLIF(?,''),?)`, kind, id, model, millis(now.UTC()))
	return err
}

func scanLocalModel(row rowScanner) (LocalModelInstallation, error) {
	var v LocalModelInstallation
	var active int
	var created, updated int64
	err := row.Scan(&v.ID, &v.ModelID, &v.Name, &v.File, &v.SourceKind, &v.Repo, &v.Revision, &v.License, &v.SHA256, &v.CompletedBytes, &v.TotalBytes, &v.DiskBytes, &v.BlobPath, &v.Backend, &v.Status, &active, &v.ErrorCode, &v.ErrorMessage, &created, &updated)
	if errors.Is(err, sql.ErrNoRows) {
		return v, ErrLocalModelNotFound
	}
	if err != nil {
		return v, err
	}
	v.Active = active == 1
	v.CreatedAt = fromMillis(created)
	v.UpdatedAt = fromMillis(updated)
	return v, nil
}

func localModelTransition(from, to string) bool {
	if from == to {
		return true
	}
	switch from {
	case "queued":
		return to == "downloading" || to == "verifying" || to == "cancelled" || to == "failed"
	case "downloading":
		return to == "verifying" || to == "cancelled" || to == "failed"
	case "verifying":
		return to == "installed" || to == "cancelled" || to == "failed"
	}
	return false
}

func (s *Store) SetLocalModelAccountStatus(ctx context.Context, status provider.AccountStatus, code, message string, now time.Time) error {
	if status != provider.StatusAuthenticated && status != provider.StatusUnavailable && status != provider.StatusUnknown {
		return ErrLocalModelState
	}
	_, err := s.db.ExecContext(ctx, `UPDATE provider_accounts SET status=?,last_checked_at_ms=?,last_authenticated_at_ms=CASE WHEN ?='authenticated' THEN ? ELSE last_authenticated_at_ms END,last_error_code=NULLIF(?,''),last_error_message=NULLIF(?,''),updated_at_ms=? WHERE provider_account_id='provider_account:local_models:default'`, status, millis(now.UTC()), status, millis(now.UTC()), code, message, millis(now.UTC()))
	return err
}
