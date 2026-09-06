package home

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"unicode/utf8"
)

const (
	projectDocumentName = "PROJECT.md"
	projectPendingRoot  = ".pending-projects"
	projectManifestName = "stage.json"
	projectDocumentMax  = 64 * 1024
)

var projectTemporarySequence atomic.Uint64

var (
	ErrProjectDocumentChanged = errors.New("Project document changed elsewhere")
	ErrProjectFolderConflict  = errors.New("destination folder contains a different PROJECT.md")
	ErrInvalidProjectFolder   = errors.New("Project folder must be one absolute path")
	ErrInvalidProjectDocument = errors.New("invalid Project document")
)

// ProjectDocument is one exact Project document and its transient digest.
type ProjectDocument struct {
	Content string
	Digest  string
}

// ProjectDocumentStageKind identifies the publish rule for one central stage.
type ProjectDocumentStageKind string

const (
	ProjectDocumentCreate  ProjectDocumentStageKind = "create"
	ProjectDocumentReplace ProjectDocumentStageKind = "replace"
	ProjectDocumentMove    ProjectDocumentStageKind = "move"
)

// ProjectDocumentStage is one durable document staged inside NOEMA_HOME.
type ProjectDocumentStage struct {
	ProjectID              string
	RequestDigest          string
	Kind                   ProjectDocumentStageKind
	ExpectedDocumentDigest string
	Document               ProjectDocument
}

type projectDocumentManifest struct {
	ProjectID              string                   `json:"project_id"`
	RequestDigest          string                   `json:"request_digest"`
	Kind                   ProjectDocumentStageKind `json:"kind"`
	ExpectedDocumentDigest string                   `json:"expected_document_digest,omitempty"`
	DocumentDigest         string                   `json:"document_digest"`
}

// DefaultProjectDocument returns the initial Project context.
func DefaultProjectDocument(name string, description string) string {
	content := "# " + strings.TrimSpace(name) + "\n"
	if description = strings.TrimSpace(description); description != "" {
		content += "\n" + description + "\n"
	}
	return content
}

// StageProjectDocument stages one create document under its exact request key.
func StageProjectDocument(root *os.Root, projectID, requestDigest, content string) (ProjectDocumentStage, error) {
	return stageProjectDocument(root, ProjectDocumentStage{
		ProjectID: projectID, RequestDigest: requestDigest, Kind: ProjectDocumentCreate,
		Document: digestProjectDocument(content),
	})
}

// PrepareProjectDocumentReplace verifies the live digest and stages one replacement.
func PrepareProjectDocumentReplace(
	root *os.Root,
	projectID string,
	folder *string,
	expectedDigest string,
	content string,
	requestDigest string,
) (ProjectDocumentStage, ProjectDocument, error) {
	current, err := ReadProjectDocument(root, projectID, folder)
	if err != nil {
		return ProjectDocumentStage{}, ProjectDocument{}, err
	}
	if !validProjectDigest(expectedDigest) {
		return ProjectDocumentStage{}, ProjectDocument{}, errors.New("invalid expected Project document digest")
	}
	if current.Digest != expectedDigest {
		return ProjectDocumentStage{}, ProjectDocument{}, ErrProjectDocumentChanged
	}
	stage, err := stageProjectDocument(root, ProjectDocumentStage{
		ProjectID: projectID, RequestDigest: requestDigest, Kind: ProjectDocumentReplace,
		ExpectedDocumentDigest: expectedDigest, Document: digestProjectDocument(content),
	})
	return stage, stage.Document, err
}

