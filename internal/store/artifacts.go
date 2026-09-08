package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"strings"
	"time"

	"github.com/uptrace/bun"
)

var (
	ErrArtifactNotFound            = errors.New("artifact not found")
	ErrInvalidArtifactVersionIndex = errors.New("invalid version index")
)

const (
	ArtifactLocalFile        = "local_file"
	ArtifactExternalURL      = "external_url"
	maxArtifactMetadataBytes = 64 * 1024
)

// ArtifactOwner identifies the object that controls one Artifact.
type ArtifactOwner struct {
	ObjectType string
	ObjectID   string
}

// ArtifactSource records optional conversation provenance.
type ArtifactSource struct {
	ConversationID string
	TurnID         string
	ItemID         string
}

// Artifact is one stored Artifact family.
type Artifact struct {
	ID               string
	Owner            ArtifactOwner
	Title            string
	Description      *string
	Kind             string
	StorageKind      string
	CurrentVersionID string
	CreatedByActorID string
	Source           ArtifactSource
	Metadata         map[string]any
	CreatedAt        time.Time
	UpdatedAt        time.Time
}

// ArtifactVersion is one immutable Artifact payload.
type ArtifactVersion struct {
	ID                string
	ArtifactID        string
	Index             int64
	Title             *string
	LocalRelativePath *string
	ExternalURL       *string
	MediaType         *string
	ByteSize          *int64
	ContentSHA256     *string
	CreatedByActorID  string
	Source            ArtifactSource
	Metadata          map[string]any
	CreatedAt         time.Time
}

// ArtifactWithVersions combines one Artifact with its immutable history.
type ArtifactWithVersions struct {
	Artifact       Artifact
	CurrentVersion ArtifactVersion
	Versions       []ArtifactVersion
}

// NewArtifactID creates one portable Artifact identifier.
func NewArtifactID() (string, error) { return newID("artifact") }

// NewArtifactVersionID creates one portable Artifact version identifier.
func NewArtifactVersionID() (string, error) { return newID("artifact_version") }

// ArtifactOwnerAuthorized reports whether the local human controls one owner.
func (s *Store) ArtifactOwnerAuthorized(ctx context.Context, owner ArtifactOwner) (bool, error) {
	return artifactOwnerAuthorized(ctx, s.db, owner)
}

