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

// TaskDocument is one exact Task document and its SHA-256 digest.
type TaskDocument struct {
	Content string
	Digest  string
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
