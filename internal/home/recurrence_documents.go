package home

import (
	"encoding/hex"
	"encoding/json"
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

const recurrencePendingRootName = ".pending-recurrences"

var recurrenceTemporarySequence atomic.Uint64

// RecurrenceDocumentStage binds one document replacement to one Task command.
type RecurrenceDocumentStage struct {
	RecurrenceID           string
	RequestDigest          string
	ExpectedDocumentDigest string
	Document               TaskDocument
}

type recurrenceDocumentStageFile struct {
	RecurrenceID           string `json:"recurrence_id"`
	RequestDigest          string `json:"request_digest"`
	ExpectedDocumentDigest string `json:"expected_document_digest"`
	DocumentDigest         string `json:"document_digest"`
	Content                string `json:"content"`
}

// ErrRecurrenceDocumentChanged means the expected template digest is stale.
var ErrRecurrenceDocumentChanged = errors.New("recurrence Task document changed elsewhere")

// PrepareRecurrenceDocumentReplace checks the live digest and saves one durable replacement.
func PrepareRecurrenceDocumentReplace(
	root *os.Root, recurrenceID, expectedDigest, content, requestDigest string,
) (RecurrenceDocumentStage, error) {
	current, err := ReadRecurrenceDocument(root, recurrenceID)
	if err != nil {
		return RecurrenceDocumentStage{}, err
	}
	if current.Digest != expectedDigest {
		return RecurrenceDocumentStage{}, ErrRecurrenceDocumentChanged
	}
	stage := RecurrenceDocumentStage{RecurrenceID: recurrenceID, RequestDigest: requestDigest,
		ExpectedDocumentDigest: expectedDigest, Document: taskDocument(content)}
	if err := validateRecurrenceStage(stage); err != nil {
		return RecurrenceDocumentStage{}, err
	}
	pending, err := ensureDirectory(root, recurrencePendingRootName)
	if err != nil {
		return RecurrenceDocumentStage{}, err
	}
	defer pending.Close()
	if existing, err := ReadRecurrenceDocumentStage(root, requestDigest); err == nil {
		if !equalRecurrenceStage(existing, stage) {
			return RecurrenceDocumentStage{}, errors.New("recurrence document stage key has different content")
		}
		return existing, nil
	} else if !errors.Is(err, os.ErrNotExist) {
		return RecurrenceDocumentStage{}, err
	}
	stored := recurrenceDocumentStageFile{RecurrenceID: recurrenceID, RequestDigest: requestDigest,
		ExpectedDocumentDigest: expectedDigest, DocumentDigest: stage.Document.Digest, Content: content}
	encoded, err := json.Marshal(stored)
	if err != nil {
		return RecurrenceDocumentStage{}, err
	}
	temporary := fmt.Sprintf(".new-%s-%d-%d", requestDigest, os.Getpid(), recurrenceTemporarySequence.Add(1))
	file, err := pending.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return RecurrenceDocumentStage{}, err
	}
	written, writeErr := file.Write(encoded)
	if writeErr == nil && written != len(encoded) {
		writeErr = io.ErrShortWrite
	}
	if writeErr == nil {
		writeErr = file.Sync()
	}
	closeErr := file.Close()
	if writeErr != nil || closeErr != nil {
		_ = pending.Remove(temporary)
		return RecurrenceDocumentStage{}, errors.Join(writeErr, closeErr)
	}
	name := requestDigest + ".json"
	if err := pending.Rename(temporary, name); err != nil {
		_ = pending.Remove(temporary)
		if existing, readErr := ReadRecurrenceDocumentStage(root, requestDigest); readErr == nil && equalRecurrenceStage(existing, stage) {
			return existing, nil
		}
		return RecurrenceDocumentStage{}, err
	}
	if err := syncDirectory(pending, "."); err != nil {
		return RecurrenceDocumentStage{}, err
	}
	return stage, nil
}

// CommitRecurrenceDocumentStage publishes one replacement and removes its stage.
func CommitRecurrenceDocumentStage(root *os.Root, stage RecurrenceDocumentStage) (TaskDocument, error) {
	stored, err := ReadRecurrenceDocumentStage(root, stage.RequestDigest)
	if err != nil {
		return TaskDocument{}, err
	}
	if !equalRecurrenceStage(stored, stage) {
		return TaskDocument{}, errors.New("recurrence document stage does not match its stored authority")
	}
	current, err := ReadRecurrenceDocument(root, stage.RecurrenceID)
	if err == nil && current.Digest == stage.Document.Digest {
		return current, DiscardRecurrenceDocumentStage(root, stage.RequestDigest)
	}
	if err != nil || current.Digest != stage.ExpectedDocumentDigest {
		return TaskDocument{}, ErrRecurrenceDocumentChanged
	}
	published, err := WriteRecurrenceDocument(root, stage.RecurrenceID, stage.Document.Content,
		&stage.ExpectedDocumentDigest)
	if err != nil {
		return TaskDocument{}, err
	}
	return published, DiscardRecurrenceDocumentStage(root, stage.RequestDigest)
}

