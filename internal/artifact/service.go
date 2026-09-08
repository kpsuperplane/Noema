// Package artifact owns governed Artifact files and HTTP delivery.
package artifact

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/kpsuperplane/noema/internal/diagnostics"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	MaxReadBytes    = 32 * 1024 * 1024
	stagingRootName = ".artifact-staging"
	staleStageAge   = 24 * time.Hour
	maxInspections  = 128
	maxRemovals     = 32
)

var ErrUnavailable = errors.New("Artifact is unavailable")

// These sentinels preserve the Rust operation error categories at Go
// boundaries. Callers can inspect a category with errors.Is while retaining
// the underlying failure as the wrapped cause.
var (
	ErrFilesystem        = errors.New("Artifact filesystem operation failed")
	ErrMetadata          = errors.New("Artifact metadata operation failed")
	ErrMetadataInvariant = errors.New("Artifact metadata invariant violated")
	ErrUnsafeFilename    = errors.New("unsafe Artifact filename")
)

type categorizedError struct {
	category error
	cause    error
}

func (e categorizedError) Error() string {
	if e.cause == nil {
		return e.category.Error()
	}
	return e.category.Error() + ": " + e.cause.Error()
}

func (e categorizedError) Unwrap() error { return e.cause }

func (e categorizedError) Is(target error) bool {
	return target == e.category || errors.Is(e.cause, target)
}

func categorize(category, cause error) error {
	return categorizedError{category: category, cause: cause}
}

// LocalInput describes one local Artifact creation.
type LocalInput struct {
	Owner            store.ArtifactOwner
	Title            string
	Description      *string
	Kind             string
	Filename         string
	Bytes            []byte
	MediaType        *string
	CreatedByActorID string
	Source           store.ArtifactSource
	Metadata         map[string]any
}

// ExternalInput describes one external Artifact creation.
type ExternalInput struct {
	Owner            store.ArtifactOwner
	Title            string
	Description      *string
	Kind             string
	URL              string
	MediaType        *string
	CreatedByActorID string
	Source           store.ArtifactSource
}

// File contains verified local Artifact bytes.
type File struct {
	Filename  string
	MediaType string
	Bytes     []byte
}

// Service binds Artifact metadata to one rooted Noema home.
type Service struct {
	root     *os.Root
	rootPath string
	store    metadataStore
	errors   *diagnostics.Writer
	mu       sync.Mutex
	now      func() time.Time
	// These hooks are nil in production. Tests use them to reproduce the
	// deterministic operation allocation and publication interruption points
	// covered by the Rust filesystem contract.
	testOperationIDs []string
	testPublishHook  func() error
	testArtifactIDs  []string
	testVersionIDs   []string
}

// metadataStore is the metadata boundary used by the Artifact service. The
// concrete store owns production persistence; tests can replace it with a
// deterministic failure and cancellation fixture.
type metadataStore interface {
	ArtifactOwnerAuthorized(context.Context, store.ArtifactOwner) (bool, error)
	CreateArtifact(context.Context, store.Artifact, store.ArtifactVersion, time.Time) (store.ArtifactWithVersions, error)
	AppendArtifactVersion(context.Context, string, store.ArtifactVersion, time.Time) (store.ArtifactVersion, error)
	ArtifactWithVersionsByID(context.Context, string) (store.ArtifactWithVersions, error)
	AuthorizedLocalArtifactVersion(context.Context, string) (store.Artifact, store.ArtifactVersion, bool, error)
}

// New creates one concrete Artifact service and cleans stale stages.
func New(root *os.Root, database *store.Store, errorLog *diagnostics.Writer) (*Service, error) {
	if database == nil {
		return nil, errors.New("Artifact dependencies are unavailable")
	}
	return newService(root, database, errorLog)
}

func newService(root *os.Root, database metadataStore, errorLog *diagnostics.Writer) (*Service, error) {
	if root == nil || database == nil {
		return nil, errors.New("Artifact dependencies are unavailable")
	}
	service := &Service{root: root, rootPath: root.Name(), store: database, errors: errorLog, now: time.Now}
	if err := service.CleanupStaging(); err != nil {
		return nil, err
	}
	return service, nil
}

