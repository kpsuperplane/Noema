// Package diagnostics writes bounded developer error records.
package diagnostics

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/home"
)

const (
	// MaxRecordBytes is the largest complete JSONL record.
	MaxRecordBytes = 4 * 1024
	// MaxFileBytes bounds errors.log without a rotation process.
	MaxFileBytes  = 8 * 1024 * 1024
	maxFields     = 6
	maxValueBytes = 512
)

// Field is one safe value selected by the emitting subsystem.
type Field struct{ name, value string }

// Text creates one diagnostic field.
func Text(name, value string) Field {
	if len(value) > maxValueBytes {
		value = value[:maxValueBytes]
		for !utf8.ValidString(value) {
			value = value[:len(value)-1]
		}
	}
	return Field{name: name, value: value}
}

// Writer appends complete records to one private file.
type Writer struct {
	mu   sync.Mutex
	file *os.File
}

// Open creates or opens errors.log.
func Open(path string) (*Writer, error) {
	directory, name := filepath.Split(path)
	if name == "" {
		return nil, errors.New("diagnostic path must name a file")
	}
	root, err := os.OpenRoot(directory)
	if err != nil {
		return nil, fmt.Errorf("open diagnostic directory: %w", err)
	}
	defer root.Close()
	file, err := root.OpenFile(name, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open diagnostic file: %w", err)
	}
	if err := home.ProtectFile(path); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("protect diagnostic file: %w", err)
	}
	return &Writer{file: file}, nil
}

// Write appends one bounded JSONL record.
func (w *Writer) Write(event string, fields ...Field) error {
	if w == nil {
		return nil
	}
	if !validName(event, true) || len(fields) > maxFields {
		return errors.New("diagnostic record is invalid")
	}
	values := make(map[string]string, len(fields))
	for _, field := range fields {
		if !validName(field.name, false) {
			return errors.New("diagnostic field is invalid")
		}
		values[field.name] = field.value
	}
	record, err := json.Marshal(struct {
		Time   string            `json:"time"`
		Event  string            `json:"event"`
		Fields map[string]string `json:"fields,omitempty"`
	}{time.Now().UTC().Format(time.RFC3339Nano), event, values})
	if err != nil {
		return fmt.Errorf("encode diagnostic record: %w", err)
	}
	record = append(record, '\n')
	if len(record) > MaxRecordBytes {
		return fmt.Errorf("diagnostic record exceeds %d bytes", MaxRecordBytes)
	}
	w.mu.Lock()
	defer w.mu.Unlock()
	info, err := w.file.Stat()
	if err != nil {
		return fmt.Errorf("inspect diagnostic file: %w", err)
	}
	if info.Size()+int64(len(record)) > MaxFileBytes {
		return fmt.Errorf("diagnostic file exceeds %d bytes", MaxFileBytes)
	}
	if _, err := w.file.Write(record); err != nil {
		return fmt.Errorf("write diagnostic record: %w", err)
	}
	return nil
}

// Close closes errors.log.
func (w *Writer) Close() error {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.file.Close()
}

func validName(value string, dotted bool) bool {
	if value == "" || len(value) > 64 || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for _, character := range value[1:] {
		if character >= 'a' && character <= 'z' || character >= '0' && character <= '9' ||
			character == '_' || dotted && (character == '.' || character == '-') {
			continue
		}
		return false
	}
	return true
}
