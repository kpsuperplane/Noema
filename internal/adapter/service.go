package adapter

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/script"
	"github.com/kpsuperplane/noema/internal/store"
)

var (
	ErrOutcomeUncertain       = errors.New("adapter request outcome is uncertain")
	ErrAuthenticationRequired = errors.New("adapter authentication is required")
	errCursorBindingMismatch  = errors.New("adapter continuation binding mismatch")
	errCursorExpired          = errors.New("adapter continuation expired")
)

// Service owns reviewed adapter files, public indexes, and calls.
type Service struct {
	files            *fileAuthority
	database         *store.Store
	mu               sync.Mutex
	oauthCallback    string
	oauthAttempts    map[string]*oauthAttempt
	oauthEvents      map[string]OAuthAttemptEvent
	oauthSubscribers map[string]map[chan OAuthAttemptEvent]struct{}
	oauthCompleted   func(OAuthAttemptEvent)
	httpClient       *http.Client
	oauthClient      *http.Client
}

// NewService recovers and indexes one fresh Go adapter authority.
func NewService(root *os.Root, database *store.Store) (*Service, error) {
	return newService(root, database, nil)
}

func newService(root *os.Root, database *store.Store, client *http.Client) (*Service, error) {
	if database == nil {
		return nil, errors.New("adapter store is unavailable")
	}
	files, err := newFileAuthority(root)
	if err != nil {
		return nil, err
	}
	service := &Service{files: files, database: database, oauthAttempts: make(map[string]*oauthAttempt), oauthEvents: make(map[string]OAuthAttemptEvent), oauthSubscribers: make(map[string]map[chan OAuthAttemptEvent]struct{}), httpClient: client, oauthClient: client}
	if err := service.reconcile(context.Background()); err != nil {
		return nil, err
	}
	return service, nil
}

// SetupTools returns the two fixed adapter definition tools.
func (s *Service) SetupTools() []provider.GenerationTool {
	return []provider.GenerationTool{
		{Name: DefinitionTemplateTool, Description: "List current API definitions or return one concise revision base.", InputSchema: json.RawMessage(`{"type":"object","properties":{"semantic_digest":{"type":"string","maxLength":64},"operation_ids":{"type":"array","maxItems":32,"items":{"type":"string"}}},"additionalProperties":false}`)},
		{Name: ProposeDefinitionTool, Description: "Propose one public HTTP API definition from official HTTPS documentation for human review. Call adapter.definition_template first. Follow its examples for new definitions and revisions. Never include credentials or private user data.", InputSchema: json.RawMessage(`{"type":"object","properties":{"source_reference":{"type":"string","maxLength":4096},"new_definition":{"type":"object","additionalProperties":true},"base_semantic_digest":{"type":"string"},"revision":{"type":"object","additionalProperties":true},"upsert_operations":{"type":"array","maxItems":128,"items":{"type":"object","additionalProperties":true}},"remove_operation_ids":{"type":"array","maxItems":128,"items":{"type":"string"}}},"required":["source_reference"],"additionalProperties":false}`)},
	}
}

// ExecuteSetup executes one fixed adapter definition tool.
func (s *Service) ExecuteSetup(name string, raw json.RawMessage) (json.RawMessage, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	var value any
	var err error
	switch name {
	case DefinitionTemplateTool:
		value, err = s.definitionTemplate(raw)
	case ProposeDefinitionTool:
		value, err = s.propose(raw)
	default:
		err = errors.New("adapter setup tool is unavailable")
	}
	if err != nil {
		return failure("invalid_input", err.Error()), false
	}
	payload, err := script.MarshalJSON(value)
	if err != nil {
		return failure("unavailable", "Adapter result could not be encoded"), false
	}
	return payload, true
}

// definitionHelp restores the worked examples from the Rust setup tool.
// The current parser and compiler remain the authority for proposal validation.
func definitionHelp() map[string]any {
	return map[string]any{
		"instructions": []string{
			"Replace example values with facts from the service's HTTPS documentation.",
			"For a new API, submit new_definition and complete upsert_operations.",
			"For a revision, load its exact semantic_digest and submit revision with operation changes.",
			"Use a root HTTPS origin with path /. Put every API prefix in operation paths.",
			"Declare read_only, idempotent, destructive, and open_world for each operation.",
			"Use flat_object, object_list, or scalar_list responses when their fields cover the documented response.",
			"Use custom for other responses. Bound each string with maxBytes and each array with maxItems.",
			"Each string in a response recipe needs max_bytes. Use truncate only for display text, never identifiers.",
			"Use language lua for transforms. Each source must return a function.",
			"Credential field input accepts kind and fields only. Document input also requires media_type and normalize.",
			"Request authentication reads input.credentials. Never include credential values in a proposal or Chat.",
			"OAuth authentication uses only kind and profile_digest from oauth_profiles. Do not add profile_id, scopes, setup, or request_auth.",
			"For OAuth, upsert each operation with authorization.kind oauth_scopes and accepted_scope_sets. Retain its other fields from the revision template.",
			"Reuse this template when correcting a proposal.",
		},
		"proposal_template": json.RawMessage(`{
			"source_reference":"https://developers.example.com/api",
			"new_definition":{
				"definition_id":"example_service","adapter_id":"example_service","display_name":"Example Service",
				"definition_revision":"v1","origin":"https://api.example.com/","authentication":{"kind":"none"}
			},
			"upsert_operations":[{
				"operation_id":"list_items","description":"List a page of items.","method":"GET","path":"/v1/items",
				"authorization":{"kind":"none"},"arguments":[],
				"read_only":true,"idempotent":true,"destructive":false,"open_world":true,
				"pagination":{"kind":"none"},
				"response":{"kind":"object_list","source_pointer":"/items","output_name":"items","max_items":4,
					"fields":[{"name":"id","source_pointer":"/id","type":"string","max_bytes":256,"required":true},
						{"name":"name","source_pointer":"/name","type":"string","max_bytes":512,"truncate":true}]}
			}]
		}`),
		"revision_template": json.RawMessage(`{
			"source_reference":"https://developers.example.com/api","base_semantic_digest":"exact digest from revision_base",
			"revision":{"definition_revision":"v2"},"upsert_operations":[],"remove_operation_ids":["removed_operation_id"]
		}`),
		"credential_authentication_example": json.RawMessage(`{
			"kind":"credential",
			"setup":{"credential_type":"API key","setup_url":"https://developers.example.com/api-keys",
				"instructions":["Create an API key and enter it in the protected setup field."],
				"input":{"kind":"fields","fields":[{"id":"api_key","label":"API key"}]}},
			"request_auth":{"language":"lua","source":"return function(input) return {headers={['X-API-Key']=input.credentials.api_key}} end"}
		}`),
		"oauth_authentication_example":          json.RawMessage(`{"kind":"oauth2_authorization_code_pkce","profile_digest":"0000000000000000000000000000000000000000000000000000000000000000"}`),
		"oauth_operation_authorization_example": json.RawMessage(`{"kind":"oauth_scopes","accepted_scope_sets":[["scope.read"]]}`),
		"flat_object_response_example": json.RawMessage(`{
			"kind":"flat_object","fields":[{"name":"id","source_pointer":"/id","type":"string","max_bytes":256,"required":true}]
		}`),
		"scalar_list_response_example": json.RawMessage(`{
			"kind":"scalar_list","source_pointer":"/labels","output_name":"labels","max_items":16,
			"item":{"type":"string","max_bytes":128}
		}`),
		"custom_response_example": json.RawMessage(`{
			"kind":"custom","accepted_content_types":["application/json"],
			"transform":{"language":"lua","source":"return function(response) local body=json.decode(response.body); return {id=body.id} end"},
			"output_schema":{"type":"object","properties":{"id":{"type":"string","maxBytes":256}},"required":["id"],"additionalProperties":false}
		}`),
		"response_token_pagination_example": json.RawMessage(`{
			"kind":"response_token","response_pointer":"/nextPageToken","request_argument":"pageToken",
			"page_size":{"request_argument":"maxResults","value":8}
		}`),
		"argument_example": json.RawMessage(`{
			"name":"query","description":"Search terms.","location":"query","type":"string","required":false
		}`),
		"enums": map[string][]string{
			"authentication.kind":          {"none", "credential", "oauth2_authorization_code_pkce"},
			"argument.location":            {"path", "query", "json_body"},
			"argument.type":                {"string", "integer", "number", "boolean", "string_array"},
			"operation.authorization.kind": {"none", "oauth_scopes"},
		},
	}
}