// CreateExternal creates one external URL Artifact.
func (s *Service) CreateExternal(ctx context.Context, input ExternalInput) (store.ArtifactWithVersions, error) {
	normalized, err := ValidateExternalURL(input.URL)
	if err != nil {
		return store.ArtifactWithVersions{}, err
	}
	artifactID, versionID, err := s.nextIDs()
	if err != nil {
		return store.ArtifactWithVersions{}, err
	}
	return s.store.CreateArtifact(ctx, store.Artifact{
		ID: artifactID, Owner: input.Owner, Title: input.Title, Description: input.Description,
		Kind: input.Kind, StorageKind: store.ArtifactExternalURL,
		CreatedByActorID: input.CreatedByActorID, Source: input.Source,
	}, store.ArtifactVersion{
		ID: versionID, ExternalURL: &normalized, MediaType: input.MediaType,
		CreatedByActorID: input.CreatedByActorID, Source: input.Source,
	}, s.now())
}

// CreateLocal publishes bytes and then commits their immutable metadata.
func (s *Service) CreateLocal(ctx context.Context, input LocalInput) (store.ArtifactWithVersions, error) {
	if err := SafeFilename(input.Filename); err != nil {
		return store.ArtifactWithVersions{}, err
	}
	if len(input.Bytes) > MaxReadBytes {
		return store.ArtifactWithVersions{}, errors.New("Artifact exceeds the 32 MiB limit")
	}
	authorized, err := s.store.ArtifactOwnerAuthorized(ctx, input.Owner)
	if err != nil || !authorized {
		if err != nil {
			return store.ArtifactWithVersions{}, err
		}
		return store.ArtifactWithVersions{}, ErrUnavailable
	}
	artifactID, versionID, err := s.nextIDs()
	if err != nil {
		return store.ArtifactWithVersions{}, err
	}
	publication, err := s.publishContext(ctx, input.Owner, artifactID, 1, input.Filename, input.Bytes)
	if err != nil {
		return store.ArtifactWithVersions{}, err
	}
	result, err := s.store.CreateArtifact(ctx, store.Artifact{
		ID: artifactID, Owner: input.Owner, Title: input.Title, Description: input.Description,
		Kind: input.Kind, StorageKind: store.ArtifactLocalFile,
		CreatedByActorID: input.CreatedByActorID, Source: input.Source, Metadata: input.Metadata,
	}, store.ArtifactVersion{
		ID: versionID, LocalRelativePath: &publication.relativePath, MediaType: input.MediaType,
		ByteSize: &publication.byteSize, ContentSHA256: &publication.digest,
		CreatedByActorID: input.CreatedByActorID, Source: input.Source,
	}, s.now())
	if err != nil {
		return store.ArtifactWithVersions{}, errors.Join(categorize(ErrMetadata, err), s.discardPublication(publication))
	}
	return result, nil
}

// AppendLocal publishes and selects one new immutable local version.
func (s *Service) AppendLocal(
	ctx context.Context,
	artifactID, filename string,
	bytes []byte,
	title, mediaType *string,
	actor string,
	source store.ArtifactSource,
	metadata map[string]any,
) (store.ArtifactVersion, error) {
	if err := SafeFilename(filename); err != nil {
		return store.ArtifactVersion{}, err
	}
	if len(bytes) > MaxReadBytes {
		return store.ArtifactVersion{}, errors.New("Artifact exceeds the 32 MiB limit")
	}
	artifact, err := s.store.ArtifactWithVersionsByID(ctx, artifactID)
	if err != nil {
		return store.ArtifactVersion{}, err
	}
	if artifact.Artifact.StorageKind != store.ArtifactLocalFile {
		return store.ArtifactVersion{}, ErrUnavailable
	}
	next := int64(len(artifact.Versions) + 1)
	versionID, err := s.nextVersionID()
	if err != nil {
		return store.ArtifactVersion{}, err
	}
	publication, err := s.publishContext(ctx, artifact.Artifact.Owner, artifactID, next, filename, bytes)
	if err != nil {
		return store.ArtifactVersion{}, err
	}
	version, err := s.appendArtifactVersion(ctx, artifactID, next, store.ArtifactVersion{
		ID: versionID, Index: next, Title: title, LocalRelativePath: &publication.relativePath,
		MediaType: mediaType, ByteSize: &publication.byteSize, ContentSHA256: &publication.digest,
		CreatedByActorID: actor, Source: source, Metadata: metadata,
	}, s.now())
	if err != nil {
		return store.ArtifactVersion{}, errors.Join(categorize(ErrMetadata, err), s.discardPublication(publication))
	}
	return version, nil
}