// ReadRecurrenceDocumentStage reads one exact durable replacement.
func ReadRecurrenceDocumentStage(root *os.Root, requestDigest string) (RecurrenceDocumentStage, error) {
	if !validRecurrenceStageDigest(requestDigest) {
		return RecurrenceDocumentStage{}, errors.New("invalid recurrence document stage key")
	}
	pending, err := openRealRoot(root, recurrencePendingRootName)
	if err != nil {
		return RecurrenceDocumentStage{}, err
	}
	defer pending.Close()
	if _, err := pending.Lstat(requestDigest + ".json"); err != nil {
		return RecurrenceDocumentStage{}, err
	}
	data, err := readBoundedRegularFile(pending, requestDigest+".json", taskDocumentLimit*7)
	if err != nil {
		return RecurrenceDocumentStage{}, err
	}
	var stored recurrenceDocumentStageFile
	if err := json.Unmarshal(data, &stored); err != nil {
		return RecurrenceDocumentStage{}, err
	}
	stage := RecurrenceDocumentStage{RecurrenceID: stored.RecurrenceID, RequestDigest: stored.RequestDigest,
		ExpectedDocumentDigest: stored.ExpectedDocumentDigest, Document: taskDocument(stored.Content)}
	if stored.RequestDigest != requestDigest || stored.DocumentDigest != stage.Document.Digest {
		return RecurrenceDocumentStage{}, errors.New("recurrence document stage does not match its path or digest")
	}
	return stage, validateRecurrenceStage(stage)
}

// RecurrenceDocumentStages lists complete stages and removes interrupted temporary files.
func RecurrenceDocumentStages(root *os.Root) ([]RecurrenceDocumentStage, error) {
	pending, err := ensureDirectory(root, recurrencePendingRootName)
	if err != nil {
		return nil, err
	}
	defer pending.Close()
	entries, err := fs.ReadDir(pending.FS(), ".")
	if err != nil {
		return nil, err
	}
	stages := make([]RecurrenceDocumentStage, 0, len(entries))
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".new-") && entry.Type().IsRegular() {
			if err := pending.Remove(entry.Name()); err != nil {
				return nil, err
			}
			continue
		}
		requestDigest, ok := strings.CutSuffix(entry.Name(), ".json")
		if !ok || !entry.Type().IsRegular() {
			return nil, fmt.Errorf("invalid recurrence document stage %q", entry.Name())
		}
		stage, err := ReadRecurrenceDocumentStage(root, requestDigest)
		if err != nil {
			return nil, err
		}
		stages = append(stages, stage)
	}
	return stages, syncDirectory(pending, ".")
}

// DiscardRecurrenceDocumentStage removes one exact durable replacement.
func DiscardRecurrenceDocumentStage(root *os.Root, requestDigest string) error {
	if !validRecurrenceStageDigest(requestDigest) {
		return errors.New("invalid recurrence document stage key")
	}
	pending, err := openRealRoot(root, recurrencePendingRootName)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	defer pending.Close()
	if err := pending.Remove(requestDigest + ".json"); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return syncDirectory(pending, ".")
}

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
		return CommitTaskDocument(root, taskID)
	}
	if !errors.Is(readErr, os.ErrNotExist) {
		return readErr
	}
	if _, err := CreatePendingTaskDocument(root, taskID, template.Content); err != nil {
		return err
	}
	return CommitTaskDocument(root, taskID)
}

// StageRecurrenceDocumentToTask saves one occurrence snapshot before its Task row commits.
func StageRecurrenceDocumentToTask(root *os.Root, recurrenceID, taskID string) error {
	template, err := ReadRecurrenceDocument(root, recurrenceID)
	if err != nil {
		return err
	}
	_, err = CreatePendingTaskDocument(root, taskID, template.Content)
	return err
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

func validateRecurrenceStage(stage RecurrenceDocumentStage) error {
	if _, err := recurrenceName(stage.RecurrenceID); err != nil {
		return err
	}
	if !validRecurrenceStageDigest(stage.RequestDigest) ||
		!validRecurrenceStageDigest(stage.ExpectedDocumentDigest) ||
		len(stage.Document.Content) > taskDocumentLimit || !utf8.ValidString(stage.Document.Content) ||
		stage.Document != taskDocument(stage.Document.Content) {
		return errors.New("invalid recurrence document stage")
	}
	return nil
}

func validRecurrenceStageDigest(value string) bool {
	decoded, err := hex.DecodeString(value)
	return err == nil && len(decoded) == 32 && hex.EncodeToString(decoded) == value
}

func equalRecurrenceStage(left, right RecurrenceDocumentStage) bool {
	return left.RecurrenceID == right.RecurrenceID && left.RequestDigest == right.RequestDigest &&
		left.ExpectedDocumentDigest == right.ExpectedDocumentDigest && left.Document == right.Document
}
