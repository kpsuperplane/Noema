// Package adapter owns reviewed HTTP API definitions and credential-free connections.
package adapter

import (
	"encoding/json"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	DefinitionTemplateTool = "adapter.definition_template"
	ProposeDefinitionTool  = "adapter.propose_definition"
)

// Manifest is one closed reviewed HTTP API definition.
type Manifest struct {
	SchemaVersion      int            `json:"schema_version"`
	DefinitionID       string         `json:"definition_id"`
	AdapterID          string         `json:"adapter_id"`
	DisplayName        string         `json:"display_name,omitempty"`
	DefinitionRevision string         `json:"definition_revision"`
	Reviewed           bool           `json:"reviewed"`
	Origin             string         `json:"origin"`
	Authentication     Authentication `json:"authentication"`
	Operations         []Operation    `json:"operations"`
}

type Authentication struct {
	Kind string `json:"kind"`
}

type Authorization struct {
	Kind              string     `json:"kind"`
	AcceptedScopeSets [][]string `json:"accepted_scope_sets,omitempty"`
}

type Operation struct {
	OperationID       string            `json:"operation_id"`
	Description       string            `json:"description"`
	SourceDescription string            `json:"source_description,omitempty"`
	Method            string            `json:"method"`
	Path              string            `json:"path"`
	Authorization     Authorization     `json:"authorization"`
	FixedHeaders      map[string]string `json:"fixed_headers,omitempty"`
	FixedQuery        map[string]string `json:"fixed_query,omitempty"`
	Arguments         []Argument        `json:"arguments,omitempty"`
	JSONBodyTemplate  any               `json:"json_body_template,omitempty"`
	Behavior          BehaviorHints     `json:"behavior"`
	Retry             string            `json:"retry"`
	Pagination        Pagination        `json:"pagination"`
	Response          Response          `json:"response"`
}

type Argument struct {
	Name        string   `json:"name"`
	Description string   `json:"description"`
	Location    string   `json:"location"`
	Type        string   `json:"type"`
	Required    bool     `json:"required,omitempty"`
	EnumValues  []string `json:"enum_values,omitempty"`
}

type Hint struct {
	Value  *bool   `json:"value"`
	Source *string `json:"source"`
}

type BehaviorHints struct {
	ReadOnly    Hint `json:"readOnly"`
	Idempotent  Hint `json:"idempotent"`
	Destructive Hint `json:"destructive"`
	OpenWorld   Hint `json:"openWorld"`
}

type Pagination struct {
	Kind            string    `json:"kind"`
	ResponsePointer string    `json:"response_pointer,omitempty"`
	RequestArgument string    `json:"request_argument,omitempty"`
	PageSize        *PageSize `json:"page_size,omitempty"`
}

type PageSize struct {
	RequestArgument string `json:"request_argument"`
	Value           int    `json:"value"`
}

type Response struct {
	AcceptedContentTypes []string     `json:"accepted_content_types"`
	Transform            *Transform   `json:"transform,omitempty"`
	OutputSchema         OutputSchema `json:"output_schema"`
}

type Transform struct {
	Language string `json:"language"`
	Source   string `json:"source"`
}

type OutputSchema struct {
	Type                 string                  `json:"type"`
	Properties           map[string]OutputSchema `json:"properties,omitempty"`
	Required             []string                `json:"required,omitempty"`
	AdditionalProperties *bool                   `json:"additionalProperties,omitempty"`
	Items                *OutputSchema           `json:"items,omitempty"`
	MaxBytes             *int                    `json:"maxBytes,omitempty"`
	MaxItems             *int                    `json:"maxItems,omitempty"`
}

// Definition is one checked filesystem definition.
type Definition struct {
	Manifest            Manifest
	SourceReference     string
	SemanticDigest      string
	Operations          []CompiledOperation
	Replaces            []string
	AffectedConnections []string
	Superseded          bool
}

type CompiledOperation struct {
	Operation
	Digest      string
	InputSchema json.RawMessage
	Behavior    store.ActionBehavior
}

// Connection is one credential-free connection authority.
type Connection struct {
	SchemaVersion      int                          `json:"schema_version"`
	ConnectionID       string                       `json:"connection_id"`
	ConnectionSlug     string                       `json:"connection_slug"`
	ConnectionLabel    string                       `json:"connection_label,omitempty"`
	SemanticDigest     string                       `json:"semantic_digest"`
	Status             string                       `json:"status"`
	ConnectionRevision int                          `json:"connection_revision"`
	PolicyRevision     int                          `json:"policy_revision"`
	DataSharingPolicy  string                       `json:"data_sharing_policy,omitempty"`
	UnsafeActionPolicy string                       `json:"unsafe_action_policy,omitempty"`
	AllowedOperations  []string                     `json:"allowed_operations"`
	Overrides          map[string]OperationOverride `json:"operation_overrides,omitempty"`
}

type OperationOverride struct {
	Enabled        bool                  `json:"enabled"`
	PolicyRevision int                   `json:"policy_revision"`
	SourceRevision string                `json:"source_revision"`
	Behavior       *store.ActionBehavior `json:"behavior,omitempty"`
}

// Binding is one immutable model-visible adapter operation authority.
type Binding struct {
	Name, Description, ConnectionID, DefinitionID          string
	SemanticDigest, OperationID, OperationDigest           string
	ConnectionRevision, PolicyRevision, ToolPolicyRevision int
	InputSchema                                            json.RawMessage
	Behavior                                               store.ActionBehavior
	ReviewRoute                                            store.ActionReviewRoute
}

func GenerationTools(bindings []Binding) []provider.GenerationTool {
	result := make([]provider.GenerationTool, len(bindings))
	for i, binding := range bindings {
		result[i] = provider.GenerationTool{Name: binding.Name, Description: binding.Description, InputSchema: append(json.RawMessage(nil), binding.InputSchema...)}
	}
	return result
}

type DefinitionTransition struct {
	AddedOperations, ChangedOperations, RemovedOperations []string `json:"-"`
}

type ServiceSnapshot struct {
	Definitions []Definition
	Connections []Connection
}

type Cursor struct {
	Reference, ConnectionID, SemanticDigest, OperationID, OperationDigest, ArgumentsDigest, Token string
	ConnectionRevision                                                                            int
	ExpiresAt                                                                                     time.Time
}
