// Package adapter owns reviewed HTTP API definitions and connections.
package adapter

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	DefinitionTemplateTool  = "adapter.definition_template"
	ProposeDefinitionTool   = "adapter.propose_definition"
	AdapterInvokerKey       = "adapter_json_v1"
	DefinitionTemplateToken = "adapter-setup-v1:definition-template"
	ProposeDefinitionToken  = "adapter-setup-v1:propose-definition"
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
	Kind          string           `json:"kind"`
	ProfileDigest string           `json:"profile_digest,omitempty"`
	Setup         *CredentialSetup `json:"setup,omitempty"`
	RequestAuth   *Transform       `json:"request_auth,omitempty"`
}

func (a *Authentication) UnmarshalJSON(raw []byte) error {
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(raw, &fields); err != nil {
		return err
	}
	var kind string
	if err := json.Unmarshal(fields["kind"], &kind); err != nil {
		return err
	}
	if kind == "none" {
		if len(fields) != 1 {
			return errors.New("adapter authentication is invalid")
		}
		a.Kind = kind
		return nil
	}
	if kind == "oauth2_authorization_code_pkce" {
		if len(fields) != 2 || json.Unmarshal(fields["profile_digest"], &a.ProfileDigest) != nil {
			return errors.New("adapter authentication is invalid")
		}
		a.Kind = kind
		return nil
	}
	if kind != "credential" || len(fields) != 3 || fields["setup"] == nil || fields["request_auth"] == nil {
		return errors.New("adapter authentication is invalid")
	}
	var setup CredentialSetup
	var request Transform
	if decodeExactJSON(fields["setup"], &setup) != nil || decodeExactJSON(fields["request_auth"], &request) != nil {
		return errors.New("adapter authentication is invalid")
	}
	*a = Authentication{Kind: kind, Setup: &setup, RequestAuth: &request}
	return nil
}

type CredentialSetup struct {
	CredentialType string          `json:"credential_type"`
	SetupURL       string          `json:"setup_url"`
	Instructions   []string        `json:"instructions"`
	Input          CredentialInput `json:"input"`
}

type CredentialInput struct {
	Kind      string            `json:"kind"`
	MediaType string            `json:"media_type,omitempty"`
	Fields    []CredentialField `json:"fields"`
	Normalize *Transform        `json:"normalize,omitempty"`
}

func (i *CredentialInput) UnmarshalJSON(raw []byte) error {
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(raw, &fields); err != nil {
		return err
	}
	var kind string
	if err := json.Unmarshal(fields["kind"], &kind); err != nil {
		return err
	}
	var values []CredentialField
	if err := json.Unmarshal(fields["fields"], &values); err != nil {
		return err
	}
	if kind == "fields" {
		if len(fields) != 2 {
			return errors.New("adapter credential input is invalid")
		}
		*i = CredentialInput{Kind: kind, Fields: values}
		return nil
	}
	if kind != "document" || len(fields) != 4 {
		return errors.New("adapter credential input is invalid")
	}
	var media string
	var normalize Transform
	if json.Unmarshal(fields["media_type"], &media) != nil || decodeExactJSON(fields["normalize"], &normalize) != nil {
		return errors.New("adapter credential input is invalid")
	}
	*i = CredentialInput{Kind: kind, MediaType: media, Fields: values, Normalize: &normalize}
	return nil
}

type CredentialField struct {
	ID    string `json:"id"`
	Label string `json:"label"`
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
	SourceDigest        string
	SourceFormat        string
	SemanticDigest      string
	Operations          []CompiledOperation
	Replaces            []string
	AffectedConnections []string
	Superseded          bool
}

// ReviewStatus is the projection-safe review state used by setup callers.
func (d Definition) ReviewStatus() string {
	if d.Superseded {
		return "superseded"
	}
	if d.Manifest.Reviewed {
		return "reviewed"
	}
	return "pending"
}

type CompiledOperation struct {
	Operation
	Digest string
	// Token is the bounded, model-visible definition authority. Invocation
	// wraps it with connection and authentication revisions before use.
	Token       string
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
	Authentication     ConnectionAuthentication     `json:"authentication"`
}

type ConnectionAuthentication struct {
	Kind         string `json:"kind"`
	GenerationID string `json:"generation_id,omitempty"`
	GrantID      string `json:"grant_id,omitempty"`
	Revision     int    `json:"revision,omitempty"`
}

type CredentialInputValue struct {
	FieldValues map[string]string
	Document    []byte
}

type OperationOverride struct {
	Enabled        bool                  `json:"enabled"`
	PolicyRevision int                   `json:"policy_revision"`
	SourceRevision string                `json:"source_revision"`
	Behavior       *store.ActionBehavior `json:"behavior,omitempty"`
}

// Binding is one immutable model-visible adapter operation authority.
type Binding struct {
	Name, Description, ConnectionID, DefinitionID string
	SemanticDigest, OperationID, OperationDigest  string
	// InvokerKey and OperationToken identify the exact runtime authority used
	// by the adapter catalog and invoker boundary.
	InvokerKey, OperationToken                             string
	ConnectionRevision, PolicyRevision, ToolPolicyRevision int
	CredentialRevision                                     int
	GrantID, AccountID                                     string
	InputSchema                                            json.RawMessage
	Behavior                                               store.ActionBehavior
	ReviewRoute                                            store.ActionReviewRoute
}