// CreateArtifact stores one Artifact and its first immutable version atomically.
func (s *Store) CreateArtifact(
	ctx context.Context,
	artifact Artifact,
	version ArtifactVersion,
	now time.Time,
) (ArtifactWithVersions, error) {
	if err := validateArtifact(artifact); err != nil {
		return ArtifactWithVersions{}, err
	}
	if err := validateArtifactVersion(version, artifact.ID, 1, artifact.StorageKind); err != nil {
		return ArtifactWithVersions{}, err
	}
	artifactMetadata, err := encodeArtifactMetadata(artifact.Metadata)
	if err != nil {
		return ArtifactWithVersions{}, err
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ArtifactWithVersions{}, fmt.Errorf("begin Artifact creation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	authorized, err := artifactOwnerAuthorized(ctx, tx, artifact.Owner)
	if err != nil {
		return ArtifactWithVersions{}, err
	}
	if !authorized {
		return ArtifactWithVersions{}, ErrArtifactNotFound
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO artifacts (
    artifact_id, owner_object_type, owner_object_id, title, description,
    artifact_kind, storage_kind, current_version_id, created_by_actor_id,
    source_conversation_id, source_turn_id, source_item_id, metadata_json,
    created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?)`,
		artifact.ID, artifact.Owner.ObjectType, artifact.Owner.ObjectID, strings.TrimSpace(artifact.Title),
		nullableArtifactString(artifact.Description), strings.TrimSpace(artifact.Kind), artifact.StorageKind,
		artifact.CreatedByActorID, nullableText(artifact.Source.ConversationID), nullableText(artifact.Source.TurnID),
		nullableText(artifact.Source.ItemID), artifactMetadata, millis(now), millis(now)); err != nil {
		return ArtifactWithVersions{}, fmt.Errorf("insert Artifact: %w", err)
	}
	version.Index = 1
	version.ArtifactID = artifact.ID
	version.CreatedAt = now
	if err := insertArtifactVersion(ctx, tx, version); err != nil {
		return ArtifactWithVersions{}, err
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE artifacts SET current_version_id = ? WHERE artifact_id = ?`, version.ID, artifact.ID); err != nil {
		return ArtifactWithVersions{}, fmt.Errorf("select current Artifact version: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return ArtifactWithVersions{}, fmt.Errorf("commit Artifact creation: %w", err)
	}
	artifact.Title = strings.TrimSpace(artifact.Title)
	artifact.Kind = strings.TrimSpace(artifact.Kind)
	artifact.Metadata = normalizedArtifactMetadata(artifact.Metadata)
	version.Metadata = normalizedArtifactMetadata(version.Metadata)
	artifact.CurrentVersionID = version.ID
	artifact.CreatedAt, artifact.UpdatedAt = now, now
	return ArtifactWithVersions{Artifact: artifact, CurrentVersion: version, Versions: []ArtifactVersion{version}}, nil
}

// AppendArtifactVersion adds one immutable version and selects it as current.
func (s *Store) AppendArtifactVersion(
	ctx context.Context,
	artifactID string,
	version ArtifactVersion,
	now time.Time,
) (ArtifactVersion, error) {
	return s.appendArtifactVersion(ctx, artifactID, nil, version, now)
}

// AppendArtifactVersionAtIndex appends one version when the caller's expected
// next index is positive and still current.
func (s *Store) AppendArtifactVersionAtIndex(
	ctx context.Context,
	artifactID string,
	expectedIndex int64,
	version ArtifactVersion,
	now time.Time,
) (ArtifactVersion, error) {
	if expectedIndex < 1 {
		return ArtifactVersion{}, fmt.Errorf("%w: %d", ErrInvalidArtifactVersionIndex, expectedIndex)
	}
	return s.appendArtifactVersion(ctx, artifactID, &expectedIndex, version, now)
}

func (s *Store) appendArtifactVersion(
	ctx context.Context,
	artifactID string,
	expectedIndex *int64,
	version ArtifactVersion,
	now time.Time,
) (ArtifactVersion, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ArtifactVersion{}, fmt.Errorf("begin Artifact version append: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var storageKind string
	var next int64
	if err := tx.QueryRowContext(ctx, `
SELECT storage_kind, COALESCE(MAX(artifact_versions.version_index), 0) + 1
FROM artifacts LEFT JOIN artifact_versions USING (artifact_id)
WHERE artifacts.artifact_id = ? AND artifacts.deleted_at_ms IS NULL
GROUP BY artifacts.artifact_id`, artifactID).Scan(&storageKind, &next); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return ArtifactVersion{}, ErrArtifactNotFound
		}
		return ArtifactVersion{}, fmt.Errorf("inspect Artifact append: %w", err)
	}
	if expectedIndex != nil && next != *expectedIndex {
		return ArtifactVersion{}, fmt.Errorf("artifact version append conflict: expected %d, current %d", *expectedIndex, next)
	}
	if err := validateArtifactVersion(version, artifactID, next, storageKind); err != nil {
		return ArtifactVersion{}, err
	}
	version.ArtifactID, version.Index, version.CreatedAt = artifactID, next, now.UTC()
	if err := insertArtifactVersion(ctx, tx, version); err != nil {
		return ArtifactVersion{}, err
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE artifacts SET current_version_id = ?, updated_at_ms = ? WHERE artifact_id = ?`,
		version.ID, millis(version.CreatedAt), artifactID); err != nil {
		return ArtifactVersion{}, fmt.Errorf("select appended Artifact version: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return ArtifactVersion{}, fmt.Errorf("commit Artifact version: %w", err)
	}
	version.Metadata = normalizedArtifactMetadata(version.Metadata)
	return version, nil
}

// ArtifactWithVersionsByID returns one Artifact and its version history.
func (s *Store) ArtifactWithVersionsByID(ctx context.Context, artifactID string) (ArtifactWithVersions, error) {
	artifact, err := scanArtifact(s.db.QueryRowContext(ctx, artifactSelect+`
WHERE artifact_id = ? AND deleted_at_ms IS NULL`, artifactID))
	if err != nil {
		return ArtifactWithVersions{}, err
	}
	versions, err := s.artifactVersions(ctx, artifactID)
	if err != nil {
		return ArtifactWithVersions{}, err
	}
	for _, version := range versions {
		if version.ID == artifact.CurrentVersionID {
			return ArtifactWithVersions{Artifact: artifact, CurrentVersion: version, Versions: versions}, nil
		}
	}
	return ArtifactWithVersions{}, errors.New("Artifact current version is unavailable")
}

// ArtifactVersionWithArtifact returns one version and its Artifact family.
func (s *Store) ArtifactVersionWithArtifact(ctx context.Context, versionID string) (ArtifactWithVersions, ArtifactVersion, error) {
	version, err := scanArtifactVersion(s.db.QueryRowContext(ctx, artifactVersionSelect+`
WHERE artifact_version_id = ?`, versionID))
	if err != nil {
		return ArtifactWithVersions{}, ArtifactVersion{}, err
	}
	artifact, err := s.ArtifactWithVersionsByID(ctx, version.ArtifactID)
	return artifact, version, err
}

// ArtifactsForOwner lists current Artifacts from newest to oldest.
func (s *Store) ArtifactsForOwner(ctx context.Context, owner ArtifactOwner, limit int) ([]ArtifactWithVersions, error) {
	if limit < 1 {
		limit = -1
	} else if limit > 100 {
		limit = 100
	}
	rows, err := s.db.QueryContext(ctx, `
SELECT artifact_id FROM artifacts
WHERE owner_object_type = ? AND owner_object_id = ? AND deleted_at_ms IS NULL
ORDER BY updated_at_ms DESC, artifact_id DESC LIMIT ?`, owner.ObjectType, owner.ObjectID, limit)
	if err != nil {
		return nil, fmt.Errorf("list Artifacts: %w", err)
	}
	defer rows.Close()
	ids := make([]string, 0)
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, fmt.Errorf("scan Artifact id: %w", err)
		}
		ids = append(ids, id)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read Artifact ids: %w", err)
	}
	result := make([]ArtifactWithVersions, 0, len(ids))
	for _, id := range ids {
		artifact, err := s.ArtifactWithVersionsByID(ctx, id)
		if err != nil {
			return nil, err
		}
		result = append(result, artifact)
	}
	return result, nil
}

// AuthorizedLocalArtifactVersion returns local bytes metadata for the local human.
func (s *Store) AuthorizedLocalArtifactVersion(
	ctx context.Context,
	versionID string,
) (Artifact, ArtifactVersion, bool, error) {
	artifact, version, err := s.ArtifactVersionWithArtifact(ctx, versionID)
	if errors.Is(err, ErrArtifactNotFound) {
		return Artifact{}, ArtifactVersion{}, false, nil
	}
	if err != nil {
		return Artifact{}, ArtifactVersion{}, false, err
	}
	authorized, err := s.ArtifactOwnerAuthorized(ctx, artifact.Artifact.Owner)
	if err != nil || !authorized || artifact.Artifact.StorageKind != ArtifactLocalFile || version.LocalRelativePath == nil {
		return Artifact{}, ArtifactVersion{}, false, err
	}
	return artifact.Artifact, version, true, nil
}

func (s *Store) artifactVersions(ctx context.Context, artifactID string) ([]ArtifactVersion, error) {
	rows, err := s.db.QueryContext(ctx, artifactVersionSelect+`
WHERE artifact_id = ? ORDER BY version_index`, artifactID)
	if err != nil {
		return nil, fmt.Errorf("list Artifact versions: %w", err)
	}
	defer rows.Close()
	versions := make([]ArtifactVersion, 0)
	for rows.Next() {
		version, err := scanArtifactVersion(rows)
		if err != nil {
			return nil, err
		}
		versions = append(versions, version)
	}
	return versions, rows.Err()
}

func artifactOwnerAuthorized(ctx context.Context, query rowQueryer, owner ArtifactOwner) (bool, error) {
	var statement string
	switch owner.ObjectType {
	case "conversation":
		statement = "SELECT EXISTS(SELECT 1 FROM conversations WHERE conversation_id = ? AND owner_human_id = 'human:local')"
	case "task":
		statement = "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id = ?)"
	default:
		return false, nil
	}
	var exists bool
	if err := query.QueryRowContext(ctx, statement, owner.ObjectID).Scan(&exists); err != nil {
		return false, fmt.Errorf("authorize Artifact owner: %w", err)
	}
	return exists, nil
}

func insertArtifactVersion(ctx context.Context, tx bun.Tx, version ArtifactVersion) error {
	metadata, err := encodeArtifactMetadata(version.Metadata)
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx, `
INSERT INTO artifact_versions (
    artifact_version_id, artifact_id, version_index, title, local_relative_path,
    external_url, media_type, byte_size, content_sha256, created_by_actor_id,
    source_conversation_id, source_turn_id, source_item_id, metadata_json, created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		version.ID, version.ArtifactID, version.Index, nullableArtifactString(version.Title),
		nullableArtifactString(version.LocalRelativePath), nullableArtifactString(version.ExternalURL),
		nullableArtifactString(version.MediaType), nullableInt64(version.ByteSize),
		nullableArtifactString(version.ContentSHA256), version.CreatedByActorID,
		nullableText(version.Source.ConversationID), nullableText(version.Source.TurnID),
		nullableText(version.Source.ItemID), metadata, millis(version.CreatedAt))
	if err != nil {
		return fmt.Errorf("insert Artifact version: %w", err)
	}
	return nil
}

const artifactSelect = `
SELECT artifact_id, owner_object_type, owner_object_id, title, description,
       artifact_kind, storage_kind, COALESCE(current_version_id, ''), created_by_actor_id,
       COALESCE(source_conversation_id, ''), COALESCE(source_turn_id, ''),
       COALESCE(source_item_id, ''), metadata_json, created_at_ms, updated_at_ms
FROM artifacts `

const artifactVersionSelect = `
SELECT artifact_version_id, artifact_id, version_index, title, local_relative_path,
       external_url, media_type, byte_size, content_sha256, created_by_actor_id,
       COALESCE(source_conversation_id, ''), COALESCE(source_turn_id, ''),
       COALESCE(source_item_id, ''), metadata_json, created_at_ms
FROM artifact_versions `

func scanArtifact(row rowScanner) (Artifact, error) {
	var value Artifact
	var description sql.NullString
	var metadata string
	var created, updated int64
	err := row.Scan(&value.ID, &value.Owner.ObjectType, &value.Owner.ObjectID, &value.Title, &description,
		&value.Kind, &value.StorageKind, &value.CurrentVersionID, &value.CreatedByActorID,
		&value.Source.ConversationID, &value.Source.TurnID, &value.Source.ItemID, &metadata, &created, &updated)
	if errors.Is(err, sql.ErrNoRows) {
		return Artifact{}, ErrArtifactNotFound
	}
	if err != nil {
		return Artifact{}, fmt.Errorf("scan Artifact: %w", err)
	}
	value.Description = nullStringPointer(description)
	value.Metadata, err = decodeArtifactMetadata(metadata)
	if err != nil {
		return Artifact{}, err
	}
	value.CreatedAt, value.UpdatedAt = fromMillis(created), fromMillis(updated)
	return value, nil
}

func scanArtifactVersion(row rowScanner) (ArtifactVersion, error) {
	var value ArtifactVersion
	var title, localPath, externalURL, mediaType, digest sql.NullString
	var metadata string
	var size sql.NullInt64
	var created int64
	err := row.Scan(&value.ID, &value.ArtifactID, &value.Index, &title, &localPath, &externalURL,
		&mediaType, &size, &digest, &value.CreatedByActorID, &value.Source.ConversationID,
		&value.Source.TurnID, &value.Source.ItemID, &metadata, &created)
	if errors.Is(err, sql.ErrNoRows) {
		return ArtifactVersion{}, ErrArtifactNotFound
	}
	if err != nil {
		return ArtifactVersion{}, fmt.Errorf("scan Artifact version: %w", err)
	}
	value.Title = nullStringPointer(title)
	value.LocalRelativePath = nullStringPointer(localPath)
	value.ExternalURL = nullStringPointer(externalURL)
	value.MediaType = nullStringPointer(mediaType)
	value.ContentSHA256 = nullStringPointer(digest)
	value.Metadata, err = decodeArtifactMetadata(metadata)
	if err != nil {
		return ArtifactVersion{}, err
	}
	if value.ExternalURL != nil {
		parsed, parseErr := url.Parse(*value.ExternalURL)
		if parseErr != nil || parsed.Host == "" || (parsed.Scheme != "http" && parsed.Scheme != "https") {
			return ArtifactVersion{}, errors.New("stored Artifact external URL is invalid")
		}
	}
	if size.Valid {
		value.ByteSize = &size.Int64
	}
	value.CreatedAt = fromMillis(created)
	return value, nil
}

func validateArtifact(value Artifact) error {
	if !validArtifactID(value.ID) || strings.TrimSpace(value.Title) == "" || strings.TrimSpace(value.Kind) == "" ||
		strings.TrimSpace(value.CreatedByActorID) == "" {
		return errors.New("invalid Artifact")
	}
	if value.Owner.ObjectType != "conversation" && value.Owner.ObjectType != "task" {
		return errors.New("invalid Artifact owner")
	}
	if strings.TrimSpace(value.Owner.ObjectID) == "" || (value.StorageKind != ArtifactLocalFile && value.StorageKind != ArtifactExternalURL) {
		return errors.New("invalid Artifact")
	}
	return nil
}

func validateArtifactVersion(value ArtifactVersion, artifactID string, index int64, storageKind string) error {
	if !validArtifactVersionID(value.ID) || strings.TrimSpace(value.CreatedByActorID) == "" {
		return errors.New("invalid Artifact version")
	}
	if value.ArtifactID != "" && value.ArtifactID != artifactID || value.Index != 0 && value.Index != index {
		return errors.New("invalid Artifact version target")
	}
	local := value.LocalRelativePath != nil && *value.LocalRelativePath != ""
	external := value.ExternalURL != nil && *value.ExternalURL != ""
	if local == external || local != (storageKind == ArtifactLocalFile) {
		return errors.New("Artifact storage kind mismatch")
	}
	return nil
}

func validArtifactID(id string) bool        { return validPrefixedID(id, "artifact:") }
func validArtifactVersionID(id string) bool { return validPrefixedID(id, "artifact_version:") }

func validPrefixedID(id, prefix string) bool {
	value, ok := strings.CutPrefix(id, prefix)
	if !ok || len(value) != 32 {
		return false
	}
	for _, character := range value {
		if character < '0' || character > '9' && character < 'a' || character > 'f' {
			return false
		}
	}
	return true
}

type rowQueryer interface {
	QueryRowContext(context.Context, string, ...any) *sql.Row
}

func nullableText(value string) any {
	if value == "" {
		return nil
	}
	return value
}

func nullableArtifactString(value *string) any {
	if value == nil {
		return nil
	}
	return *value
}

func nullableInt64(value *int64) any {
	if value == nil {
		return nil
	}
	return *value
}

func encodeArtifactMetadata(value map[string]any) (string, error) {
	bytes, err := json.Marshal(normalizedArtifactMetadata(value))
	if err != nil || len(bytes) > maxArtifactMetadataBytes {
		return "", errors.New("invalid Artifact metadata")
	}
	return string(bytes), nil
}

func decodeArtifactMetadata(value string) (map[string]any, error) {
	if len(value) > maxArtifactMetadataBytes {
		return nil, errors.New("stored Artifact metadata exceeds its size limit")
	}
	var metadata map[string]any
	if err := json.Unmarshal([]byte(value), &metadata); err != nil || metadata == nil {
		return nil, errors.New("stored Artifact metadata is invalid")
	}
	return metadata, nil
}

func normalizedArtifactMetadata(value map[string]any) map[string]any {
	if value == nil {
		return map[string]any{}
	}
	return value
}
