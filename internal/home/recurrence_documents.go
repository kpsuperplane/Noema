package home

import (
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"strings"
	"sync/atomic"
	"unicode/utf8"
)

const recurrenceRootName = "recurrences"

var recurrenceTemporarySequence atomic.Uint64

// ErrRecurrenceDocumentChanged means the expected template digest is stale.
var ErrRecurrenceDocumentChanged = errors.New("recurrence Task document changed elsewhere")

// ReadRecurrenceDocument reads one recurring TASK.md authority.
func ReadRecurrenceDocument(root *os.Root, recurrenceID string) (TaskDocument, error) {
	directory, err := openRecurrenceRoot(root, recurrenceID, false)
	if err != nil {
		return TaskDocument{}, err
	}
	defer directory.Close()
	return readTaskDocumentFile(directory)
}

// EnsureRecurrenceDocument creates one template without replacing existing authority.
func EnsureRecurrenceDocument(root *os.Root, recurrenceID, content string) (TaskDocument, error) {
	current, err := ReadRecurrenceDocument(root, recurrenceID)
	if err == nil {
		return current, nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return TaskDocument{}, err
	}
	return WriteRecurrenceDocument(root, recurrenceID, content, nil)
}

// WriteRecurrenceDocument atomically replaces one template after an optional digest fence.
func WriteRecurrenceDocument(
	root *os.Root, recurrenceID, content string, expectedDigest *string,
) (TaskDocument, error) {
	if len(content) > taskDocumentLimit || !utf8.ValidString(content) {
		return TaskDocument{}, errors.New("invalid recurrence Task document")
	}
	directory, err := openRecurrenceRoot(root, recurrenceID, true)
	if err != nil {
		return TaskDocument{}, err
	}
	defer directory.Close()
	if err := removeRecurrenceTemporaries(directory); err != nil {
		return TaskDocument{}, err
	}
	current, readErr := readTaskDocumentFile(directory)
	if expectedDigest != nil {
		if readErr != nil || current.Digest != *expectedDigest {
			return TaskDocument{}, ErrRecurrenceDocumentChanged
		}
	} else if readErr != nil && !errors.Is(readErr, os.ErrNotExist) {
		return TaskDocument{}, readErr
	}
	if info, statErr := directory.Lstat(taskDocumentName); statErr == nil && !info.Mode().IsRegular() {
		return TaskDocument{}, errors.New("recurrence Task document is not a regular file")
	} else if statErr != nil && !errors.Is(statErr, os.ErrNotExist) {
		return TaskDocument{}, statErr
	}
	temporary := fmt.Sprintf(".TASK.md-%d-%d", os.Getpid(), recurrenceTemporarySequence.Add(1))
	file, err := directory.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return TaskDocument{}, err
	}
	written, writeErr := io.WriteString(file, content)
	if writeErr == nil && written != len(content) {
		writeErr = io.ErrShortWrite
	}
	if writeErr == nil {
		writeErr = file.Sync()
	}
	closeErr := file.Close()
	if writeErr != nil || closeErr != nil {
		_ = directory.Remove(temporary)
		return TaskDocument{}, errors.Join(writeErr, closeErr)
	}
	if err := directory.Rename(temporary, taskDocumentName); err != nil {
		_ = directory.Remove(temporary)
		return TaskDocument{}, err
	}
	if err := syncDirectory(directory, "."); err != nil {
		return TaskDocument{}, err
	}
	return taskDocument(content), nil
}

// CopyRecurrenceDocumentToTask gives one occurrence an immutable template snapshot.
func CopyRecurrenceDocumentToTask(root *os.Root, recurrenceID, taskID string) error {
	template, err := ReadRecurrenceDocument(root, recurrenceID)
	if err != nil {
		return err
	}
	_, readErr := ReadTaskDocument(root, taskID)
	if readErr == nil {
		return nil
	}
	if !errors.Is(readErr, os.ErrNotExist) {
		return readErr
	}
	if _, err := CreatePendingTaskDocument(root, taskID, template.Content); err != nil {
		return err
	}
	return CommitTaskDocument(root, taskID)
}

// DeleteRecurrenceDocument removes one obsolete template.
func DeleteRecurrenceDocument(root *os.Root, recurrenceID string) error {
	if root == nil {
		return errors.New("home is unavailable")
	}
	name, err := recurrenceName(recurrenceID)
	if err != nil {
		return err
	}
	recurrences, err := openRealRoot(root, recurrenceRootName)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	defer recurrences.Close()
	directory, err := openRealRoot(recurrences, name)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if err := directory.Remove(taskDocumentName); err != nil && !errors.Is(err, os.ErrNotExist) {
		_ = directory.Close()
		return err
	}
	if err := removeRecurrenceTemporaries(directory); err != nil {
		_ = directory.Close()
		return err
	}
	if err := directory.Close(); err != nil {
		return err
	}
	if err := recurrences.Remove(name); err != nil {
		return err
	}
	return syncDirectory(recurrences, ".")
}

func openRecurrenceRoot(root *os.Root, id string, create bool) (*os.Root, error) {
	if root == nil {
		return nil, errors.New("home is unavailable")
	}
	name, err := recurrenceName(id)
	if err != nil {
		return nil, err
	}
	var recurrences *os.Root
	if create {
		recurrences, err = ensureDirectory(root, recurrenceRootName)
	} else {
		recurrences, err = openRealRoot(root, recurrenceRootName)
	}
	if err != nil {
		return nil, err
	}
	defer recurrences.Close()
	if create {
		if err := recurrences.Mkdir(name, 0o700); err != nil && !errors.Is(err, os.ErrExist) {
			return nil, err
		}
	}
	return openRealRoot(recurrences, name)
}

func recurrenceName(id string) (string, error) {
	name, ok := strings.CutPrefix(id, "recurrence:")
	if !ok || name == "" || strings.ContainsAny(name, `/\`) || strings.ContainsRune(name, 0) ||
		name == "." || name == ".." {
		return "", errors.New("invalid recurrence id")
	}
	return name, nil
}

func readTaskDocumentFile(root *os.Root) (TaskDocument, error) {
	if _, err := root.Lstat(taskDocumentName); err != nil {
		return TaskDocument{}, err
	}
	data, err := readBoundedRegularFile(root, taskDocumentName, taskDocumentLimit)
	if err != nil {
		return TaskDocument{}, err
	}
	if !utf8.Valid(data) {
		return TaskDocument{}, errors.New("Task document is not valid UTF-8")
	}
	return taskDocument(string(data)), nil
}

func removeRecurrenceTemporaries(root *os.Root) error {
	entries, err := fs.ReadDir(root.FS(), ".")
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if !strings.HasPrefix(entry.Name(), ".TASK.md-") {
			continue
		}
		if !entry.Type().IsRegular() {
			return errors.New("recurrence publication temporary is not a regular file")
		}
		if err := root.Remove(entry.Name()); err != nil {
			return err
		}
	}
	return nil
}
