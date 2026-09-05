package model

import (
	"encoding/json"
	"io"
)

// JSONValue preserves any JSON shape for a field that does not require an object.
type JSONValue struct {
	Value any
}

func (value JSONValue) MarshalGQL(writer io.Writer) {
	_ = json.NewEncoder(writer).Encode(value.Value)
}

func (value *JSONValue) UnmarshalGQL(input any) error {
	value.Value = input
	return nil
}