// PrepareProjectDocumentMove stages the current source for one folder move.
func PrepareProjectDocumentMove(
	root *os.Root,
	projectID string,
	currentFolder *string,
	nextFolder *string,
	requestDigest string,
) (ProjectDocumentStage, error) {
	current, err := ReadProjectDocument(root, projectID, currentFolder)
	if err != nil {
		return ProjectDocumentStage{}, err
	}
	if !equalString(currentFolder, nextFolder) {
		target, targetErr := openProjectDocumentRoot(root, projectID, nextFolder, false)
		if targetErr == nil {
			existing, readErr := readProjectDocumentRoot(target)
			_ = target.Close()
			if readErr == nil && existing.Digest != current.Digest {
				return ProjectDocumentStage{}, ErrProjectFolderConflict
			}
			if readErr != nil && !errors.Is(readErr, os.ErrNotExist) {
				return ProjectDocumentStage{}, readErr
			}
		} else if !errors.Is(targetErr, os.ErrNotExist) {
			return ProjectDocumentStage{}, targetErr
		}
	}
	return stageProjectDocument(root, ProjectDocumentStage{
		ProjectID: projectID, RequestDigest: requestDigest, Kind: ProjectDocumentMove,
		ExpectedDocumentDigest: current.Digest, Document: current,
	})
}

// CommitProjectDocumentStage atomically publishes one central stage.
func CommitProjectDocumentStage(
	root *os.Root,
	stage ProjectDocumentStage,
	folder *string,
) (ProjectDocument, bool, error) {
	stored, err := ReadProjectDocumentStage(root, stage.ProjectID, stage.RequestDigest)
	if err != nil {
		return ProjectDocument{}, false, err
	}
	if !equalProjectStage(stored, stage) {
		return ProjectDocument{}, false, errors.New("Project document stage does not match its stored authority")
	}
	target, err := openProjectDocumentRoot(root, stage.ProjectID, folder, true)
	if err != nil {
		return ProjectDocument{}, false, err
	}
	defer target.Close()
	current, readErr := readProjectDocumentRoot(target)
	if readErr == nil && current.Digest == stage.Document.Digest {
		if err := syncDirectory(target, "."); err != nil {
			return ProjectDocument{}, false, err
		}
		_ = DiscardProjectDocumentStage(root, stage.ProjectID, stage.RequestDigest)
		return current, false, nil
	}
	if readErr == nil {
		switch stage.Kind {
		case ProjectDocumentCreate, ProjectDocumentMove:
			return ProjectDocument{}, false, ErrProjectFolderConflict
		case ProjectDocumentReplace:
			if current.Digest != stage.ExpectedDocumentDigest {
				return ProjectDocument{}, false, ErrProjectDocumentChanged
			}
		default:
			return ProjectDocument{}, false, errors.New("invalid Project document stage kind")
		}
	} else if !errors.Is(readErr, os.ErrNotExist) {
		return ProjectDocument{}, false, readErr
	} else if stage.Kind == ProjectDocumentReplace {
		return ProjectDocument{}, false, ErrProjectDocumentChanged
	}
	if err := publishProjectDocument(target, stage); err != nil {
		return ProjectDocument{}, false, err
	}
	_ = DiscardProjectDocumentStage(root, stage.ProjectID, stage.RequestDigest)
	return stage.Document, false, nil
}

// DiscardProjectDocumentStage removes only one exact central stage.
func DiscardProjectDocumentStage(root *os.Root, projectID, requestDigest string) error {
	project, request, err := openProjectStageRoot(root, projectID, requestDigest)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if err := removeKnownStageFiles(request); err != nil {
		_ = request.Close()
		_ = project.Close()
		return err
	}
	if err := request.Close(); err != nil {
		_ = project.Close()
		return err
	}
	if err := project.Remove(requestDigest); err != nil {
		_ = project.Close()
		return err
	}
	if err := syncDirectory(project, "."); err != nil {
		_ = project.Close()
		return err
	}
	return project.Close()
}