func (s *Service) definitionTemplate(raw json.RawMessage) (any, error) {
	var input struct {
		SemanticDigest string   `json:"semantic_digest,omitempty"`
		OperationIDs   []string `json:"operation_ids,omitempty"`
	}
	if decodeExactJSON(raw, &input) != nil || len(input.OperationIDs) > 32 {
		return nil, errors.New("adapter definition template arguments are invalid")
	}
	if input.SemanticDigest == "" {
		definitions, err := s.files.definitions()
		if err != nil {
			return nil, err
		}
		values := make([]map[string]any, 0)
		for _, definition := range definitions {
			if !definition.Superseded {
				values = append(values, map[string]any{"semantic_digest": definition.SemanticDigest, "definition_id": definition.Manifest.DefinitionID, "adapter_id": definition.Manifest.AdapterID, "display_name": DisplayName(definition), "definition_revision": definition.Manifest.DefinitionRevision, "reviewed": definition.Manifest.Reviewed})
			}
		}
		if len(values) > 100 {
			values = values[:100]
		}
		help := definitionHelp()
		help["definitions"] = values
		oauth, err := s.files.oauthSnapshot()
		if err != nil {
			return nil, err
		}
		profiles := make([]map[string]string, 0, len(oauth.Profiles))
		for _, profile := range oauth.Profiles {
			profiles = append(profiles, map[string]string{"profile_id": profile.ProfileID, "profile_digest": profile.ProfileDigest, "authorization_endpoint": profile.AuthorizationEndpoint, "token_endpoint": profile.TokenEndpoint})
		}
		help["oauth_profiles"] = profiles
		return help, nil
	}
	definitions, err := s.files.definitions()
	if err != nil {
		return nil, err
	}
	var definition Definition
	for _, candidate := range definitions {
		if candidate.SemanticDigest == input.SemanticDigest {
			definition = candidate
			break
		}
	}
	if definition.SemanticDigest == "" || definition.Superseded {
		return nil, errors.New("adapter revision base is unavailable")
	}
	selected := make([]map[string]any, 0, len(input.OperationIDs))
	seen := map[string]bool{}
	for _, id := range input.OperationIDs {
		if seen[id] {
			return nil, errors.New("adapter operation selection is invalid")
		}
		seen[id] = true
		found := false
		for _, operation := range definition.Manifest.Operations {
			if operation.OperationID == id {
				raw, err := json.Marshal(operation)
				if err != nil {
					return nil, err
				}
				var proposal map[string]any
				if err := json.Unmarshal(raw, &proposal); err != nil {
					return nil, err
				}
				delete(proposal, "behavior")
				delete(proposal, "retry")
				proposal["read_only"] = *operation.Behavior.ReadOnly.Value
				proposal["idempotent"] = *operation.Behavior.Idempotent.Value
				proposal["destructive"] = *operation.Behavior.Destructive.Value
				proposal["open_world"] = *operation.Behavior.OpenWorld.Value
				proposal["response"].(map[string]any)["kind"] = "custom"
				selected = append(selected, proposal)
				found = true
				break
			}
		}
		if !found {
			return nil, errors.New("adapter operation is unavailable")
		}
	}
	return map[string]any{"revision_base": map[string]any{"semantic_digest": definition.SemanticDigest, "source_reference": definition.SourceReference, "definition_id": definition.Manifest.DefinitionID, "adapter_id": definition.Manifest.AdapterID, "display_name": definition.Manifest.DisplayName, "definition_revision": definition.Manifest.DefinitionRevision, "origin": definition.Manifest.Origin, "authentication": definition.Manifest.Authentication, "operations": selected}}, nil
}

