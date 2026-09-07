package mcp

import (
	"encoding/json"
	"strings"
)

// BindingPersistencePolicy controls which safe views may be retained for one
// model-visible MCP authority.
type BindingPersistencePolicy string

const (
	BindingPersistenceRaw      BindingPersistencePolicy = "raw"
	BindingPersistenceRedacted BindingPersistencePolicy = "redacted"
	BindingPersistenceOmitted  BindingPersistencePolicy = "omitted"
)

// PersistedBindingViews are the independent durable argument and output views.
type PersistedBindingViews struct {
	Arguments any
	Output    any
}

// PersistedViews applies the binding's configured persistence policy.
func (b Binding) PersistedViews(arguments, output any) PersistedBindingViews {
	return PersistedBindingViews{Arguments: b.persistView(arguments), Output: b.persistView(output)}
}

func (b Binding) persistView(value any) any {
	switch b.PersistencePolicy {
	case BindingPersistenceOmitted:
		return nil
	case BindingPersistenceRedacted:
		return redactBindingValue(cloneBindingValue(value))
	default:
		return cloneBindingValue(value)
	}
}

func cloneBindingValue(value any) any {
	raw, err := json.Marshal(value)
	if err != nil {
		return value
	}
	var clone any
	if err := json.Unmarshal(raw, &clone); err != nil {
		return value
	}
	return clone
}

func redactBindingValue(value any) any {
	switch current := value.(type) {
	case map[string]any:
		for key, child := range current {
			if bindingCredentialFields[strings.ToLower(key)] {
				current[key] = "[REDACTED]"
				continue
			}
			current[key] = redactBindingValue(child)
		}
	case []any:
		for index, child := range current {
			current[index] = redactBindingValue(child)
		}
	}
	return value
}

var bindingCredentialFields = map[string]bool{
	"access_token": true, "api_key": true, "apikey": true, "authorization": true,
	"client_secret": true, "cookie": true, "password": true, "refresh_token": true,
}