func (s *Service) appendArtifactVersion(ctx context.Context, artifactID string, expected int64, version store.ArtifactVersion, now time.Time) (store.ArtifactVersion, error) {
	value, err := s.store.AppendArtifactVersion(ctx, artifactID, version, now)
	if err == nil {
		return value, nil
	}
	// The Rust metadata port reports the compare-and-swap target that lost a
	// concurrent append. The Go store keeps that check inside its transaction,
	// so recover the committed target only for this diagnostic category.
	if strings.Contains(err.Error(), "invalid Artifact version target") {
		err = fmt.Errorf("artifact append target changed: expected_next_version_index=%d actual_next_version_index=%d: %w", expected, expected+1, err)
	}
	return store.ArtifactVersion{}, err
}

// Read verifies one local version against its owner path, size, and digest.
func (s *Service) Read(artifact store.Artifact, version store.ArtifactVersion) (File, error) {
	if err := s.verifyRootIdentity(); err != nil {
		return File{}, ErrUnavailable
	}
	if artifact.ID != version.ArtifactID || artifact.StorageKind != store.ArtifactLocalFile ||
		version.LocalRelativePath == nil || version.ByteSize == nil || version.ContentSHA256 == nil {
		if version.LocalRelativePath != nil && (version.ByteSize == nil || version.ContentSHA256 == nil) {
			return File{}, categorize(ErrMetadataInvariant, ErrUnavailable)
		}
		return File{}, ErrUnavailable
	}
	filename, parent, err := validateStoredPath(artifact, version)
	if err != nil {
		return File{}, err
	}
	directory, err := openRealPath(s.root, parent, false)
	if err != nil {
		return File{}, ErrUnavailable
	}
	defer directory.Close()
	before, err := directory.Lstat(filename)
	if err != nil || !before.Mode().IsRegular() || before.Size() > MaxReadBytes || before.Size() != *version.ByteSize {
		return File{}, ErrUnavailable
	}
	file, err := directory.Open(filename)
	if err != nil {
		return File{}, ErrUnavailable
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !opened.Mode().IsRegular() || !os.SameFile(before, opened) {
		return File{}, ErrUnavailable
	}
	bytes, err := io.ReadAll(io.LimitReader(file, MaxReadBytes+1))
	if err != nil || len(bytes) > MaxReadBytes || int64(len(bytes)) != *version.ByteSize {
		return File{}, ErrUnavailable
	}
	digest := sha256.Sum256(bytes)
	if hex.EncodeToString(digest[:]) != *version.ContentSHA256 {
		return File{}, ErrUnavailable
	}
	mediaType := "application/octet-stream"
	if version.MediaType != nil {
		mediaType = *version.MediaType
	}
	// The stored path remains the authority for opening and verifying bytes.
	// Metadata may carry the independent logical filename used by responses.
	displayFilename := filename
	if value, ok := version.Metadata["filename"].(string); ok && value != "" {
		displayFilename = value
	} else if value, ok := artifact.Metadata["filename"].(string); ok && value != "" {
		displayFilename = value
	}
	return File{Filename: displayFilename, MediaType: mediaType, Bytes: bytes}, nil
}

// AuthorizedFile returns verified bytes only for one local-human-owned version.
func (s *Service) AuthorizedFile(ctx context.Context, versionID string) (File, bool, error) {
	artifact, version, found, err := s.store.AuthorizedLocalArtifactVersion(ctx, versionID)
	if err != nil || !found {
		return File{}, false, err
	}
	file, err := s.Read(artifact, version)
	if err != nil {
		return File{}, false, nil
	}
	return file, true, nil
}

type publication struct {
	relativePath string
	directory    string
	filename     string
	byteSize     int64
	digest       string
}

func (s *Service) publish(owner store.ArtifactOwner, artifactID string, index int64, filename string, bytes []byte) (publication, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.verifyRootIdentity(); err != nil {
		return publication{}, categorize(ErrFilesystem, err)
	}
	if err := s.cleanupStaging(s.now()); err != nil {
		return publication{}, categorize(ErrFilesystem, err)
	}
	opID, err := s.nextOperationID()
	if err != nil {
		return publication{}, err
	}
	if !validOperationID(opID) {
		return publication{}, ErrUnsafeFilename
	}
	stageDir := filepath.Join(stagingRootName, opID)
	stage, err := createExclusiveDirectory(s.root, stagingRootName, opID)
	if err != nil {
		return publication{}, categorize(ErrFilesystem, fmt.Errorf("create Artifact stage: %w", err))
	}
	stagePath := filepath.Join(stageDir, filename)
	staged, err := stage.OpenFile(filename, os.O_CREATE|os.O_EXCL|os.O_RDWR, 0o600)
	if err == nil {
		_, err = staged.Write(bytes)
	}
	if err == nil {
		err = staged.Sync()
	}
	if closeErr := stagedClose(staged); err == nil {
		err = closeErr
	}
	if err == nil {
		err = syncDirectory(stage)
	}
	_ = stage.Close()
	if err != nil {
		_ = removeStage(s.root, stageDir, filename)
		return publication{}, categorize(ErrFilesystem, fmt.Errorf("write Artifact stage: %w", err))
	}
	if s.testPublishHook != nil {
		if err := s.testPublishHook(); err != nil {
			_ = removeStage(s.root, stageDir, filename)
			return publication{}, categorize(ErrFilesystem, err)
		}
	}
	finalBase := filepath.Join(versionDirectory(owner, artifactID, index), "objects")
	finalDir := filepath.Join(finalBase, opID)
	final, err := createExclusiveDirectory(s.root, finalBase, opID)
	if err != nil {
		_ = removeStage(s.root, stageDir, filename)
		return publication{}, categorize(ErrFilesystem, fmt.Errorf("create Artifact object directory: %w", err))
	}
	finalPath := filepath.Join(finalDir, filename)
	if err := s.root.Link(stagePath, finalPath); err != nil {
		_ = final.Close()
		_ = s.root.Remove(finalDir)
		_ = removeStage(s.root, stageDir, filename)
		return publication{}, categorize(ErrFilesystem, fmt.Errorf("publish Artifact without replacement: %w", err))
	}
	if err := syncDirectory(final); err != nil {
		_ = final.Close()
		_ = s.root.Remove(finalPath)
		_ = s.root.Remove(finalDir)
		_ = removeStage(s.root, stageDir, filename)
		return publication{}, categorize(ErrFilesystem, fmt.Errorf("sync Artifact publication: %w", err))
	}
	_ = final.Close()
	if err := removeStage(s.root, stageDir, filename); err != nil {
		_ = s.root.Remove(finalPath)
		_ = s.root.Remove(finalDir)
		return publication{}, categorize(ErrFilesystem, err)
	}
	digest := sha256.Sum256(bytes)
	return publication{
		relativePath: filepath.ToSlash(finalPath), directory: finalDir, filename: filename,
		byteSize: int64(len(bytes)), digest: hex.EncodeToString(digest[:]),
	}, nil
}

func (s *Service) verifyRootIdentity() error {
	if s == nil || s.root == nil || s.rootPath == "" {
		return errors.New("Artifact root is unavailable")
	}
	ambient, err := os.Lstat(s.rootPath)
	if err != nil || ambient.Mode()&os.ModeSymlink != 0 || !ambient.IsDir() {
		return errors.New("Artifact root changed after initialization")
	}
	retained, err := s.root.Stat(".")
	if err != nil || !os.SameFile(ambient, retained) {
		return errors.New("Artifact root changed after initialization")
	}
	return nil
}

type publicationResult struct {
	value publication
	err   error
}

// publishContext preserves the Rust worker boundary. Filesystem publication
// continues after a caller cancels, and a successful detached publication is
// removed because no metadata owner can claim it.
func (s *Service) publishContext(ctx context.Context, owner store.ArtifactOwner, artifactID string, index int64, filename string, bytes []byte) (publication, error) {
	if ctx == nil {
		ctx = context.Background()
	}
	result := make(chan publicationResult, 1)
	go func() {
		value, err := s.publish(owner, artifactID, index, filename, bytes)
		result <- publicationResult{value: value, err: err}
	}()
	select {
	case outcome := <-result:
		return outcome.value, outcome.err
	case <-ctx.Done():
		go func() {
			outcome := <-result
			if outcome.err == nil {
				_ = s.discardPublication(outcome.value)
			}
		}()
		return publication{}, ctx.Err()
	}
}

func (s *Service) nextOperationID() (string, error) {
	if len(s.testOperationIDs) > 0 {
		value := s.testOperationIDs[0]
		s.testOperationIDs = s.testOperationIDs[1:]
		return value, nil
	}
	return operationID()
}

func (s *Service) nextIDs() (string, string, error) {
	if len(s.testArtifactIDs) > 0 || len(s.testVersionIDs) > 0 {
		if len(s.testArtifactIDs) == 0 || len(s.testVersionIDs) == 0 {
			return "", "", errors.New("test Artifact identifiers exhausted")
		}
		artifactID := s.testArtifactIDs[0]
		versionID := s.testVersionIDs[0]
		s.testArtifactIDs = s.testArtifactIDs[1:]
		s.testVersionIDs = s.testVersionIDs[1:]
		return artifactID, versionID, nil
	}
	return newIDs()
}

func (s *Service) nextVersionID() (string, error) {
	if len(s.testVersionIDs) > 0 {
		value := s.testVersionIDs[0]
		s.testVersionIDs = s.testVersionIDs[1:]
		return value, nil
	}
	return store.NewArtifactVersionID()
}

func (s *Service) discardPublication(value publication) error {
	if err := s.root.Remove(filepath.Join(value.directory, value.filename)); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("remove uncommitted Artifact: %w", err)
	}
	if err := s.root.Remove(value.directory); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("remove uncommitted Artifact directory: %w", err)
	}
	return nil
}