func (s *Service) propose(raw json.RawMessage) (any, error) {
	if len(raw) > manifestLimit {
		return nil, errors.New("adapter proposal is too large")
	}
	var input proposalInput
	if decodeExactJSON(raw, &input) != nil {
		return nil, errors.New("adapter proposal is invalid")
	}
	var base *Manifest
	if input.BaseSemanticDigest != "" {
		definitions, err := s.files.definitions()
		if err != nil {
			return nil, err
		}
		var definition Definition
		for _, candidate := range definitions {
			if candidate.SemanticDigest == input.BaseSemanticDigest {
				definition = candidate
				break
			}
		}
		if definition.SemanticDigest == "" || definition.Superseded {
			return nil, errors.New("adapter revision base changed")
		}
		copy := definition.Manifest
		base = &copy
	}
	manifest, err := buildManifest(input, base)
	if err != nil {
		return nil, err
	}
	definition, err := Compile(manifest)
	if err != nil {
		return nil, err
	}
	replaces := []string(nil)
	affected := []string(nil)
	if input.BaseSemanticDigest != "" {
		replaces = []string{input.BaseSemanticDigest}
		affected, err = s.affectedConnectionIDs(replaces)
		if err != nil {
			return nil, err
		}
	}
	definition, err = s.files.installDefinition(manifest, input.SourceReference, replaces, affected)
	if err != nil {
		return nil, err
	}
	if err = s.reconcile(context.Background()); err != nil {
		return nil, err
	}
	return map[string]any{"status": "review_required", "semantic_digest": definition.SemanticDigest, "definition_id": manifest.DefinitionID, "manifest": manifest}, nil
}

// Approve publishes one exact pending revision and creates its credential-free connection.
func (s *Service) Approve(ctx context.Context, digest string) (Definition, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	all, err := s.files.definitions()
	if err != nil {
		return Definition{}, err
	}
	var pending Definition
	for _, candidate := range all {
		if candidate.SemanticDigest == digest {
			pending = candidate
			break
		}
	}
	if pending.SemanticDigest == "" {
		return Definition{}, errors.New("adapter definition review is unavailable")
	}
	if pending.Manifest.Reviewed && !pending.Superseded {
		if err = s.adoptConnections(pending); err != nil {
			return Definition{}, err
		}
		if err = s.ensureConnection(pending); err != nil {
			return Definition{}, err
		}
		return pending, s.reconcile(ctx)
	}
	reviewedManifest := pending.Manifest
	reviewedManifest.Reviewed = true
	expected, compileErr := Compile(reviewedManifest)
	if compileErr != nil {
		return Definition{}, compileErr
	}
	if pending.Superseded {
		for _, candidate := range all {
			if candidate.SemanticDigest == expected.SemanticDigest && candidate.Manifest.Reviewed && !candidate.Superseded {
				if err = s.adoptConnections(candidate); err != nil {
					return Definition{}, err
				}
				if err = s.ensureConnection(candidate); err != nil {
					return Definition{}, err
				}
				return candidate, s.reconcile(ctx)
			}
		}
		return Definition{}, errors.New("adapter definition review is stale")
	}
	currentAffected, err := s.affectedConnectionIDs(pending.Replaces)
	if err != nil || strings.Join(currentAffected, "\x00") != strings.Join(pending.AffectedConnections, "\x00") {
		return Definition{}, errors.New("adapter definition transition changed")
	}
	reviewed, err := s.files.installDefinition(reviewedManifest, pending.SourceReference, pending.Replaces, pending.AffectedConnections)
	if err != nil {
		return Definition{}, err
	}
	if err = s.adoptConnections(reviewed); err != nil {
		return Definition{}, err
	}
	if err = s.ensureConnection(reviewed); err != nil {
		return Definition{}, err
	}
	if err = s.reconcile(ctx); err != nil {
		return Definition{}, err
	}
	return reviewed, nil
}

func (s *Service) affectedConnectionIDs(replaces []string) ([]string, error) {
	wanted, err := s.replacementLineage(replaces)
	if err != nil {
		return nil, err
	}
	connections, err := s.files.connections()
	if err != nil {
		return nil, err
	}
	result := make([]string, 0)
	for _, connection := range connections {
		if wanted[connection.SemanticDigest] {
			result = append(result, connection.ConnectionID)
		}
	}
	sort.Strings(result)
	return result, nil
}

func (s *Service) adoptConnections(replacement Definition) error {
	wanted, err := s.replacementLineage(replacement.Replaces)
	if err != nil {
		return err
	}
	connections, err := s.files.connections()
	if err != nil {
		return err
	}
	newOperations := make(map[string]CompiledOperation, len(replacement.Operations))
	for _, operation := range replacement.Operations {
		newOperations[operation.OperationID] = operation
	}
	for _, connection := range connections {
		if !wanted[connection.SemanticDigest] {
			continue
		}
		current, loadErr := s.files.loadDefinition(connection.SemanticDigest)
		if loadErr != nil {
			return loadErr
		}
		oldOperations := make(map[string]CompiledOperation, len(current.Operations))
		for _, operation := range current.Operations {
			oldOperations[operation.OperationID] = operation
		}
		allowed := connection.AllowedOperations[:0]
		for _, id := range connection.AllowedOperations {
			if _, exists := newOperations[id]; exists {
				allowed = append(allowed, id)
			}
		}
		connection.AllowedOperations = allowed
		before := len(connection.Overrides)
		for id, override := range connection.Overrides {
			oldOperation, oldOK := oldOperations[id]
			newOperation, newOK := newOperations[id]
			if !oldOK || !newOK || !sameOperationContract(oldOperation, newOperation) {
				delete(connection.Overrides, id)
				continue
			}
			override.SourceRevision = newOperation.Digest
			connection.Overrides[id] = override
		}
		connection.SemanticDigest = replacement.SemanticDigest
		if !sameAuthentication(current.Manifest.Authentication, replacement.Manifest.Authentication) {
			// A changed auth scheme has no grant yet. Keep the connection linked
			// to the reviewed definition while OAuth setup is pending.
			connection.Authentication = ConnectionAuthentication{Kind: "pending"}
			connection.Overrides = map[string]OperationOverride{}
			if replacement.Manifest.Authentication.Kind == "none" {
				connection.Status = "active"
				connection.AllowedOperations = connection.AllowedOperations[:0]
				for id := range newOperations {
					connection.AllowedOperations = append(connection.AllowedOperations, id)
				}
				sort.Strings(connection.AllowedOperations)
			} else {
				connection.Status = "authentication_required"
				// SQLite and the connection contract require an empty array while
				// authentication is pending, not a JSON null value.
				connection.AllowedOperations = []string{}
				connection.PolicyRevision++
			}
		}
		connection.ConnectionRevision++
		if before != len(connection.Overrides) || len(connection.Overrides) != 0 {
			connection.PolicyRevision++
		}
		if _, err = s.files.replaceConnection(connection); err != nil {
			return err
		}
	}
	return nil
}

