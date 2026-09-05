package home

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"strings"
	"unicode/utf8"
)

const (
	taskDocumentName  = "TASK.md"
	taskDocumentLimit = 64 * 1024
	taskRootName      = "tasks"
	pendingRootName   = ".pending"
)

// ErrTaskDocumentStageStale means a newer committed document replaced this stage base.
var ErrTaskDocumentStageStale = errors.New("Task document stage is stale")

// TaskDocument is one exact Task document and its SHA-256 digest.
type TaskDocument struct {
	Content string
	Digest  string
}

// TaskDocumentStage is one durable replacement prepared before its database command.
type TaskDocumentStage struct {
	TaskID, RequestDigest, ExpectedDigest string
	Document                              TaskDocument
}

// PrepareTaskDocumentReplace checks the current digest and saves one replacement.
func PrepareTaskDocumentReplace(root *os.Root, taskID, expectedDigest, content, requestDigest string) (TaskDocumentStage, error) {
	current, err := ReadTaskDocument(root, taskID)
	if err != nil {
		return TaskDocumentStage{}, err
	}
	if current.Digest != expectedDigest {
		return TaskDocumentStage{}, errors.New("Task document changed")
	}
	if !validTaskDocumentDigest(requestDigest) || len(content) > taskDocumentLimit || !utf8.ValidString(content) {
		return TaskDocumentStage{}, errors.New("invalid Task document replacement")
	}
	stage := TaskDocumentStage{TaskID: taskID, RequestDigest: requestDigest,
		ExpectedDigest: expectedDigest, Document: taskDocument(content)}
	task, err := openTaskDocumentRoot(root, taskID)
	if err != nil {
		return TaskDocumentStage{}, err
	}
	defer task.Close()
	name := taskDocumentStageName(requestDigest)
	file, err := task.OpenFile(name, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if errors.Is(err, os.ErrExist) {
		existing, readErr := readBoundedRegularFile(task, name, taskDocumentLimit)
		if readErr != nil || taskDocument(string(existing)) != stage.Document {
			return TaskDocumentStage{}, errors.New("Task document stage has different content")
		}
	} else if err != nil {
		return TaskDocumentStage{}, err
	} else {
		if written, writeErr := io.WriteString(file, content); writeErr != nil || written != len(content) {
			_ = file.Close()
			_ = task.Remove(name)
			if writeErr == nil {
				writeErr = io.ErrShortWrite
			}
			return TaskDocumentStage{}, writeErr
		}
		if err := file.Sync(); err != nil {
			_ = file.Close()
			return TaskDocumentStage{}, err
		}
		if err := file.Close(); err != nil {
			return TaskDocumentStage{}, err
		}
	}
	if err := syncDirectory(task, "."); err != nil {
		return TaskDocumentStage{}, err
	}
	baseName := taskDocumentStageBaseName(requestDigest)
	base, err := task.OpenFile(baseName, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if errors.Is(err, os.ErrExist) {
		stored, readErr := readBoundedRegularFile(task, baseName, 64)
		if readErr != nil || string(stored) != expectedDigest {
			return TaskDocumentStage{}, errors.New("Task document stage has a different base")
		}
	} else if err != nil {
		return TaskDocumentStage{}, err
	} else {
		if _, err = io.WriteString(base, expectedDigest); err == nil {
			err = base.Sync()
		}
		if closeErr := base.Close(); err == nil {
			err = closeErr
		}
		if err != nil {
			return TaskDocumentStage{}, err
		}
	}
	if err := syncDirectory(task, "."); err != nil {
		return TaskDocumentStage{}, err
	}
	return stage, nil
}

// CommitTaskDocumentStage atomically publishes one prepared replacement.
func CommitTaskDocumentStage(root *os.Root, stage TaskDocumentStage) (TaskDocument, error) {
	if !validTaskDocumentDigest(stage.RequestDigest) || stage.Document != taskDocument(stage.Document.Content) {
		return TaskDocument{}, errors.New("invalid Task document stage")
	}
	task, err := openTaskDocumentRoot(root, stage.TaskID)
	if err != nil {
		return TaskDocument{}, err
	}
	defer task.Close()
	current, err := readTaskDocumentFile(task)
	if err != nil {
		return TaskDocument{}, err
	}
	name := taskDocumentStageName(stage.RequestDigest)
	if current.Digest == stage.Document.Digest {
		_ = task.Remove(name)
		_ = task.Remove(taskDocumentStageBaseName(stage.RequestDigest))
		return current, nil
	}
	if current.Digest != stage.ExpectedDigest {
		return TaskDocument{}, errors.New("Task document changed before publication")
	}
	staged, err := readBoundedRegularFile(task, name, taskDocumentLimit)
	if err != nil || taskDocument(string(staged)) != stage.Document {
		return TaskDocument{}, errors.New("Task document stage is unavailable")
	}
	if err := task.Rename(name, taskDocumentName); err != nil {
		return TaskDocument{}, err
	}
	_ = task.Remove(taskDocumentStageBaseName(stage.RequestDigest))
	if err := syncDirectory(task, "."); err != nil {
		return TaskDocument{}, err
	}
	return stage.Document, nil
}

// RecoverTaskDocumentStage publishes or verifies one committed receipt result.
func RecoverTaskDocumentStage(root *os.Root, taskID, requestDigest, documentDigest string) (TaskDocument, error) {
	current, err := ReadTaskDocument(root, taskID)
	if err == nil && current.Digest == documentDigest {
		_ = DiscardTaskDocumentStage(root, taskID, requestDigest)
		return current, nil
	}
	if err != nil {
		return TaskDocument{}, err
	}
	task, openErr := openTaskDocumentRoot(root, taskID)
	if openErr != nil {
		return TaskDocument{}, errors.Join(err, openErr)
	}
	defer task.Close()
	name := taskDocumentStageName(requestDigest)
	base, baseErr := readBoundedRegularFile(task, taskDocumentStageBaseName(requestDigest), 64)
	if baseErr != nil || current.Digest != string(base) {
		return TaskDocument{}, ErrTaskDocumentStageStale
	}
	staged, readErr := readBoundedRegularFile(task, name, taskDocumentLimit)
	if readErr != nil || taskDocument(string(staged)).Digest != documentDigest {
		return TaskDocument{}, errors.New("committed Task document stage is unavailable")
	}
	if renameErr := task.Rename(name, taskDocumentName); renameErr != nil {
		return TaskDocument{}, renameErr
	}
	_ = task.Remove(taskDocumentStageBaseName(requestDigest))
	if syncErr := syncDirectory(task, "."); syncErr != nil {
		return TaskDocument{}, syncErr
	}
	return taskDocument(string(staged)), nil
}

// DiscardTaskDocumentStage removes one uncommitted replacement.
func DiscardTaskDocumentStage(root *os.Root, taskID, requestDigest string) error {
	task, err := openTaskDocumentRoot(root, taskID)
	if err != nil {
		return err
	}
	defer task.Close()
	if err := task.Remove(taskDocumentStageName(requestDigest)); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if err := task.Remove(taskDocumentStageBaseName(requestDigest)); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return syncDirectory(task, ".")
}

// ReconcileTaskDocumentStages publishes committed replacements after an interrupted command.
func ReconcileTaskDocumentStages(root *os.Root, limit int, receipt func(string, string) (string, bool, error)) error {
	if root == nil || limit < 1 || receipt == nil {
		return errors.New("Task document stage recovery is unavailable")
	}
	tasks, err := openRealRoot(root, taskRootName)
	if err != nil {
		return err
	}
	defer tasks.Close()
	directory, err := tasks.Open(".")
	if err != nil {
		return err
	}
	defer directory.Close()
	candidates := 0
	for {
		entries, readErr := directory.ReadDir(128)
		if readErr != nil && !errors.Is(readErr, io.EOF) {
			return readErr
		}
		for _, entry := range entries {
			if !entry.IsDir() || entry.Name() == pendingRootName {
				continue
			}
			if artifactTask, ok := strings.CutPrefix(entry.Name(), "task_"); ok {
				if _, err := taskName("task:" + artifactTask); err == nil {
					continue
				}
			}
			taskID := "task:" + entry.Name()
			if _, err := taskName(taskID); err != nil {
				return err
			}
			task, err := openRealRoot(tasks, entry.Name())
			if err != nil {
				return err
			}
			taskDirectory, err := task.Open(".")
			if err != nil {
				_ = task.Close()
				return err
			}
			for {
				files, fileErr := taskDirectory.ReadDir(128)
				if fileErr != nil && !errors.Is(fileErr, io.EOF) {
					_ = taskDirectory.Close()
					_ = task.Close()
					return fileErr
				}
				for _, file := range files {
					digest, found := strings.CutPrefix(file.Name(), ".TASK.md.publish-")
					if !found || strings.HasSuffix(digest, ".base") || !validTaskDocumentDigest(digest) {
						continue
					}
					candidates++
					if candidates > limit {
						_ = taskDirectory.Close()
						_ = task.Close()
						return errors.New("Task document stage recovery limit exceeded")
					}
					documentDigest, committed, err := receipt(taskID, digest)
					if err != nil {
						_ = taskDirectory.Close()
						_ = task.Close()
						return err
					}
					if !committed {
						err = DiscardTaskDocumentStage(root, taskID, digest)
					} else if _, err = RecoverTaskDocumentStage(root, taskID, digest, documentDigest); errors.Is(err, ErrTaskDocumentStageStale) {
						err = DiscardTaskDocumentStage(root, taskID, digest)
					}
					if err != nil {
						_ = taskDirectory.Close()
						_ = task.Close()
						return err
					}
				}
				if errors.Is(fileErr, io.EOF) {
					break
				}
			}
			_ = taskDirectory.Close()
			_ = task.Close()
		}
		if errors.Is(readErr, io.EOF) {
			break
		}
	}
	return nil
}

func openTaskDocumentRoot(root *os.Root, taskID string) (*os.Root, error) {
	name, err := taskName(taskID)
	if err != nil {
		return nil, err
	}
	tasks, err := openRealRoot(root, taskRootName)
	if err != nil {
		return nil, err
	}
	defer tasks.Close()
	return openRealRoot(tasks, name)
}
func taskDocumentStageName(digest string) string     { return ".TASK.md.publish-" + digest }
func taskDocumentStageBaseName(digest string) string { return taskDocumentStageName(digest) + ".base" }
func validTaskDocumentDigest(value string) bool {
	decoded, err := hex.DecodeString(value)
	return err == nil && len(decoded) == 32 && hex.EncodeToString(decoded) == value
}

// CreatePendingTaskDocument writes a durable document before its Task row commits.
func CreatePendingTaskDocument(root *os.Root, taskID string, content string) (TaskDocument, error) {
	if root == nil {
		return TaskDocument{}, errors.New("home is unavailable")
	}
	if len(content) > taskDocumentLimit {
		return TaskDocument{}, errors.New("task document exceeds the 64 KiB limit")
	}
	if !utf8.ValidString(content) {
		return TaskDocument{}, errors.New("task document is not valid UTF-8")
	}
	name, err := taskName(taskID)
	if err != nil {
		return TaskDocument{}, err
	}

	tasks, err := ensureDirectory(root, taskRootName)
	if err != nil {
		return TaskDocument{}, err
	}
	defer tasks.Close()
	pending, err := ensureDirectory(tasks, pendingRootName)
	if err != nil {
		return TaskDocument{}, err
	}
	defer pending.Close()
	if err := pending.Mkdir(name, 0o700); err != nil {
		return TaskDocument{}, fmt.Errorf("create pending Task directory: %w", err)
	}
	task, err := openRealRoot(pending, name)
	if err != nil {
		_ = pending.Remove(name)
		return TaskDocument{}, err
	}

	if err := writeTaskDocument(task, content); err != nil {
		_ = task.Close()
		return TaskDocument{}, errors.Join(err, removeTaskDirectory(pending, name))
	}
	if err := syncDirectory(task, "."); err != nil {
		_ = task.Close()
		return TaskDocument{}, errors.Join(err, removeTaskDirectory(pending, name))
	}
	if err := task.Close(); err != nil {
		return TaskDocument{}, errors.Join(
			fmt.Errorf("close pending Task directory: %w", err),
			removeTaskDirectory(pending, name),
		)
	}
	if err := syncDirectory(pending, "."); err != nil {
		return TaskDocument{}, errors.Join(err, removeTaskDirectory(pending, name))
	}
	if err := syncDirectory(tasks, "."); err != nil {
		return TaskDocument{}, errors.Join(err, removeTaskDirectory(pending, name))
	}
	if err := syncDirectory(root, "."); err != nil {
		return TaskDocument{}, errors.Join(err, removeTaskDirectory(pending, name))
	}
	return taskDocument(content), nil
}

// CommitTaskDocument makes one pending Task document current.
func CommitTaskDocument(root *os.Root, taskID string) error {
	if root == nil {
		return errors.New("home is unavailable")
	}
	name, err := taskName(taskID)
	if err != nil {
		return err
	}
	tasks, err := openRealRoot(root, taskRootName)
	if err != nil {
		return err
	}
	defer tasks.Close()
	pending, err := openRealRoot(tasks, pendingRootName)
	if err != nil {
		return err
	}
	defer pending.Close()
	pendingPath := pendingRootName + string(os.PathSeparator) + name
	if err := tasks.Rename(pendingPath, name); err != nil {
		current, currentErr := readTaskDocumentAt(tasks, name)
		staged, stagedErr := readTaskDocumentAt(pending, name)
		if currentErr == nil && errors.Is(stagedErr, os.ErrNotExist) {
			return nil
		}
		if currentErr == nil && stagedErr == nil && current == staged {
			if removeErr := removeTaskDirectory(pending, name); removeErr != nil {
				return removeErr
			}
			return syncDirectory(pending, ".")
		}
		return fmt.Errorf("commit Task document: %w", errors.Join(err, currentErr, stagedErr))
	}
	if err := syncDirectory(pending, "."); err != nil {
		return err
	}
	if err := syncDirectory(tasks, "."); err != nil {
		return err
	}
	return nil
}

// ReadTaskDocument reads one exact current or interrupted TASK.md file.
func ReadTaskDocument(root *os.Root, taskID string) (TaskDocument, error) {
	if root == nil {
		return TaskDocument{}, errors.New("home is unavailable")
	}
	name, err := taskName(taskID)
	if err != nil {
		return TaskDocument{}, err
	}
	tasks, err := openRealRoot(root, taskRootName)
	if err != nil {
		return TaskDocument{}, err
	}
	defer tasks.Close()
	current, err := readTaskDocumentAt(tasks, name)
	if err == nil {
		return current, nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return TaskDocument{}, err
	}
	pending, pendingErr := openRealRoot(tasks, pendingRootName)
	if pendingErr != nil {
		return TaskDocument{}, errors.Join(err, pendingErr)
	}
	defer pending.Close()
	return readTaskDocumentAt(pending, name)
}

// RecoverTaskDocuments completes or removes interrupted Task creates.
func RecoverTaskDocuments(
	root *os.Root,
	exists func(string) (bool, error),
) error {
	if root == nil || exists == nil {
		return errors.New("task document recovery is unavailable")
	}
	tasks, err := ensureDirectory(root, taskRootName)
	if err != nil {
		return err
	}
	defer tasks.Close()
	pending, err := ensureDirectory(tasks, pendingRootName)
	if err != nil {
		return err
	}
	defer pending.Close()
	entries, err := fs.ReadDir(pending.FS(), ".")
	if err != nil {
		return fmt.Errorf("list pending Task documents: %w", err)
	}
	for _, entry := range entries {
		if !entry.IsDir() || entry.Type()&os.ModeSymlink != 0 {
			return fmt.Errorf("pending Task entry %q is not a real directory", entry.Name())
		}
		taskID := "task:" + entry.Name()
		if _, err := taskName(taskID); err != nil {
			return fmt.Errorf("pending Task entry %q is invalid: %w", entry.Name(), err)
		}
		stored, err := exists(taskID)
		if err != nil {
			return fmt.Errorf("inspect pending Task %s: %w", taskID, err)
		}
		if err := reconcileTaskDocument(tasks, pending, entry.Name(), stored); err != nil {
			return fmt.Errorf("recover Task document %s: %w", taskID, err)
		}
	}
	if len(entries) > 0 {
		if err := syncDirectory(tasks, "."); err != nil {
			return err
		}
		if err := syncDirectory(pending, "."); err != nil {
			return err
		}
	}
	return nil
}

// DiscardPendingTaskDocument removes one staged document after a failed row create.
func DiscardPendingTaskDocument(root *os.Root, taskID string) error {
	if root == nil {
		return errors.New("home is unavailable")
	}
	name, err := taskName(taskID)
	if err != nil {
		return err
	}
	tasks, err := openRealRoot(root, taskRootName)
	if err != nil {
		return err
	}
	defer tasks.Close()
	pending, err := openRealRoot(tasks, pendingRootName)
	if err != nil {
		return err
	}
	defer pending.Close()
	if err := removeTaskDirectory(pending, name); err != nil {
		return err
	}
	return syncDirectory(pending, ".")
}

func reconcileTaskDocument(tasks *os.Root, pending *os.Root, name string, stored bool) error {
	if !stored {
		return removeTaskDirectory(pending, name)
	}
	pendingPath := pendingRootName + string(os.PathSeparator) + name
	if err := tasks.Rename(pendingPath, name); err == nil {
		return nil
	} else {
		current, currentErr := readTaskDocumentAt(tasks, name)
		staged, stagedErr := readTaskDocumentAt(pending, name)
		if currentErr != nil || stagedErr != nil || current != staged {
			return errors.Join(err, currentErr, stagedErr)
		}
		return removeTaskDirectory(pending, name)
	}
}

func writeTaskDocument(root *os.Root, content string) error {
	file, err := root.OpenFile(taskDocumentName, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return fmt.Errorf("create Task document: %w", err)
	}
	if written, err := io.WriteString(file, content); err != nil || written != len(content) {
		if err == nil {
			err = io.ErrShortWrite
		}
		_ = file.Close()
		return fmt.Errorf("write Task document: %w", err)
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return fmt.Errorf("sync Task document: %w", err)
	}
	if err := file.Close(); err != nil {
		return fmt.Errorf("close Task document: %w", err)
	}
	return nil
}

func readTaskDocumentAt(parent *os.Root, name string) (TaskDocument, error) {
	task, err := openRealRoot(parent, name)
	if err != nil {
		return TaskDocument{}, err
	}
	defer task.Close()
	before, err := task.Lstat(taskDocumentName)
	if err != nil {
		return TaskDocument{}, fmt.Errorf("inspect Task document path: %w", err)
	}
	if !before.Mode().IsRegular() {
		return TaskDocument{}, errors.New("task document is not a regular file")
	}
	file, err := task.Open(taskDocumentName)
	if err != nil {
		return TaskDocument{}, fmt.Errorf("open Task document: %w", err)
	}
	defer file.Close()
	openInfo, err := file.Stat()
	if err != nil {
		return TaskDocument{}, fmt.Errorf("inspect open Task document: %w", err)
	}
	pathInfo, err := task.Lstat(taskDocumentName)
	if err != nil {
		return TaskDocument{}, fmt.Errorf("inspect Task document path: %w", err)
	}
	if !openInfo.Mode().IsRegular() || !pathInfo.Mode().IsRegular() ||
		!os.SameFile(before, openInfo) || !os.SameFile(openInfo, pathInfo) {
		return TaskDocument{}, errors.New("task document is not one stable regular file")
	}
	if openInfo.Size() > taskDocumentLimit {
		return TaskDocument{}, errors.New("task document exceeds the 64 KiB limit")
	}
	content, err := io.ReadAll(io.LimitReader(file, taskDocumentLimit+1))
	if err != nil {
		return TaskDocument{}, fmt.Errorf("read Task document: %w", err)
	}
	if len(content) > taskDocumentLimit {
		return TaskDocument{}, errors.New("task document exceeds the 64 KiB limit")
	}
	if !utf8.Valid(content) {
		return TaskDocument{}, errors.New("task document is not valid UTF-8")
	}
	return taskDocument(string(content)), nil
}

func ensureDirectory(parent *os.Root, name string) (*os.Root, error) {
	if err := parent.Mkdir(name, 0o700); err != nil && !errors.Is(err, os.ErrExist) {
		return nil, fmt.Errorf("create Task directory %q: %w", name, err)
	}
	return openRealRoot(parent, name)
}

func openRealRoot(parent *os.Root, name string) (*os.Root, error) {
	before, err := parent.Lstat(name)
	if err != nil {
		return nil, fmt.Errorf("inspect Task directory %q: %w", name, err)
	}
	if !before.IsDir() || before.Mode()&os.ModeSymlink != 0 {
		return nil, fmt.Errorf("task directory %q is not a real directory", name)
	}
	child, err := parent.OpenRoot(name)
	if err != nil {
		return nil, fmt.Errorf("open Task directory %q: %w", name, err)
	}
	opened, err := child.Stat(".")
	if err != nil || !os.SameFile(before, opened) {
		_ = child.Close()
		return nil, fmt.Errorf("task directory %q changed while opening", name)
	}
	return child, nil
}

func removeTaskDirectory(parent *os.Root, name string) error {
	task, err := openRealRoot(parent, name)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}
	if err := task.Remove(taskDocumentName); err != nil && !errors.Is(err, os.ErrNotExist) {
		_ = task.Close()
		return fmt.Errorf("remove Task document: %w", err)
	}
	if err := task.Close(); err != nil {
		return fmt.Errorf("close Task directory before removal: %w", err)
	}
	if err := parent.Remove(name); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("remove Task directory: %w", err)
	}
	return nil
}

func taskName(taskID string) (string, error) {
	value, ok := strings.CutPrefix(taskID, "task:")
	if !ok || len(value) != 32 {
		return "", errors.New("invalid Task identifier")
	}
	decoded, err := hex.DecodeString(value)
	if err != nil || hex.EncodeToString(decoded) != value {
		return "", errors.New("invalid Task identifier")
	}
	return value, nil
}

func taskDocument(content string) TaskDocument {
	digest := sha256.Sum256([]byte(content))
	return TaskDocument{Content: content, Digest: hex.EncodeToString(digest[:])}
}