// Invocation is the adapter invoker boundary. The caller must provide the
// catalog's exact invoker key and operation authority token.
type Invocation struct {
	InvokerKey     string
	Operation      string
	OperationToken string
	Arguments      json.RawMessage
}

const operationAuthorityVersion = 1

// operationAuthorityV1 is the opaque catalog authority carried with one
// invocation. It binds the model-visible operation to every current revision.
type operationAuthorityV1 struct {
	AccountID              string `json:"account_id,omitempty"`
	CanonicalName          string `json:"canonical_name"`
	ConnectionID           string `json:"connection_id"`
	ConnectionRevision     int    `json:"connection_revision"`
	ConnectionSlug         string `json:"connection_slug"`
	CredentialRevision     int    `json:"credential_revision,omitempty"`
	DefinitionToken        string `json:"definition_token"`
	GrantAuthorityRevision int    `json:"grant_authority_revision,omitempty"`
	GrantID                string `json:"grant_id,omitempty"`
	OperationDigest        string `json:"operation_digest"`
	OperationID            string `json:"operation_id"`
	PolicyRevision         int    `json:"policy_revision"`
	SemanticDigest         string `json:"semantic_digest"`
	Version                int    `json:"version"`
}

func makeOperationAuthority(binding Binding, connectionSlug string) (string, error) {
	authority := operationAuthorityV1{
		Version:            operationAuthorityVersion,
		CanonicalName:      binding.Name,
		ConnectionID:       binding.ConnectionID,
		ConnectionSlug:     connectionSlug,
		AccountID:          binding.AccountID,
		SemanticDigest:     binding.SemanticDigest,
		OperationID:        binding.OperationID,
		OperationDigest:    binding.OperationDigest,
		DefinitionToken:    binding.OperationToken,
		ConnectionRevision: binding.ConnectionRevision,
		PolicyRevision:     binding.PolicyRevision,
	}
	if binding.GrantID != "" {
		authority.GrantID = binding.GrantID
		authority.GrantAuthorityRevision = binding.CredentialRevision
	} else if binding.CredentialRevision != 0 {
		authority.CredentialRevision = binding.CredentialRevision
	}
	raw, err := json.Marshal(authority)
	if err != nil || len(raw) > maximumAuthorityTokenBytes {
		return "", errors.New("adapter operation authority is invalid")
	}
	return string(raw), nil
}

func parseOperationAuthority(raw string) (operationAuthorityV1, error) {
	if len(raw) == 0 || len(raw) > maximumAuthorityTokenBytes {
		return operationAuthorityV1{}, errors.New("adapter operation authority is invalid")
	}
	var authority operationAuthorityV1
	if decodeExactJSON([]byte(raw), &authority) != nil || authority.Version != operationAuthorityVersion || authority.CanonicalName == "" || authority.ConnectionID == "" || authority.ConnectionSlug == "" || !validDigest(authority.SemanticDigest) || !validDigest(authority.OperationDigest) || authority.DefinitionToken == "" || authority.ConnectionRevision <= 0 || authority.PolicyRevision <= 0 || authority.GrantID != "" && authority.GrantAuthorityRevision <= 0 || authority.GrantID == "" && authority.GrantAuthorityRevision != 0 || authority.CredentialRevision != 0 && authority.GrantID != "" {
		return operationAuthorityV1{}, errors.New("adapter operation authority is invalid")
	}
	var canonical operationAuthorityV1
	if json.Unmarshal([]byte(raw), &canonical) != nil {
		return operationAuthorityV1{}, errors.New("adapter operation authority is invalid")
	}
	canonicalRaw, _ := json.Marshal(canonical)
	if !bytes.Equal(canonicalRaw, []byte(raw)) {
		return operationAuthorityV1{}, errors.New("adapter operation authority is invalid")
	}
	return authority, nil
}

// SetupBinding describes one setup tool at the catalog boundary.
type SetupBinding struct {
	Tool              provider.GenerationTool
	InvokerKey        string
	OperationToken    string
	ExecutionDecision string
}

// PersistArguments and PersistOutput apply the setup binding's ordinary
// persistence boundary. They preserve schema metadata while removing secret
// values before proposal material is retained or logged.
func (b SetupBinding) PersistArguments(value any) any { return sanitizeProposalPayload(value) }
func (b SetupBinding) PersistOutput(value any) any    { return sanitizeProposalPayload(value) }

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

// AvailabilityNotice explains why one reviewed operation is absent from the
// callable catalog.
type AvailabilityNotice struct {
	Name, Status, DefinitionDigest, OperationID string
}

type Cursor struct {
	Reference, ConnectionID, SemanticDigest, OperationID, OperationDigest, ArgumentsDigest, Token string
	GrantID, AccountID                                                                            string
	ConnectionRevision, GrantRevision                                                             int
	ExpiresAt                                                                                     time.Time
}

// GoString prevents the continuation token from entering diagnostics.
func (c Cursor) GoString() string {
	return fmt.Sprintf("adapter.Cursor{Reference:%q, ConnectionID:%q, GrantID:%q, AccountID:%q, ConnectionRevision:%d, GrantRevision:%d, SemanticDigest:%q, OperationID:%q, OperationDigest:%q, ArgumentsDigest:%q, Token:%q, ExpiresAt:%s}", c.Reference, c.ConnectionID, c.GrantID, c.AccountID, c.ConnectionRevision, c.GrantRevision, c.SemanticDigest, c.OperationID, c.OperationDigest, c.ArgumentsDigest, "[REDACTED]", c.ExpiresAt.UTC().Format(time.RFC3339Nano))
}