func sameAuthentication(left, right Authentication) bool {
	a, aerr := normalizedJSON(left)
	b, berr := normalizedJSON(right)
	return aerr == nil && berr == nil && string(a) == string(b)
}

func (s *Service) replacementLineage(replaces []string) (map[string]bool, error) {
	definitions, err := s.files.definitions()
	if err != nil {
		return nil, err
	}
	byDigest := make(map[string]Definition, len(definitions))
	for _, definition := range definitions {
		byDigest[definition.SemanticDigest] = definition
	}
	wanted := make(map[string]bool, len(replaces))
	pending := append([]string(nil), replaces...)
	for len(pending) != 0 {
		digest := pending[len(pending)-1]
		pending = pending[:len(pending)-1]
		if wanted[digest] {
			continue
		}
		definition, exists := byDigest[digest]
		if !exists {
			return nil, errors.New("adapter revision lineage changed")
		}
		wanted[digest] = true
		pending = append(pending, definition.Replaces...)
	}
	return wanted, nil
}

func sameOperationContract(left, right CompiledOperation) bool {
	left.Description, left.SourceDescription = "", ""
	right.Description, right.SourceDescription = "", ""
	for index := range left.Arguments {
		left.Arguments[index].Description = ""
	}
	for index := range right.Arguments {
		right.Arguments[index].Description = ""
	}
	left.Digest, right.Digest = "", ""
	left.InputSchema, right.InputSchema = nil, nil
	leftRaw, leftErr := normalizedJSON(left.Operation)
	rightRaw, rightErr := normalizedJSON(right.Operation)
	return leftErr == nil && rightErr == nil && string(leftRaw) == string(rightRaw)
}

func (s *Service) ensureConnection(definition Definition) error {
	if definition.Manifest.Authentication.Kind != "none" {
		return nil
	}
	connections, err := s.files.connections()
	if err != nil {
		return err
	}
	for _, connection := range connections {
		if connection.SemanticDigest == definition.SemanticDigest {
			return nil
		}
	}
	allowed := make([]string, len(definition.Operations))
	for i, operation := range definition.Operations {
		allowed[i] = operation.OperationID
	}
	sort.Strings(allowed)
	id := randomHex()
	_, err = s.files.installConnection(Connection{SchemaVersion: 2, ConnectionID: id, ConnectionSlug: "personal-" + id[:8], SemanticDigest: definition.SemanticDigest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: allowed, Overrides: map[string]OperationOverride{}, Authentication: ConnectionAuthentication{Kind: "none"}})
	return err
}

// SetupCredentialConnection creates or replaces one protected direct credential.
func (s *Service) SetupCredentialConnection(ctx context.Context, digest, replacement string, input CredentialInputValue) (Definition, Connection, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	definition, err := s.files.loadDefinition(digest)
	if err != nil || !definition.Manifest.Reviewed || definition.Superseded || definition.Manifest.Authentication.Kind != "credential" {
		return Definition{}, Connection{}, errors.New("adapter credential setup is unavailable")
	}
	fields, err := normalizeCredential(*definition.Manifest.Authentication.Setup, input)
	if err != nil {
		return Definition{}, Connection{}, err
	}
	generationID := randomHex()
	generation := credentialGeneration{SchemaVersion: 2, GenerationID: generationID, Fields: fields}
	allowed := make([]string, len(definition.Operations))
	for i, operation := range definition.Operations {
		allowed[i] = operation.OperationID
	}
	sort.Strings(allowed)
	var connection Connection
	if replacement == "" {
		id := randomHex()
		connection = Connection{SchemaVersion: 2, ConnectionID: id, ConnectionSlug: "personal-" + id[:8], SemanticDigest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: allowed, Overrides: map[string]OperationOverride{}, Authentication: ConnectionAuthentication{Kind: "credential", GenerationID: generationID, Revision: 1}}
		connection, err = s.files.installCredentialConnection(connection, generation)
	} else {
		connection, err = s.files.loadConnection(replacement)
		if err != nil || connection.SemanticDigest != digest || connection.Authentication.Kind != "credential" || connection.Status != "authentication_required" {
			return Definition{}, Connection{}, errors.New("adapter credential replacement is unavailable")
		}
		connection.Authentication.GenerationID = generationID
		connection.Authentication.Revision++
		connection.Status = "active"
		connection.AllowedOperations = allowed
		connection.ConnectionRevision++
		connection, err = s.files.replaceCredential(connection, generation)
	}
	if err == nil {
		err = s.reconcile(ctx)
	}
	return definition, connection, err
}

// Cancel removes one exact pending definition.
func (s *Service) Cancel(ctx context.Context, digest string) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	definition, err := s.files.loadDefinition(digest)
	if err != nil {
		return false, nil
	}
	if definition.Manifest.Reviewed {
		return false, errors.New("reviewed adapter definition cannot be cancelled")
	}
	if err = s.files.quarantine("definitions", digest); err != nil {
		return false, err
	}
	return true, s.reconcile(ctx)
}

// DeleteConnection removes one exact connection revision.
func (s *Service) DeleteConnection(ctx context.Context, id string, revision int) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, err := s.files.loadConnection(id)
	if err != nil {
		return false, nil
	}
	if value.ConnectionRevision != revision {
		return false, errors.New("adapter connection revision changed")
	}
	if err = s.files.quarantine("connections", id); err != nil {
		return false, err
	}
	return true, s.reconcile(ctx)
}