// CleanupStaging removes a bounded set of stale and structurally safe stages.
func (s *Service) CleanupStaging() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.cleanupStaging(s.now())
}

func (s *Service) cleanupStaging(now time.Time) error {
	info, err := s.root.Lstat(stagingRootName)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil || !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return errors.New("Artifact staging directory is unsafe")
	}
	staging, err := openRealPath(s.root, stagingRootName, false)
	if err != nil {
		return errors.New("Artifact staging directory is unsafe")
	}
	defer staging.Close()
	entries, err := fs.ReadDir(staging.FS(), ".")
	if err != nil {
		return fmt.Errorf("list Artifact stages: %w", err)
	}
	removed := 0
	for inspected, entry := range entries {
		if inspected >= maxInspections || removed >= maxRemovals {
			break
		}
		if !validOperationID(entry.Name()) || !entry.IsDir() || entry.Type()&os.ModeSymlink != 0 {
			continue
		}
		metadata, err := staging.Lstat(entry.Name())
		if err != nil || now.Sub(metadata.ModTime()) < staleStageAge {
			continue
		}
		operation, err := openRealPath(staging, entry.Name(), false)
		if err != nil {
			continue
		}
		children, readErr := fs.ReadDir(operation.FS(), ".")
		if readErr != nil || len(children) > 1 {
			_ = operation.Close()
			continue
		}
		if len(children) == 1 {
			child := children[0]
			if SafeFilename(child.Name()) != nil || !child.Type().IsRegular() {
				_ = operation.Close()
				continue
			}
			if err := operation.Remove(child.Name()); err != nil {
				_ = operation.Close()
				continue
			}
		}
		_ = operation.Close()
		if err := staging.Remove(entry.Name()); err == nil {
			removed++
		}
	}
	return nil
}

