package store

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"unicode"
)

const maxTaskDomainIDBytes = 255

// TaskID is the validated opaque identifier for one Task.
type TaskID string

// ParseTaskID validates one persisted Task identifier.
func ParseTaskID(value string) (TaskID, error) {
	validated, err := parseTaskDomainID(value, "task", "task:")
	return TaskID(validated), err
}

// String returns the stable wire representation.
func (id TaskID) String() string { return string(id) }

// MarshalJSON writes a validated Task identifier as an opaque string.
func (id TaskID) MarshalJSON() ([]byte, error) {
	if _, err := ParseTaskID(id.String()); err != nil {
		return nil, err
	}
	return json.Marshal(id.String())
}

// UnmarshalJSON reads and validates an opaque Task identifier.
func (id *TaskID) UnmarshalJSON(data []byte) error {
	var value string
	if err := json.Unmarshal(data, &value); err != nil {
		return fmt.Errorf("invalid task id: %w", err)
	}
	validated, err := ParseTaskID(value)
	if err != nil {
		return err
	}
	*id = validated
	return nil
}

// WorkflowID is the validated opaque identifier for one workflow.
type WorkflowID string

// ParseWorkflowID validates one persisted Workflow identifier.
func ParseWorkflowID(value string) (WorkflowID, error) {
	validated, err := parseTaskDomainID(value, "workflow", "workflow:")
	return WorkflowID(validated), err
}

// String returns the stable wire representation.
func (id WorkflowID) String() string { return string(id) }

// MarshalJSON writes a validated Workflow identifier as an opaque string.
func (id WorkflowID) MarshalJSON() ([]byte, error) {
	if _, err := ParseWorkflowID(id.String()); err != nil {
		return nil, err
	}
	return json.Marshal(id.String())
}

// UnmarshalJSON reads and validates an opaque Workflow identifier.
func (id *WorkflowID) UnmarshalJSON(data []byte) error {
	var value string
	if err := json.Unmarshal(data, &value); err != nil {
		return fmt.Errorf("invalid workflow id: %w", err)
	}
	validated, err := ParseWorkflowID(value)
	if err != nil {
		return err
	}
	*id = validated
	return nil
}

func parseTaskDomainID(value, kind, prefix string) (string, error) {
	if strings.TrimSpace(value) == "" {
		return "", fmt.Errorf("invalid %s id: identifier cannot be blank", kind)
	}
	if len(value) > maxTaskDomainIDBytes {
		return "", fmt.Errorf("invalid %s id: identifier exceeds 255 UTF-8 bytes", kind)
	}
	for _, character := range value {
		if unicode.IsControl(character) {
			return "", fmt.Errorf("invalid %s id: identifier cannot contain control characters", kind)
		}
	}
	if !strings.HasPrefix(value, prefix) {
		return "", fmt.Errorf("invalid %s id: identifier must start with %s", kind, prefix)
	}
	if strings.TrimSpace(strings.TrimPrefix(value, prefix)) == "" {
		return "", fmt.Errorf("invalid %s id: identifier must contain a value after its prefix", kind)
	}
	return value, nil
}

// WorkEventPayload is one validated typed event payload.
//
// Its fields stay private so callers cannot pair arbitrary metadata with a
// recurrence event without passing the schema check.
type WorkEventPayload struct {
	kind  string
	value map[string]any
}

// Value returns the validated JSON-shaped payload.
func (payload WorkEventPayload) Value() map[string]any {
	return cloneTaskDomainMap(payload.value)
}

// NewRecurrenceChangedPayload constructs the closed recurrence payload.
func NewRecurrenceChangedPayload(recurrenceID string, revision uint64, reason string) (WorkEventPayload, error) {
	value := map[string]any{
		"v":             uint64(1),
		"recurrence_id": recurrenceID,
		"revision":      revision,
		"reason":        reason,
	}
	return parseRecurrenceChangedPayload(value)
}

// ParseRecurrenceChangedPayload validates a persisted recurrence payload.
func ParseRecurrenceChangedPayload(value map[string]any) (WorkEventPayload, error) {
	return parseRecurrenceChangedPayload(value)
}

func parseRecurrenceChangedPayload(value map[string]any) (WorkEventPayload, error) {
	if value == nil {
		return WorkEventPayload{}, errors.New("invalid recurrence payload: payload must be an object")
	}
	if len(value) != 4 {
		return WorkEventPayload{}, errors.New("invalid recurrence payload: payload fields do not match recurrence.changed")
	}
	for _, field := range []string{"v", "recurrence_id", "revision", "reason"} {
		if _, ok := value[field]; !ok {
			return WorkEventPayload{}, errors.New("invalid recurrence payload: payload fields do not match recurrence.changed")
		}
	}
	encoded, err := json.Marshal(value)
	if err != nil {
		return WorkEventPayload{}, fmt.Errorf("invalid recurrence payload: payload is not encodable: %w", err)
	}
	if len(encoded) > 16*1024 {
		return WorkEventPayload{}, errors.New("invalid recurrence payload: payload exceeds the 16 KiB ledger limit")
	}
	var parsed struct {
		Version      uint64 `json:"v"`
		RecurrenceID string `json:"recurrence_id"`
		Revision     uint64 `json:"revision"`
		Reason       string `json:"reason"`
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&parsed); err != nil {
		return WorkEventPayload{}, fmt.Errorf("invalid recurrence payload: %w", err)
	}
	if parsed.Version != 1 {
		return WorkEventPayload{}, errors.New("invalid recurrence payload: payload must contain numeric v=1")
	}
	if parsed.Revision == 0 {
		return WorkEventPayload{}, errors.New("invalid recurrence payload: revision must be positive")
	}
	if _, err := parseTaskDomainID(parsed.RecurrenceID, "recurrence", "recurrence:"); err != nil {
		return WorkEventPayload{}, err
	}
	return WorkEventPayload{kind: "recurrence.changed", value: map[string]any{
		"v":             parsed.Version,
		"recurrence_id": parsed.RecurrenceID,
		"revision":      parsed.Revision,
		"reason":        parsed.Reason,
	}}, nil
}

func cloneTaskDomainMap(value map[string]any) map[string]any {
	result := make(map[string]any, len(value))
	for key, item := range value {
		result[key] = item
	}
	return result
}
