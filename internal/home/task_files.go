package home

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"
)

const (
	taskFileLimit     = 64 * 1024
	taskFilePathLimit = 4096
	taskFileListLimit = 256
)

// TaskFileEntry describes one bounded Task-directory entry.
type TaskFileEntry struct {
	Path        string `json:"path"`
	IsDirectory bool   `json:"is_directory"`
	SizeBytes   *int64 `json:"size_bytes"`
}

// ReadTaskFile reads one bounded UTF-8 file from the Task directory.
func ReadTaskFile(root *os.Root, taskID, supplied string) (string, error) {
	file, err := OpenTaskFile(root, taskID, supplied)
	if err != nil {
		return "", err
	}
	defer file.Close()
	return readBoundedTaskFile(file)
}

// ReadProjectFileForTask reads one file in a linked Project folder from a Task-relative path.
func ReadProjectFileForTask(root *os.Root, taskID, folder, supplied string) (string, error) {
	if root == nil {
		return "", errors.New("Project file is unavailable")
	}
	name, err := taskName(taskID)
	if err != nil || folder == "" {
		return "", errors.New("Project file is unavailable")
	}
	taskPath := filepath.Join(root.Name(), taskRootName, name)
	target := filepath.Clean(filepath.Join(taskPath, supplied))
	relative, err := filepath.Rel(filepath.Clean(folder), target)
	if err != nil || relative == "." || relative == ".." || strings.HasPrefix(relative, ".."+string(os.PathSeparator)) || filepath.IsAbs(relative) {
		return "", errors.New("Project file is outside the Project folder")
	}
	project, err := openExternalProjectRoot(filepath.Clean(folder), false)
	if err != nil {
		return "", errors.New("Project file is unavailable")
	}
	defer project.Close()
	if err := checkTaskFilePath(project, relative, false); err != nil {
		return "", err
	}
	file, err := project.Open(relative)
	if err != nil {
		return "", errors.New("Project file is unavailable")
	}
	defer file.Close()
	return readBoundedTaskFile(file)
}

func readBoundedTaskFile(file *os.File) (string, error) {
	info, err := file.Stat()
	if err != nil || !info.Mode().IsRegular() || info.Size() > taskFileLimit {
		return "", errors.New("Task file is not bounded text")
	}
	data, err := io.ReadAll(io.LimitReader(file, taskFileLimit+1))
	if err != nil || len(data) > taskFileLimit || !utf8.Valid(data) {
		return "", errors.New("Task file is not bounded UTF-8")
	}
	return string(data), nil
}

// OpenTaskFile opens one regular file through the Task path boundary.
func OpenTaskFile(root *os.Root, taskID, supplied string) (*os.File, error) {
	path, err := normalizeTaskFilePath(supplied, false)
	if err != nil {
		return nil, err
	}
	task, err := openTaskDocumentRoot(root, taskID)
	if err != nil {
		return nil, err
	}
	defer task.Close()
	if _, err := task.Lstat(path); errors.Is(err, os.ErrNotExist) {
		return nil, fmt.Errorf("Task file is unavailable: %w", err)
	}
	if err := checkTaskFilePath(task, path, false); err != nil {
		return nil, err
	}
	file, err := task.Open(path)
	if err != nil {
		return nil, fmt.Errorf("Task file is unavailable: %w", err)
	}
	return file, nil
}

// WriteTaskFile atomically replaces one bounded UTF-8 file in the Task directory.
func WriteTaskFile(root *os.Root, taskID, supplied, content string) error {
	path, err := normalizeTaskFilePath(supplied, false)
	if err != nil {
		return err
	}
	if len(content) > taskFileLimit || !utf8.ValidString(content) {
		return errors.New("Task file content is not bounded UTF-8")
	}
	task, err := openTaskDocumentRoot(root, taskID)
	if err != nil {
		return err
	}
	defer task.Close()
	if err := ensureTaskFileParents(task, filepath.Dir(path)); err != nil {
		return err
	}
	if info, inspectErr := task.Lstat(path); inspectErr == nil &&
		(!info.Mode().IsRegular() || info.Mode()&os.ModeSymlink != 0) {
		return errors.New("Task file path is not a regular file")
	} else if inspectErr != nil && !errors.Is(inspectErr, os.ErrNotExist) {
		return errors.New("Task file path is unavailable")
	}
	temporary, err := taskFileTemporaryPath(path)
	if err != nil {
		return errors.New("Task file temporary name is unavailable")
	}
	file, err := task.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return errors.New("Task file temporary file is unavailable")
	}
	removeTemporary := true
	defer func() {
		if removeTemporary {
			_ = task.Remove(temporary)
		}
	}()
	if written, writeErr := io.WriteString(file, content); writeErr != nil || written != len(content) {
		_ = file.Close()
		return errors.New("Task file write failed")
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return errors.New("Task file sync failed")
	}
	if err := file.Close(); err != nil {
		return errors.New("Task file close failed")
	}
	if err := task.Rename(temporary, path); err != nil {
		return errors.New("Task file publication failed")
	}
	removeTemporary = false
	return syncDirectory(task, filepath.Dir(path))
}