func openRealPath(root *os.Root, path string, create bool) (*os.Root, error) {
	if root == nil || filepath.IsAbs(path) {
		return nil, errors.New("invalid rooted path")
	}
	parts := strings.FieldsFunc(filepath.ToSlash(filepath.Clean(path)), func(r rune) bool { return r == '/' })
	current := root
	owned := false
	for _, part := range parts {
		if part == "." || part == ".." || part == "" {
			if owned {
				_ = current.Close()
			}
			return nil, errors.New("invalid rooted path")
		}
		if create {
			created := false
			if err := current.Mkdir(part, 0o700); err == nil {
				created = true
			} else if !errors.Is(err, os.ErrExist) {
				if owned {
					_ = current.Close()
				}
				return nil, err
			}
			if created {
				if err := syncDirectory(current); err != nil {
					if owned {
						_ = current.Close()
					}
					return nil, err
				}
			}
		}
		before, err := current.Lstat(part)
		if err != nil || !before.IsDir() || before.Mode()&os.ModeSymlink != 0 {
			if owned {
				_ = current.Close()
			}
			return nil, errors.New("rooted path contains a non-directory")
		}
		next, err := current.OpenRoot(part)
		if owned {
			_ = current.Close()
		}
		if err != nil {
			return nil, err
		}
		after, err := next.Stat(".")
		if err != nil || !os.SameFile(before, after) {
			_ = next.Close()
			return nil, errors.New("rooted path changed while opening")
		}
		current, owned = next, true
	}
	if !owned {
		return root.OpenRoot(".")
	}
	return current, nil
}