// DeleteService removes an unconnected exact definition family.
func (s *Service) DeleteService(ctx context.Context, id, digest string) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	definitions, err := s.files.definitions()
	if err != nil {
		return false, err
	}
	connections, err := s.files.connections()
	if err != nil {
		return false, err
	}
	family := make([]Definition, 0)
	var current Definition
	for _, value := range definitions {
		if value.Manifest.DefinitionID == id {
			family = append(family, value)
			if !value.Superseded && (current.SemanticDigest == "" || value.Manifest.Reviewed) {
				current = value
			}
		}
	}
	if current.SemanticDigest == "" {
		return false, nil
	}
	if current.SemanticDigest != digest {
		return false, errors.New("adapter definition revision changed")
	}
	for _, connection := range connections {
		for _, definition := range family {
			if connection.SemanticDigest == definition.SemanticDigest {
				return false, errors.New("adapter definition has connections")
			}
		}
	}
	for _, definition := range family {
		if err = s.files.quarantine("definitions", definition.SemanticDigest); err != nil {
			return false, err
		}
	}
	return true, s.reconcile(ctx)
}

// Snapshot returns current safe filesystem authorities.
func (s *Service) Snapshot() (ServiceSnapshot, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	definitions, err := s.files.definitions()
	if err != nil {
		return ServiceSnapshot{}, err
	}
	connections, err := s.files.connections()
	return ServiceSnapshot{definitions, connections}, err
}

// SetActive changes one exact connection lifecycle state.
func (s *Service) SetActive(ctx context.Context, id string, revision int, active bool) (Connection, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, err := s.files.loadConnection(id)
	if err != nil || value.ConnectionRevision != revision {
		return Connection{}, errors.New("adapter connection revision changed")
	}
	if value.Status == "authentication_required" {
		return Connection{}, errors.New("adapter authentication is required")
	}
	if active {
		if value.Authentication.Kind == "credential" && value.Authentication.GenerationID == "" {
			return Connection{}, errors.New("adapter authentication is required")
		}
		value.Status = "active"
	} else {
		value.Status = "suspended"
	}
	value.ConnectionRevision++
	value, err = s.files.replaceConnection(value)
	if err == nil {
		err = s.reconcile(ctx)
	}
	return value, err
}

// SaveConnectionPolicy stores one exact human connection policy.
func (s *Service) SaveConnectionPolicy(ctx context.Context, id, revision string, expected int, sharing, unsafe string) (Connection, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, err := s.files.loadConnection(id)
	if err != nil || strconv.Itoa(value.ConnectionRevision) != revision || value.PolicyRevision != expected {
		return Connection{}, errors.New("adapter connection policy changed")
	}
	if sharing != "allow_automatically" && sharing != "review_every_call" || unsafe != "always_ask" && unsafe != "reviewer_may_approve" && unsafe != "never_ask" || sharing == "review_every_call" && unsafe == "never_ask" {
		return Connection{}, errors.New("adapter connection policy is invalid")
	}
	value.DataSharingPolicy, value.UnsafeActionPolicy = sharing, unsafe
	value.PolicyRevision++
	value.ConnectionRevision++
	value, err = s.files.replaceConnection(value)
	if err == nil {
		err = s.reconcile(ctx)
	}
	return value, err
}

// SaveConnectionLabel stores one exact optional label.
func (s *Service) SaveConnectionLabel(ctx context.Context, id, revision string, expected, replacement *string) (Connection, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, err := s.files.loadConnection(id)
	if err != nil || strconv.Itoa(value.ConnectionRevision) != revision {
		return Connection{}, errors.New("adapter connection revision changed")
	}
	current := value.ConnectionLabel
	if expected != nil && *expected != current {
		return Connection{}, errors.New("adapter connection label changed")
	}
	label := ""
	if replacement != nil {
		label = strings.TrimSpace(*replacement)
		if len(label) > 256 {
			return Connection{}, errors.New("adapter connection label is invalid")
		}
	}
	value.ConnectionLabel = label
	value.ConnectionRevision++
	value, err = s.files.replaceConnection(value)
	if err == nil {
		err = s.reconcile(ctx)
	}
	return value, err
}

// ChangeTool updates one exact operation policy.
func (s *Service) ChangeTool(ctx context.Context, id, revision, tool, source string, expected int, enabled *bool, behavior *store.ActionBehavior, reset bool) (OperationOverride, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	connection, err := s.files.loadConnection(id)
	if err != nil || strconv.Itoa(connection.ConnectionRevision) != revision {
		return OperationOverride{}, errors.New("adapter connection revision changed")
	}
	definition, err := s.files.loadDefinition(connection.SemanticDigest)
	if err != nil {
		return OperationOverride{}, err
	}
	var operation *CompiledOperation
	for i := range definition.Operations {
		if definition.Operations[i].OperationID == tool && definition.Operations[i].Digest == source {
			operation = &definition.Operations[i]
			break
		}
	}
	if operation == nil {
		return OperationOverride{}, errors.New("adapter operation revision changed")
	}
	override, exists := connection.Overrides[tool]
	if !exists {
		override = OperationOverride{Enabled: true, PolicyRevision: 1, SourceRevision: source}
	}
	if override.PolicyRevision != expected {
		return OperationOverride{}, errors.New("adapter operation policy changed")
	}
	if connection.Overrides == nil {
		connection.Overrides = make(map[string]OperationOverride)
	}
	if reset {
		override.Behavior = nil
		override.Enabled = true
	} else {
		if enabled != nil {
			override.Enabled = *enabled
		}
		if behavior != nil {
			copy := *behavior
			override.Behavior = &copy
		}
	}
	override.PolicyRevision++
	connection.Overrides[tool] = override
	connection.PolicyRevision++
	connection.ConnectionRevision++
	_, err = s.files.replaceConnection(connection)
	if err == nil {
		err = s.reconcile(ctx)
	}
	return override, err
}

// Bindings returns current callable HTTP operations.
func (s *Service) Bindings() ([]Binding, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.bindings()
}

