// Package diagnostics writes bounded structured diagnostics.
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
	// MaxRecordBytes is the largest encoded record, including its newline.
	MaxRecordBytes = 4 * 1024
	// MaxFileBytes bounds one diagnostic file.
	MaxFileBytes  = 8 * 1024 * 1024
	maxEventBytes = 64
	maxFieldCount = 32
	maxFieldBytes = 64
	maxValueBytes = 3 * 1024
)

var errDirectSerialization = errors.New("diagnostic values require the bounded writer")

// Value is a closed set of approved diagnostic value categories.
// Credential types cannot implement this interface.
type Value interface {
	diagnosticValue() fieldValue
}

type valueKind uint8

const (
	stringKind valueKind = iota
	labelKind
	integerKind
	booleanKind
)

type fieldValue struct {
	kind    valueKind
	text    string
	integer int64
	boolean bool
}

type safeValue struct {
	value fieldValue
}

func (v safeValue) diagnosticValue() fieldValue {
	return v.value
}

// MarshalJSON prevents values from bypassing the bounded record writer.
func (safeValue) MarshalJSON() ([]byte, error) {
	return nil, errDirectSerialization
}

// Identifier marks an ordinary opaque identifier.
func Identifier(value string) Value {
	return textValue(value)
}

// Path marks an ordinary filesystem path.
func Path(value string) Value {
	return textValue(value)
}

// URL marks an ordinary non-credential URL.
func URL(value string) Value {
	return textValue(value)
}

// ModelName marks an ordinary model name.
func ModelName(value string) Value {
	return textValue(value)
}

// ClientID marks an ordinary non-secret client identifier.
func ClientID(value string) Value {
	return textValue(value)
}

// Label marks a bounded machine label, such as a state or error code.
func Label(value string) Value {
	return safeValue{value: fieldValue{kind: labelKind, text: value}}
}

// Count marks an integer measurement.
func Count(value int64) Value {
	return safeValue{value: fieldValue{kind: integerKind, integer: value}}
}

// Flag marks a Boolean state.
func Flag(value bool) Value {
	return safeValue{value: fieldValue{kind: booleanKind, boolean: value}}
}

func textValue(value string) Value {
	return safeValue{value: fieldValue{kind: stringKind, text: value}}
}

// Field is one named diagnostic value.
type Field struct {
	name  string
	value fieldValue
}

// NewField checks one explicitly classified diagnostic value.
func NewField(name string, value Value) (Field, error) {
	if !validName(name, maxFieldBytes, false) {
		return Field{}, errors.New("diagnostic field name must use lower snake case")
	}
	if value == nil {
		return Field{}, errors.New("diagnostic field value cannot be nil")
	}

	field := Field{name: name, value: value.diagnosticValue()}
	switch field.value.kind {
	case stringKind:
		if !utf8.ValidString(field.value.text) {
			return Field{}, fmt.Errorf("diagnostic field %q is not valid UTF-8", name)
		}
		if len(field.value.text) > maxValueBytes {
			return Field{}, fmt.Errorf("diagnostic field %q exceeds %d bytes", name, maxValueBytes)
		}
	case labelKind:
		if !validName(field.value.text, maxFieldBytes, true) {
			return Field{}, fmt.Errorf("diagnostic field %q has an invalid label", name)
		}
	case integerKind, booleanKind:
	default:
		return Field{}, fmt.Errorf("diagnostic field %q has an invalid type", name)
	}
	return field, nil
}

// MarshalJSON prevents fields from bypassing the bounded record writer.
func (Field) MarshalJSON() ([]byte, error) {
	return nil, errDirectSerialization
}

// Record is one structured diagnostic event.
type Record struct {
	at     time.Time
	event  string
	fields []Field
}