// ProjectDocumentStages enumerates complete central stages for startup recovery.
func ProjectDocumentStages(root *os.Root) ([]ProjectDocumentStage, error) {
	pending, err := ensureRealRoot(root, projectPendingRoot)
	if err != nil {
		return nil, err
	}
	defer pending.Close()
	projects, err := fs.ReadDir(pending.FS(), ".")
	if err != nil {
		return nil, err
	}
	stages := make([]ProjectDocumentStage, 0)
	for _, projectEntry := range projects {
		projectID := "project:" + projectEntry.Name()
		if !projectEntry.IsDir() || projectEntry.Type()&os.ModeSymlink != 0 {
			return nil, fmt.Errorf("pending Project entry %q is not a real directory", projectEntry.Name())
		}
		if _, err := projectName(projectID); err != nil {
			return nil, err
		}
		project, err := openRealRoot(pending, projectEntry.Name())
		if err != nil {
			return nil, err
		}
		requests, err := fs.ReadDir(project.FS(), ".")
		_ = project.Close()
		if err != nil {
			return nil, err
		}
		for _, requestEntry := range requests {
			if strings.HasPrefix(requestEntry.Name(), ".new-") {
				continue
			}
			stage, err := ReadProjectDocumentStage(root, projectID, requestEntry.Name())
			if err != nil {
				return nil, err
			}
			stages = append(stages, stage)
		}
	}
	return stages, nil
}

// ReadProjectDocument reads one exact current Project document.
func ReadProjectDocument(root *os.Root, projectID string, folder *string) (ProjectDocument, error) {
	target, err := openProjectDocumentRoot(root, projectID, folder, false)
	if err != nil {
		return ProjectDocument{}, err
	}
	defer target.Close()
	return readProjectDocumentRoot(target)
}

func stageProjectDocument(root *os.Root, stage ProjectDocumentStage) (ProjectDocumentStage, error) {
	if root == nil || !validProjectDigest(stage.RequestDigest) {
		return ProjectDocumentStage{}, errors.New("invalid Project document stage")
	}
	if _, err := projectName(stage.ProjectID); err != nil {
		return ProjectDocumentStage{}, err
	}
	checked, err := checkedProjectDocument(stage.Document.Content)
	if err != nil {
		return ProjectDocumentStage{}, err
	}
	stage.Document = checked
	if !validProjectStageMetadata(stage) {
		return ProjectDocumentStage{}, errors.New("invalid Project document stage kind")
	}
	pending, project, err := ensureProjectStageRoot(root, stage.ProjectID)
	if err != nil {
		return ProjectDocumentStage{}, err
	}
	defer pending.Close()
	defer project.Close()
	if existing, err := ReadProjectDocumentStage(root, stage.ProjectID, stage.RequestDigest); err == nil {
		if !equalProjectStage(existing, stage) {
			return ProjectDocumentStage{}, errors.New("Project document stage key has different content")
		}
		return existing, nil
	} else if !errors.Is(err, os.ErrNotExist) {
		return ProjectDocumentStage{}, err
	}
	temporary := fmt.Sprintf(".new-%s-%d-%d", stage.RequestDigest, os.Getpid(), projectTemporarySequence.Add(1))
	if err := project.Mkdir(temporary, 0o700); err != nil {
		return ProjectDocumentStage{}, err
	}
	request, err := openRealRoot(project, temporary)
	if err != nil {
		_ = project.Remove(temporary)
		return ProjectDocumentStage{}, err
	}
	manifest := projectDocumentManifest{ProjectID: stage.ProjectID, RequestDigest: stage.RequestDigest,
		Kind: stage.Kind, ExpectedDocumentDigest: stage.ExpectedDocumentDigest,
		DocumentDigest: stage.Document.Digest}
	encoded, err := json.Marshal(manifest)
	if err == nil {
		err = writeProjectStageFile(request, projectManifestName, string(encoded))
	}
	if err == nil {
		err = writeProjectStageFile(request, projectDocumentName, stage.Document.Content)
	}
	if err == nil {
		err = syncDirectory(request, ".")
	}
	_ = request.Close()
	if err != nil {
		_ = discardStageDirectory(project, temporary)
		return ProjectDocumentStage{}, err
	}
	if err := project.Rename(temporary, stage.RequestDigest); err != nil {
		_ = discardStageDirectory(project, temporary)
		if existing, readErr := ReadProjectDocumentStage(root, stage.ProjectID, stage.RequestDigest); readErr == nil && equalProjectStage(existing, stage) {
			return existing, nil
		}
		return ProjectDocumentStage{}, err
	}
	if err := syncDirectory(project, "."); err != nil {
		return ProjectDocumentStage{}, err
	}
	return stage, nil
}