// AvailabilityNotices returns deterministic reasons for reviewed operations
// that remain selected but cannot enter the callable catalog.
func (s *Service) AvailabilityNotices() ([]AvailabilityNotice, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	snapshot, err := s.snapshot()
	if err != nil {
		return nil, err
	}
	definitions := make(map[string]Definition, len(snapshot.Definitions))
	for _, definition := range snapshot.Definitions {
		if definition.Manifest.Reviewed && !definition.Superseded {
			definitions[definition.SemanticDigest] = definition
		}
	}
	result := make([]AvailabilityNotice, 0)
	for _, connection := range snapshot.Connections {
		definition, ok := definitions[connection.SemanticDigest]
		if !ok || connection.Status != "active" || definition.Manifest.Authentication.Kind != "oauth2_authorization_code_pkce" {
			continue
		}
		grant, _, loadErr := s.files.loadOAuthGrant(connection.Authentication.GrantID)
		if loadErr != nil || grant.Status != "active" {
			continue
		}
		allowed := make(map[string]bool, len(connection.AllowedOperations))
		for _, operationID := range connection.AllowedOperations {
			allowed[operationID] = true
		}
		for _, operation := range definition.Operations {
			if allowed[operation.OperationID] && !operationScopesSatisfied(operation, grant.GrantedScopes) {
				result = append(result, AvailabilityNotice{Name: definition.Manifest.AdapterID + "_" + connection.ConnectionSlug + "." + operation.OperationID, Status: "authorization_scope_unavailable", DefinitionDigest: definition.SemanticDigest, OperationID: operation.OperationID})
			}
		}
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Name < result[j].Name })
	return result, nil
}
func (s *Service) bindings() ([]Binding, error) {
	snapshot, err := s.snapshot()
	if err != nil {
		return nil, err
	}
	definitions := map[string]Definition{}
	for _, definition := range snapshot.Definitions {
		if definition.Manifest.Reviewed && !definition.Superseded {
			definitions[definition.SemanticDigest] = definition
		}
	}
	result := make([]Binding, 0)
	for _, connection := range snapshot.Connections {
		definition, ok := definitions[connection.SemanticDigest]
		if !ok || connection.Status != "active" || connection.DataSharingPolicy == "" || connection.UnsafeActionPolicy == "" || definition.Manifest.Authentication.Kind == "credential" && connection.Authentication.GenerationID == "" {
			continue
		}
		grantID, accountID, authorityRevision := "", "", connection.Authentication.Revision
		connectionName := connection.ConnectionLabel
		if connectionName == "" {
			connectionName = connection.ConnectionSlug
		}
		connectionDescription := "\nAPI: " + strconv.Quote(DisplayName(definition)) + "\nConnection: " + strconv.Quote(connectionName)
		var granted []string
		if definition.Manifest.Authentication.Kind == "oauth2_authorization_code_pkce" {
			grant, _, loadErr := s.files.loadOAuthGrant(connection.Authentication.GrantID)
			if loadErr != nil || grant.Status != "active" {
				continue
			}
			grantID, authorityRevision, granted = grant.GrantID, grant.AuthorityRevision, grant.GrantedScopes
			if grant.AccountID != nil {
				accountID = *grant.AccountID
			}
			accountName := grant.GrantID
			if grant.AccountLabel != nil {
				accountName = *grant.AccountLabel
			}
			connectionDescription += "\nAccount: " + strconv.Quote(accountName)
		}
		allowed := map[string]bool{}
		for _, id := range connection.AllowedOperations {
			allowed[id] = true
		}
		for _, operation := range definition.Operations {
			if !allowed[operation.OperationID] {
				continue
			}
			if definition.Manifest.Authentication.Kind == "oauth2_authorization_code_pkce" {
				authorized := false
				for _, set := range operation.Authorization.AcceptedScopeSets {
					if scopeSubset(granted, set) {
						authorized = true
						break
					}
				}
				if !authorized {
					continue
				}
			}
			override, exists := connection.Overrides[operation.OperationID]
			if exists && !override.Enabled {
				continue
			}
			behavior := operation.Behavior
			toolRevision := 1
			if exists {
				toolRevision = override.PolicyRevision
				if override.SourceRevision != operation.Digest {
					return nil, errors.New("adapter operation policy is stale")
				}
				if override.Behavior != nil {
					behavior = *override.Behavior
				}
			}
			route := reviewRoute(connection, behavior)
			name := definition.Manifest.AdapterID + "_" + connection.ConnectionSlug + "." + operation.OperationID
			result = append(result, Binding{Name: name, Description: operation.Description + connectionDescription, ConnectionID: connection.ConnectionID, DefinitionID: definition.Manifest.DefinitionID, SemanticDigest: definition.SemanticDigest, OperationID: operation.OperationID, OperationDigest: operation.Digest, ConnectionRevision: connection.ConnectionRevision, PolicyRevision: connection.PolicyRevision, ToolPolicyRevision: toolRevision, CredentialRevision: authorityRevision, GrantID: grantID, AccountID: accountID, InputSchema: operation.InputSchema, Behavior: behavior, ReviewRoute: route})
		}
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Name < result[j].Name })
	return result, nil
}

// Binding returns one current exact model-visible operation.
func (s *Service) Binding(name string) (Binding, error) {
	values, err := s.Bindings()
	if err != nil {
		return Binding{}, err
	}
	for _, value := range values {
		if value.Name == name {
			return value, nil
		}
	}
	return Binding{}, errors.New("adapter operation is unavailable")
}