// NewRecord checks one diagnostic event and its fields.
func NewRecord(at time.Time, event string, fields ...Field) (Record, error) {
	if at.IsZero() {
		return Record{}, errors.New("diagnostic time cannot be zero")
	}
	if !validName(event, maxEventBytes, true) {
		return Record{}, errors.New("diagnostic event must use lower dotted form")
	}
	if len(fields) > maxFieldCount {
		return Record{}, fmt.Errorf("diagnostic record exceeds %d fields", maxFieldCount)
	}

	seen := make(map[string]struct{}, len(fields))
	owned := make([]Field, len(fields))
	copy(owned, fields)
	for _, field := range owned {
		if field.name == "" {
			return Record{}, errors.New("diagnostic record contains an empty field")
		}
		if _, exists := seen[field.name]; exists {
			return Record{}, fmt.Errorf("diagnostic field %q is duplicated", field.name)
		}
		seen[field.name] = struct{}{}
	}

	return Record{at: at, event: event, fields: owned}, nil
}

// MarshalJSON prevents records from bypassing size checks.
func (Record) MarshalJSON() ([]byte, error) {
	return nil, errDirectSerialization
}

// Writer appends complete JSONL records to one private file.
type Writer struct {
	mu   sync.Mutex
	file *os.File
}

// Open creates or opens a diagnostic file and restricts its permissions.
func Open(path string) (*Writer, error) {
	if path == "" {
		return nil, errors.New("diagnostic path cannot be empty")
	}
	directory, name := filepath.Split(path)
	if name == "" {
		return nil, errors.New("diagnostic path must name a file")
	}
	if directory == "" {
		directory = "."
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

// Write appends one complete bounded record.
func (w *Writer) Write(record Record) error {
	encoded, err := encode(record)
	if err != nil {
		return err
	}

	w.mu.Lock()
	defer w.mu.Unlock()
	info, err := w.file.Stat()
	if err != nil {
		return fmt.Errorf("inspect diagnostic file: %w", err)
	}
	if info.Size()+int64(len(encoded)) > MaxFileBytes {
		return fmt.Errorf("diagnostic file exceeds %d bytes", MaxFileBytes)
	}
	if _, err := w.file.Write(encoded); err != nil {
		return fmt.Errorf("write diagnostic record: %w", err)
	}
	return nil
}

// Close closes the diagnostic file.
func (w *Writer) Close() error {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.file.Close()
}

type wireRecord struct {
	Time   string         `json:"time"`
	Event  string         `json:"event"`
	Fields map[string]any `json:"fields,omitempty"`
}

func encode(record Record) ([]byte, error) {
	if record.at.IsZero() || !validName(record.event, maxEventBytes, true) {
		return nil, errors.New("diagnostic record was not created by NewRecord")
	}

	fields := make(map[string]any, len(record.fields))
	for _, field := range record.fields {
		switch field.value.kind {
		case stringKind, labelKind:
			fields[field.name] = field.value.text
		case integerKind:
			fields[field.name] = field.value.integer
		case booleanKind:
			fields[field.name] = field.value.boolean
		default:
			return nil, fmt.Errorf("diagnostic field %q has an invalid type", field.name)
		}
	}

	encoded, err := json.Marshal(wireRecord{
		Time:   record.at.UTC().Format(time.RFC3339Nano),
		Event:  record.event,
		Fields: fields,
	})
	if err != nil {
		return nil, fmt.Errorf("encode diagnostic record: %w", err)
	}
	encoded = append(encoded, '\n')
	if len(encoded) > MaxRecordBytes {
		return nil, fmt.Errorf("diagnostic record exceeds %d bytes", MaxRecordBytes)
	}
	return encoded, nil
}

func validName(value string, limit int, dotted bool) bool {
	if value == "" || len(value) > limit || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for _, character := range value[1:] {
		if character >= 'a' && character <= 'z' ||
			character >= '0' && character <= '9' ||
			character == '_' ||
			dotted && (character == '.' || character == '-') {
			continue
		}
		return false
	}
	return true
}
