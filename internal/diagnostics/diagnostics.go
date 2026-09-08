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

	"github.com/kpsuperplane/noema/internal/home"
)

const (
	MaxRecordBytes = 256 * 1024
	MaxFileBytes   = 48 * 1024 * 1024
	maxRawBytes    = 64 * 1024
)

// Field is one value selected by the emitting subsystem.
type Field struct{ name, value string }

// Text preserves an ordinary diagnostic value.
func Text(name, value string) Field { return Field{name: name, value: value} }

// Event contains one structured error record.
type Event struct {
	Timestamp  string   `json:"timestamp"`
	Severity   string   `json:"severity"`
	Category   string   `json:"category"`
	Message    string   `json:"message"`
	Context    any      `json:"context"`
	ErrorChain []string `json:"error_chain"`
	Raw        any      `json:"raw"`
}

// Writer appends complete records to one private file.
type Writer struct{ path string }

var writeMu sync.Mutex

// Open validates the log path. The first accepted record creates the file.
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
	if info, err := root.Stat(name); err == nil && info.IsDir() {
		return nil, errors.New("diagnostic path must name a file")
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return nil, err
	}
	return &Writer{path: path}, nil
}

// Write preserves selected fields in the structured event context.
func (w *Writer) Write(category string, fields ...Field) error {
	context := make(map[string]string, len(fields))
	for _, field := range fields {
		context[field.name] = field.value
	}
	return w.WriteEvent(Event{Category: category, Context: context})
}

// WriteEvent appends one bounded structured error record.
func (w *Writer) WriteEvent(event Event) error {
	return w.writeWithLimits(event, MaxFileBytes, MaxRecordBytes)
}

func (w *Writer) writeWithLimits(event Event, maxFileBytes, maxRecordBytes int) error {
	if w == nil {
		return nil
	}
	if event.Timestamp == "" {
		event.Timestamp = time.Now().UTC().Format(time.RFC3339Nano)
	}
	event.Severity = "error"
	if event.Context == nil {
		event.Context = map[string]any{}
	}
	if event.ErrorChain == nil {
		event.ErrorChain = []string{}
	}
	if event.Raw == nil {
		event.Raw = map[string]any{}
	}
	raw, err := json.Marshal(event.Raw)
	if err != nil {
		return fmt.Errorf("encode raw diagnostic data: %w", err)
	}
	if len(raw) > maxRawBytes {
		event.Raw = map[string]any{"truncated": true, "original_bytes": len(raw)}
	}
	record, err := json.Marshal(event)
	if err != nil {
		return fmt.Errorf("encode diagnostic record: %w", err)
	}
	record = append(record, '\n')
	if len(record) > maxRecordBytes {
		return fmt.Errorf("system error event is %d bytes; maximum is %d", len(record), maxRecordBytes)
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	directory, name := filepath.Split(w.path)
	root, err := os.OpenRoot(directory)
	if err != nil {
		return err
	}
	defer root.Close()
	info, err := root.Stat(name)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if err == nil && info.Mode().IsRegular() {
		if info.Size() > int64(maxFileBytes) {
			if err := root.Remove(name); err != nil {
				return err
			}
		} else if info.Size()+int64(len(record)) > int64(maxFileBytes) {
			if err := root.Remove(name + ".1"); err != nil && !errors.Is(err, os.ErrNotExist) {
				return err
			}
			if err := root.Rename(name, name+".1"); err != nil {
				return err
			}
			if err := home.ProtectFile(w.path + ".1"); err != nil {
				return err
			}
		}
	}
	file, err := root.OpenFile(name, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	defer file.Close()
	if err := home.ProtectFile(w.path); err != nil {
		return err
	}
	_, err = file.Write(record)
	return err
}

// Close completes the writer. Each append closes its file before returning.
func (w *Writer) Close() error { return nil }