// Validate checks arguments against one current exact operation without making a request.
func (s *Service) Validate(authority Binding, raw json.RawMessage) error {
	current, err := s.Binding(authority.Name)
	if err != nil || !sameBinding(current, authority) {
		return errors.New("adapter call authority changed")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	definition, err := s.files.loadDefinition(current.SemanticDigest)
	if err != nil {
		return err
	}
	for _, operation := range definition.Operations {
		if operation.OperationID == current.OperationID && operation.Digest == current.OperationDigest {
			_, _, err = encodeRequest(definition, operation, raw, "")
			return err
		}
	}
	return errors.New("adapter operation changed")
}

// Call invokes one operation after every filesystem revision check.
func (s *Service) Call(ctx context.Context, authority Binding, raw json.RawMessage) (json.RawMessage, bool, error) {
	current, err := s.Binding(authority.Name)
	if err != nil || !sameBinding(current, authority) {
		return nil, false, errors.New("adapter call authority changed")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	connection, err := s.files.loadConnection(current.ConnectionID)
	if err != nil {
		return nil, false, err
	}
	definition, err := s.files.loadDefinition(current.SemanticDigest)
	if err != nil {
		return nil, false, err
	}
	var operation CompiledOperation
	for _, value := range definition.Operations {
		if value.OperationID == current.OperationID && value.Digest == current.OperationDigest {
			operation = value
			break
		}
	}
	if operation.OperationID == "" {
		return nil, false, errors.New("adapter operation changed")
	}
	reference := continuationReference(raw)
	cursorToken := ""
	if reference != "" {
		cursor, loadErr := s.loadCursor(reference)
		if loadErr != nil {
			return nil, false, errors.New("adapter continuation is invalid")
		}
		cursorToken = cursor.Token
	}
	request, arguments, err := encodeRequest(definition, operation, raw, cursorToken)
	if err != nil {
		return nil, false, err
	}
	sensitive := map[string]bool{}
	secretValues := []string{}
	if definition.Manifest.Authentication.Kind == "credential" {
		fields, loadErr := s.files.loadCredential(connection)
		if loadErr != nil || !validCredentialFields(*definition.Manifest.Authentication.Setup, fields) {
			return nil, false, ErrAuthenticationRequired
		}
		sensitive, secretValues, err = applyCredentialAuth(definition.Manifest.Authentication, fields, operation, &request)
		if err != nil {
			return nil, false, err
		}
	}
	if definition.Manifest.Authentication.Kind == "oauth2_authorization_code_pkce" {
		grant, token, loadErr := s.oauthBearer(ctx, connection.Authentication.GrantID, false)
		if loadErr != nil {
			if errors.Is(loadErr, errOAuthRejected) {
				_ = s.invalidateOAuthAuthentication(ctx, &grant)
				return nil, false, ErrAuthenticationRequired
			}
			if errors.Is(loadErr, ErrAuthenticationRequired) {
				return nil, false, ErrAuthenticationRequired
			}
			return nil, false, loadErr
		}
		if grant.AuthorityRevision != current.CredentialRevision || bearerHeader(token.AccessToken, &request) != nil {
			return nil, false, errors.New("adapter OAuth authority changed")
		}
		if !operationScopesSatisfied(operation, token.Scopes) {
			_ = s.requireOAuthAuthentication(ctx, &grant)
			return nil, false, ErrAuthenticationRequired
		}
	}
	digest := argumentsDigest(arguments)
	if reference != "" {
		cursor, _ := s.loadCursor(reference)
		if cursorErr := validateCursorBinding(cursor, current, digest, time.Now()); cursorErr != nil {
			return nil, false, errors.New("adapter continuation is stale")
		}
	}
	response, err := s.executeHTTP(ctx, request, operation.Retry == "transport_safe_read" && current.Behavior.RepeatSafe)
	if errors.Is(err, errOutcomeUncertain) {
		return nil, false, classifyHTTPOutcome(err, current.Behavior)
	}
	if err != nil {
		if errors.Is(err, errResponseInvalid) && !current.Behavior.ReadOnly {
			return nil, false, ErrOutcomeUncertain
		}
		return nil, false, err
	}
	if response.status == 401 && definition.Manifest.Authentication.Kind == "oauth2_authorization_code_pkce" {
		grant, token, refreshErr := s.oauthBearer(ctx, connection.Authentication.GrantID, true)
		if refreshErr == nil && !operationScopesSatisfied(operation, token.Scopes) {
			refreshErr = ErrAuthenticationRequired
		}
		if refreshErr == nil && grant.AuthorityRevision == current.CredentialRevision && current.Behavior.ReadOnly {
			request.headers["Authorization"] = "Bearer " + token.AccessToken
			response, refreshErr = s.executeHTTP(ctx, request, false)
		}
		if refreshErr != nil && !errors.Is(refreshErr, errOAuthRejected) && !errors.Is(refreshErr, ErrAuthenticationRequired) {
			return nil, false, refreshErr
		}
		if refreshErr != nil || response.status == 401 || !current.Behavior.ReadOnly {
			if errors.Is(refreshErr, errOAuthRejected) {
				_ = s.invalidateOAuthAuthentication(ctx, &grant)
			} else {
				_ = s.requireOAuthAuthentication(ctx, &grant)
			}
			return nil, false, ErrAuthenticationRequired
		}
	}
	if response.status < 200 || response.status >= 300 {
		if response.status == 401 && definition.Manifest.Authentication.Kind == "credential" {
			connection.Status = "authentication_required"
			connection.ConnectionRevision++
			connection.AllowedOperations = nil
			if _, updateErr := s.files.replaceConnection(connection); updateErr != nil {
				return nil, false, updateErr
			}
			if updateErr := s.reconcile(ctx); updateErr != nil {
				return nil, false, updateErr
			}
			return nil, false, ErrAuthenticationRequired
		}
		if (response.status >= 500 || response.status >= 300 && response.status < 400) && !current.Behavior.ReadOnly {
			return nil, false, ErrOutcomeUncertain
		}
		return responseFailure(response, sensitive, secretValues), false, nil
	}
	nextToken := ""
	if operation.Pagination.Kind == "response_token" {
		value, parseErr := script.DecodeJSON(response.body)
		if parseErr != nil {
			if !current.Behavior.ReadOnly {
				return nil, false, ErrOutcomeUncertain
			}
			return nil, false, errors.New("adapter paginated response is invalid")
		}
		var ok bool
		nextToken, ok = removePointer(value, operation.Pagination.ResponsePointer)
		if !ok {
			if !current.Behavior.ReadOnly {
				return nil, false, ErrOutcomeUncertain
			}
			return nil, false, errors.New("adapter pagination token is invalid")
		}
		response.body, _ = script.MarshalJSON(value)
	}
	result, err := decodeResponse(response, operation.Response)
	if err != nil {
		if !current.Behavior.ReadOnly {
			return nil, false, ErrOutcomeUncertain
		}
		return failure("response_transform_failed", err.Error()), false, nil
	}
	if operation.Pagination.Kind == "response_token" {
		object, ok := result.(map[string]any)
		if !ok {
			return nil, false, errors.New("adapter pagination result is invalid")
		}
		if nextToken != "" {
			next := Cursor{Reference: randomHex(), ConnectionID: current.ConnectionID, GrantID: current.GrantID, AccountID: current.AccountID, ConnectionRevision: current.ConnectionRevision, SemanticDigest: current.SemanticDigest, OperationID: current.OperationID, OperationDigest: current.OperationDigest, ArgumentsDigest: digest, Token: nextToken, ExpiresAt: time.Now().Add(time.Hour)}
			if err = s.putCursor(next); err != nil {
				return nil, false, err
			}
			object["continuation"] = next.Reference
		}
		if reference != "" {
			if err = s.retireCursor(reference); err != nil {
				return nil, false, err
			}
		}
	}
	payload, err := wrapResult(sanitizeSensitiveOutput(result, sensitive, secretValues))
	return payload, err == nil, err
}

func validateCursorBinding(cursor Cursor, binding Binding, argumentsDigest string, now time.Time) error {
	if cursor.ConnectionID != binding.ConnectionID || cursor.ConnectionRevision != binding.ConnectionRevision || cursor.GrantID != binding.GrantID || cursor.AccountID != binding.AccountID || cursor.SemanticDigest != binding.SemanticDigest || cursor.OperationID != binding.OperationID || cursor.OperationDigest != binding.OperationDigest || cursor.ArgumentsDigest != argumentsDigest {
		return errCursorBindingMismatch
	}
	if !cursor.ExpiresAt.After(now) {
		return errCursorExpired
	}
	return nil
}

func classifyHTTPOutcome(err error, behavior store.ActionBehavior) error {
	if !errors.Is(err, errOutcomeUncertain) {
		return err
	}
	if behavior.ReadOnly {
		return errors.New("adapter target is unavailable")
	}
	return ErrOutcomeUncertain
}

func (s *Service) snapshot() (ServiceSnapshot, error) {
	definitions, err := s.files.definitions()
	if err != nil {
		return ServiceSnapshot{}, err
	}
	connections, err := s.files.connections()
	return ServiceSnapshot{definitions, connections}, err
}
func (s *Service) reconcile(ctx context.Context) error {
	snapshot, err := s.snapshot()
	if err != nil {
		return err
	}
	definitions := make([]store.AdapterDefinitionIndex, len(snapshot.Definitions))
	for i, value := range snapshot.Definitions {
		definitions[i] = store.AdapterDefinitionIndex{Digest: value.SemanticDigest, DefinitionID: value.Manifest.DefinitionID, AdapterID: value.Manifest.AdapterID, DefinitionRevision: value.Manifest.DefinitionRevision, SourceReference: value.SourceReference, DisplayName: value.Manifest.DisplayName, Reviewed: value.Manifest.Reviewed, Superseded: value.Superseded, OperationCount: len(value.Operations)}
	}
	connections := make([]store.AdapterConnectionIndex, len(snapshot.Connections))
	for i, value := range snapshot.Connections {
		connections[i] = store.AdapterConnectionIndex{ID: value.ConnectionID, Slug: value.ConnectionSlug, Label: value.ConnectionLabel, Digest: value.SemanticDigest, Status: value.Status, ConnectionRevision: value.ConnectionRevision, PolicyRevision: value.PolicyRevision, DataSharingPolicy: value.DataSharingPolicy, UnsafeActionPolicy: value.UnsafeActionPolicy, AllowedOperations: value.AllowedOperations}
	}
	return s.database.ReconcileAdapters(ctx, definitions, connections, time.Now())
}
func reviewRoute(connection Connection, behavior store.ActionBehavior) store.ActionReviewRoute {
	risky := (!behavior.ReadOnly && (behavior.Destructive || behavior.OpenWorld)) || connection.DataSharingPolicy == "review_every_call"
	if !risky || connection.UnsafeActionPolicy == "never_ask" {
		return ""
	}
	if connection.UnsafeActionPolicy == "always_ask" {
		return store.ActionHumanReview
	}
	return store.ActionLLMReview
}
func DisplayName(value Definition) string {
	if value.Manifest.DisplayName != "" {
		return value.Manifest.DisplayName
	}
	label := strings.Join(strings.FieldsFunc(value.Manifest.AdapterID, func(r rune) bool { return r == '_' || r == '-' }), " ")
	if label != "" {
		label = strings.ToUpper(label[:1]) + label[1:]
	}
	return label
}
func failure(code, message string) json.RawMessage {
	raw, _ := json.Marshal(map[string]any{"error": code, "message": message})
	return raw
}
func continuationReference(raw json.RawMessage) string {
	value, err := script.DecodeJSON(raw)
	if err != nil {
		return ""
	}
	object, _ := value.(map[string]any)
	text, _ := object["continuation"].(string)
	return text
}
func sameBinding(left, right Binding) bool {
	return left.Name == right.Name && left.ConnectionID == right.ConnectionID && left.DefinitionID == right.DefinitionID && left.SemanticDigest == right.SemanticDigest && left.OperationID == right.OperationID && left.OperationDigest == right.OperationDigest && left.ConnectionRevision == right.ConnectionRevision && left.PolicyRevision == right.PolicyRevision && left.ToolPolicyRevision == right.ToolPolicyRevision && left.CredentialRevision == right.CredentialRevision && left.GrantID == right.GrantID && left.Behavior == right.Behavior && left.ReviewRoute == right.ReviewRoute && string(left.InputSchema) == string(right.InputSchema)
}

func (s *Service) putCursor(value Cursor) error {
	if !validConnectionID(value.Reference) || len(value.Token) == 0 || len(value.Token) > 4096 {
		return errors.New("adapter cursor is invalid")
	}
	raw, err := json.Marshal(value)
	if err != nil || len(raw) > 16<<10 {
		return errors.New("adapter cursor is invalid")
	}
	if err := writeNewFile(s.files.root, "adapters/cursors/"+value.Reference+".json", raw); err != nil {
		return err
	}
	return home.SyncRootDirectory(s.files.root, "adapters/cursors")
}
func (s *Service) loadCursor(reference string) (Cursor, error) {
	if !validConnectionID(reference) {
		return Cursor{}, errors.New("adapter cursor is invalid")
	}
	raw, err := readRegular(s.files.root, "adapters/cursors/"+reference+".json", 16<<10)
	if err != nil {
		return Cursor{}, err
	}
	var value Cursor
	if decodeExactJSON(raw, &value) != nil || value.Reference != reference || len(value.Token) == 0 || len(value.Token) > 4096 {
		return Cursor{}, errors.New("adapter cursor is invalid")
	}
	return value, nil
}
func (s *Service) retireCursor(reference string) error {
	if !validConnectionID(reference) {
		return errors.New("adapter cursor is invalid")
	}
	err := s.files.root.Remove("adapters/cursors/" + reference + ".json")
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	return home.SyncRootDirectory(s.files.root, "adapters/cursors")
}