func publishProjectDocument(target *os.Root, stage ProjectDocumentStage) error {
	name := ".PROJECT.md.publish-" + stage.RequestDigest
	file, err := target.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if errors.Is(err, os.ErrExist) {
		existing, readErr := readNamedProjectDocument(target, name)
		if readErr != nil || existing.Digest != stage.Document.Digest {
			return errors.New("Project publication temporary has different content")
		}
	} else if err != nil {
		return err
	} else {
		if written, writeErr := io.WriteString(file, stage.Document.Content); writeErr != nil || written != len(stage.Document.Content) {
			if writeErr == nil {
				writeErr = io.ErrShortWrite
			}
			_ = file.Close()
			_ = target.Remove(name)
			return writeErr
		}
		if err := file.Sync(); err != nil {
			_ = file.Close()
			_ = target.Remove(name)
			return err
		}
		if err := file.Close(); err != nil {
			_ = target.Remove(name)
			return err
		}
	}
	if err := target.Rename(name, projectDocumentName); err != nil {
		return fmt.Errorf("publish Project document: %w", err)
	}
	if err := syncDirectory(target, "."); err != nil {
		return fmt.Errorf("sync published Project document: %w", err)
	}
	return nil
}

// ReadProjectDocumentStage loads one exact central stage for receipt replay.
func ReadProjectDocumentStage(root *os.Root, projectID, requestDigest string) (ProjectDocumentStage, error) {
	project, request, err := openProjectStageRoot(root, projectID, requestDigest)
	if err != nil {
		return ProjectDocumentStage{}, err
	}
	defer project.Close()
	defer request.Close()
	manifestBytes, err := readBoundedRegularFile(request, projectManifestName, 4096)
	if err != nil {
		return ProjectDocumentStage{}, errors.New("invalid Project document stage manifest")
	}
	var manifest projectDocumentManifest
	if err := json.Unmarshal(manifestBytes, &manifest); err != nil {
		return ProjectDocumentStage{}, errors.New("invalid Project document stage manifest")
	}
	document, err := readProjectDocumentRoot(request)
	if err != nil {
		return ProjectDocumentStage{}, err
	}
	stage := ProjectDocumentStage{ProjectID: manifest.ProjectID, RequestDigest: manifest.RequestDigest,
		Kind: manifest.Kind, ExpectedDocumentDigest: manifest.ExpectedDocumentDigest, Document: document}
	if manifest.ProjectID != projectID || manifest.RequestDigest != requestDigest ||
		manifest.DocumentDigest != document.Digest || !validProjectDigest(requestDigest) ||
		!validProjectStageMetadata(stage) {
		return ProjectDocumentStage{}, errors.New("Project document stage manifest does not match its path")
	}
	return stage, nil
}

func readBoundedRegularFile(root *os.Root, name string, limit int64) ([]byte, error) {
	before, err := root.Lstat(name)
	if err != nil || !before.Mode().IsRegular() || before.Size() > limit {
		return nil, errors.New("file is not one bounded regular file")
	}
	file, err := root.Open(name)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !opened.Mode().IsRegular() || !os.SameFile(before, opened) {
		return nil, errors.New("file changed while opening")
	}
	data, err := io.ReadAll(io.LimitReader(file, limit+1))
	if err != nil || int64(len(data)) > limit {
		return nil, errors.New("file exceeds its limit")
	}
	return data, nil
}