// DeleteTaskFile removes one unprotected file from the Task directory.
func DeleteTaskFile(root *os.Root, taskID, supplied string) error {
	path, err := normalizeTaskFilePath(supplied, false)
	if err != nil {
		return err
	}
	if path == taskDocumentName || path == "RESULT.md" {
		return errors.New("Task file is protected")
	}
	task, err := openTaskDocumentRoot(root, taskID)
	if err != nil {
		return err
	}
	defer task.Close()
	if err := checkTaskFilePath(task, path, false); err != nil {
		return err
	}
	if err := task.Remove(path); err != nil {
		return errors.New("Task file could not be deleted")
	}
	return syncDirectory(task, filepath.Dir(path))
}

// ListTaskFiles lists one Task-directory level without following symbolic links.
func ListTaskFiles(root *os.Root, taskID, supplied string) ([]TaskFileEntry, error) {
	path, err := normalizeTaskFilePath(supplied, true)
	if err != nil {
		return nil, err
	}
	task, err := openTaskDocumentRoot(root, taskID)
	if err != nil {
		return nil, err
	}
	defer task.Close()
	if path != "." {
		if err := checkTaskFilePath(task, path, true); err != nil {
			return nil, err
		}
	}
	directory, err := task.Open(path)
	if err != nil {
		return nil, errors.New("Task directory is unavailable")
	}
	defer directory.Close()
	entries, err := directory.ReadDir(taskFileListLimit + 1)
	if err != nil && !errors.Is(err, io.EOF) {
		return nil, errors.New("Task directory could not be listed")
	}
	if len(entries) > taskFileListLimit {
		return nil, errors.New("Task directory contains too many entries")
	}
	result := make([]TaskFileEntry, 0, len(entries))
	for _, entry := range entries {
		if internalTaskFileName(entry.Name()) {
			continue
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return nil, errors.New("Task directory contains a symbolic link")
		}
		info, err := entry.Info()
		if err != nil {
			return nil, errors.New("Task directory entry is unavailable")
		}
		entryPath := filepath.Join(path, entry.Name())
		if path == "." {
			entryPath = entry.Name()
		}
		value := TaskFileEntry{Path: filepath.ToSlash(entryPath), IsDirectory: info.IsDir()}
		if info.Mode().IsRegular() {
			size := info.Size()
			value.SizeBytes = &size
		} else if !info.IsDir() {
			return nil, errors.New("Task directory contains a special file")
		}
		result = append(result, value)
	}
	return result, nil
}

func internalTaskFileName(name string) bool {
	return strings.HasPrefix(name, ".TASK.md.publish-") ||
		strings.HasPrefix(name, ".TASK.md.noema-") || strings.HasSuffix(name, ".tmp") && strings.Contains(name, ".noema-")
}

func normalizeTaskFilePath(supplied string, allowDot bool) (string, error) {
	if len(supplied) > taskFilePathLimit || strings.ContainsRune(supplied, 0) ||
		filepath.IsAbs(supplied) || filepath.VolumeName(supplied) != "" {
		return "", errors.New("Task file path is outside the Task directory")
	}
	for _, part := range strings.Split(filepath.ToSlash(supplied), "/") {
		if part == ".." {
			return "", errors.New("Task file path is outside the Task directory")
		}
	}
	path := filepath.Clean(supplied)
	if path == "." && allowDot {
		return path, nil
	}
	if path == "." || path == "" {
		return "", errors.New("Task file path is required")
	}
	return path, nil
}

func checkTaskFilePath(root *os.Root, path string, directory bool) error {
	current := ""
	parts := strings.Split(filepath.ToSlash(path), "/")
	for index, part := range parts {
		current = filepath.Join(current, part)
		info, err := root.Lstat(current)
		if err != nil {
			return errors.New("Task file path is unavailable")
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return errors.New("Task file path contains a symbolic link")
		}
		last := index == len(parts)-1
		if !last && !info.IsDir() || last && directory && !info.IsDir() || last && !directory && !info.Mode().IsRegular() {
			return errors.New("Task file path has the wrong type")
		}
	}
	return nil
}

func ensureTaskFileParents(root *os.Root, parent string) error {
	if parent == "." {
		return nil
	}
	current := ""
	for _, part := range strings.Split(filepath.ToSlash(parent), "/") {
		current = filepath.Join(current, part)
		info, err := root.Lstat(current)
		switch {
		case err == nil && info.Mode()&os.ModeSymlink != 0:
			return errors.New("Task file parent contains a symbolic link")
		case err == nil && !info.IsDir():
			return errors.New("Task file parent is not a directory")
		case err == nil:
		case errors.Is(err, os.ErrNotExist):
			if err := root.Mkdir(current, 0o700); err != nil {
				return errors.New("Task file parent could not be created")
			}
		default:
			return errors.New("Task file parent is unavailable")
		}
	}
	return nil
}

func taskFileTemporaryPath(path string) (string, error) {
	var value [8]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", err
	}
	return filepath.Join(filepath.Dir(path), fmt.Sprintf(".%s.noema-%s.tmp", filepath.Base(path), hex.EncodeToString(value[:]))), nil
}