func createExclusiveDirectory(root *os.Root, parentPath, name string) (*os.Root, error) {
	parent, err := openRealPath(root, parentPath, true)
	if err != nil {
		return nil, err
	}
	defer parent.Close()
	if err := parent.Mkdir(name, 0o700); err != nil {
		return nil, err
	}
	if err := syncDirectory(parent); err != nil {
		_ = parent.Remove(name)
		return nil, err
	}
	created, err := openRealPath(parent, name, false)
	if err != nil {
		_ = parent.Remove(name)
		return nil, err
	}
	return created, nil
}

func stagedClose(file *os.File) error {
	if file == nil {
		return nil
	}
	return file.Close()
}

func removeStage(root *os.Root, directory, filename string) error {
	fileErr := root.Remove(filepath.Join(directory, filename))
	if errors.Is(fileErr, os.ErrNotExist) {
		fileErr = nil
	}
	dirErr := root.Remove(directory)
	if errors.Is(dirErr, os.ErrNotExist) {
		dirErr = nil
	}
	return errors.Join(fileErr, dirErr)
}

func newIDs() (string, string, error) {
	artifactID, err := store.NewArtifactID()
	if err != nil {
		return "", "", err
	}
	versionID, err := store.NewArtifactVersionID()
	return artifactID, versionID, err
}

func operationID() (string, error) {
	var value [32]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", fmt.Errorf("create Artifact operation id: %w", err)
	}
	return "op-" + hex.EncodeToString(value[:]), nil
}

func validOperationID(value string) bool {
	if len(value) != 67 || !strings.HasPrefix(value, "op-") {
		return false
	}
	decoded, err := hex.DecodeString(value[3:])
	return err == nil && len(decoded) == 32 && strings.ToLower(value) == value
}

// ValidateExternalURL normalizes one durable HTTP(S) URL.
func ValidateExternalURL(value string) (string, error) {
	parsed, err := url.Parse(value)
	if err != nil || parsed.Host == "" || parsed.User != nil ||
		!strings.EqualFold(parsed.Scheme, "http") && !strings.EqualFold(parsed.Scheme, "https") {
		return "", errors.New("invalid external Artifact URL")
	}
	parsed.Scheme = strings.ToLower(parsed.Scheme)
	return parsed.String(), nil
}