func ensureProjectStageRoot(root *os.Root, projectID string) (*os.Root, *os.Root, error) {
	name, err := projectName(projectID)
	if err != nil {
		return nil, nil, err
	}
	pending, err := ensureRealRoot(root, projectPendingRoot)
	if err != nil {
		return nil, nil, err
	}
	if err := pending.Mkdir(name, 0o700); err == nil {
		if err := syncDirectory(pending, "."); err != nil {
			_ = pending.Close()
			return nil, nil, err
		}
	} else if !errors.Is(err, os.ErrExist) {
		_ = pending.Close()
		return nil, nil, err
	}
	project, err := openRealRoot(pending, name)
	if err != nil {
		_ = pending.Close()
		return nil, nil, err
	}
	return pending, project, nil
}

func openProjectStageRoot(root *os.Root, projectID, requestDigest string) (*os.Root, *os.Root, error) {
	if !validProjectDigest(requestDigest) {
		return nil, nil, errors.New("invalid Project request digest")
	}
	name, err := projectName(projectID)
	if err != nil {
		return nil, nil, err
	}
	pending, err := openRealRoot(root, projectPendingRoot)
	if err != nil {
		return nil, nil, err
	}
	project, err := openRealRoot(pending, name)
	_ = pending.Close()
	if err != nil {
		return nil, nil, err
	}
	request, err := openRealRoot(project, requestDigest)
	if err != nil {
		_ = project.Close()
		return nil, nil, err
	}
	return project, request, nil
}

func removeKnownStageFiles(request *os.Root) error {
	entries, err := fs.ReadDir(request.FS(), ".")
	if err != nil {
		return err
	}
	if len(entries) != 2 {
		return errors.New("Project document stage contains unexpected entries")
	}
	for _, name := range []string{projectManifestName, projectDocumentName} {
		if err := request.Remove(name); err != nil {
			return err
		}
	}
	return nil
}

func discardStageDirectory(project *os.Root, name string) error {
	request, err := openRealRoot(project, name)
	if err != nil {
		return err
	}
	if err := removeKnownStageFiles(request); err != nil {
		_ = request.Close()
		return err
	}
	if err := request.Close(); err != nil {
		return err
	}
	return project.Remove(name)
}

func writeProjectStageFile(root *os.Root, name, content string) error {
	file, err := root.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	if written, err := io.WriteString(file, content); err != nil || written != len(content) {
		_ = file.Close()
		if err == nil {
			err = io.ErrShortWrite
		}
		return err
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return err
	}
	return file.Close()
}

func openProjectDocumentRoot(root *os.Root, projectID string, folder *string, create bool) (*os.Root, error) {
	if root == nil {
		return nil, errors.New("home is unavailable")
	}
	name, err := projectName(projectID)
	if err != nil {
		return nil, err
	}
	if folder != nil {
		return openExternalProjectRoot(*folder, create)
	}
	return openRealPathRoot(root, filepath.Join("workspaces", "personal", "projects", name, "docs"), create)
}

func openExternalProjectRoot(folder string, create bool) (*os.Root, error) {
	if folder == "" || strings.TrimSpace(folder) != folder || strings.ContainsRune(folder, 0) || !filepath.IsAbs(folder) {
		return nil, ErrInvalidProjectFolder
	}
	clean := filepath.Clean(folder)
	volume := filepath.VolumeName(clean)
	base := volume + string(os.PathSeparator)
	root, err := os.OpenRoot(base)
	if err != nil {
		return nil, err
	}
	path := strings.TrimPrefix(clean, base)
	if path == "" {
		return root, nil
	}
	opened, err := openRealPathRoot(root, path, create)
	_ = root.Close()
	return opened, err
}

func openRealPathRoot(root *os.Root, path string, create bool) (*os.Root, error) {
	current := root
	owned := false
	for _, part := range strings.FieldsFunc(filepath.ToSlash(path), func(r rune) bool { return r == '/' }) {
		if create {
			if err := current.Mkdir(part, 0o700); err == nil {
				if err := syncDirectory(current, "."); err != nil {
					if owned {
						_ = current.Close()
					}
					return nil, err
				}
			} else if !errors.Is(err, os.ErrExist) {
				if owned {
					_ = current.Close()
				}
				return nil, err
			}
		}
		next, err := openRealRoot(current, part)
		if owned {
			_ = current.Close()
		}
		if err != nil {
			return nil, err
		}
		current, owned = next, true
	}
	return current, nil
}

func ensureRealRoot(root *os.Root, name string) (*os.Root, error) {
	if root == nil {
		return nil, errors.New("home is unavailable")
	}
	if err := root.Mkdir(name, 0o700); err == nil {
		if err := syncDirectory(root, "."); err != nil {
			return nil, err
		}
	} else if !errors.Is(err, os.ErrExist) {
		return nil, err
	}
	return openRealRoot(root, name)
}

func readProjectDocumentRoot(root *os.Root) (ProjectDocument, error) {
	return readNamedProjectDocument(root, projectDocumentName)
}

func readNamedProjectDocument(root *os.Root, name string) (ProjectDocument, error) {
	before, err := root.Lstat(name)
	if err != nil {
		return ProjectDocument{}, err
	}
	if !before.Mode().IsRegular() {
		return ProjectDocument{}, errors.New("Project document is not a regular file")
	}
	file, err := root.Open(name)
	if err != nil {
		return ProjectDocument{}, err
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !opened.Mode().IsRegular() || !os.SameFile(before, opened) {
		return ProjectDocument{}, errors.New("Project document changed while opening")
	}
	if opened.Size() > projectDocumentMax {
		return ProjectDocument{}, errors.New("Project document exceeds the 64 KiB limit")
	}
	data, err := io.ReadAll(io.LimitReader(file, projectDocumentMax+1))
	if err != nil {
		return ProjectDocument{}, err
	}
	if len(data) > projectDocumentMax || !utf8.Valid(data) {
		return ProjectDocument{}, errors.New("Project document is not bounded UTF-8")
	}
	return digestProjectDocument(string(data)), nil
}

func checkedProjectDocument(content string) (ProjectDocument, error) {
	if len(content) > projectDocumentMax {
		return ProjectDocument{}, fmt.Errorf("%w: document exceeds the 64 KiB limit", ErrInvalidProjectDocument)
	}
	if !utf8.ValidString(content) {
		return ProjectDocument{}, fmt.Errorf("%w: document is not valid UTF-8", ErrInvalidProjectDocument)
	}
	return digestProjectDocument(content), nil
}

func digestProjectDocument(content string) ProjectDocument {
	digest := sha256.Sum256([]byte(content))
	return ProjectDocument{Content: content, Digest: hex.EncodeToString(digest[:])}
}

func validProjectDigest(value string) bool {
	if len(value) != 64 || value != strings.ToLower(value) {
		return false
	}
	decoded, err := hex.DecodeString(value)
	return err == nil && len(decoded) == sha256.Size
}

func validProjectStageMetadata(stage ProjectDocumentStage) bool {
	switch stage.Kind {
	case ProjectDocumentCreate:
		return stage.ExpectedDocumentDigest == ""
	case ProjectDocumentReplace, ProjectDocumentMove:
		return validProjectDigest(stage.ExpectedDocumentDigest)
	default:
		return false
	}
}

func projectName(projectID string) (string, error) {
	value, ok := strings.CutPrefix(projectID, "project:")
	if !ok || len(value) != 32 {
		return "", errors.New("invalid Project identifier")
	}
	decoded, err := hex.DecodeString(value)
	if err != nil || hex.EncodeToString(decoded) != value {
		return "", errors.New("invalid Project identifier")
	}
	return value, nil
}

func equalProjectStage(left, right ProjectDocumentStage) bool {
	return left.ProjectID == right.ProjectID && left.RequestDigest == right.RequestDigest &&
		left.Kind == right.Kind && left.ExpectedDocumentDigest == right.ExpectedDocumentDigest &&
		left.Document == right.Document
}

func equalString(left, right *string) bool {
	return left == nil && right == nil || left != nil && right != nil && *left == *right
}
