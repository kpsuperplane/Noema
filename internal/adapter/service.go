package adapter

import (
	"context"
	"encoding/json"
	"errors"
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
)

// Service owns reviewed adapter files, public indexes, and calls.
type Service struct {
	files    *fileAuthority
	database *store.Store
	mu       sync.Mutex
}

// NewService recovers and indexes one fresh Go adapter authority.
func NewService(root *os.Root, database *store.Store) (*Service, error) {
	if database == nil {
		return nil, errors.New("adapter store is unavailable")
	}
	files, err := newFileAuthority(root)
	if err != nil {
		return nil, err
	}
	service := &Service{files: files, database: database}
	if err := service.reconcile(context.Background()); err != nil {
		return nil, err
	}
	return service, nil
}

// SetupTools returns the two fixed adapter definition tools.
func (s *Service) SetupTools() []provider.GenerationTool {
	return []provider.GenerationTool{
		{Name: DefinitionTemplateTool, Description: "List current API definitions or return one concise revision base.", InputSchema: json.RawMessage(`{"type":"object","properties":{"semantic_digest":{"type":"string","maxLength":64},"operation_ids":{"type":"array","maxItems":32,"items":{"type":"string"}}},"additionalProperties":false}`)},
		{Name: ProposeDefinitionTool, Description: "Propose one public HTTP API definition from official HTTPS documentation for human review.", InputSchema: json.RawMessage(`{"type":"object","properties":{"source_reference":{"type":"string","maxLength":4096},"new_definition":{"type":"object"},"base_semantic_digest":{"type":"string"},"revision":{"type":"object"},"upsert_operations":{"type":"array","maxItems":128,"items":{"type":"object"}},"remove_operation_ids":{"type":"array","maxItems":128,"items":{"type":"string"}}},"required":["source_reference"],"additionalProperties":false}`)},
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
		return map[string]any{"definitions": values, "instructions": []string{"Use new_definition for a new public API.", "Use the exact semantic_digest for a revision.", "Use authentication kind none or credential.", "Use language lua for all transforms."}}, nil
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
	selected := make([]Operation, 0, len(input.OperationIDs))
	seen := map[string]bool{}
	for _, id := range input.OperationIDs {
		if seen[id] {
			return nil, errors.New("adapter operation selection is invalid")
		}
		seen[id] = true
		found := false
		for _, operation := range definition.Manifest.Operations {
			if operation.OperationID == id {
				selected = append(selected, operation)
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
			connection.Authentication = ConnectionAuthentication{Kind: replacement.Manifest.Authentication.Kind}
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
				connection.AllowedOperations = nil
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
		allowed := map[string]bool{}
		for _, id := range connection.AllowedOperations {
			allowed[id] = true
		}
		for _, operation := range definition.Operations {
			if !allowed[operation.OperationID] {
				continue
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
			result = append(result, Binding{Name: name, Description: operation.Description, ConnectionID: connection.ConnectionID, DefinitionID: definition.Manifest.DefinitionID, SemanticDigest: definition.SemanticDigest, OperationID: operation.OperationID, OperationDigest: operation.Digest, ConnectionRevision: connection.ConnectionRevision, PolicyRevision: connection.PolicyRevision, ToolPolicyRevision: toolRevision, CredentialRevision: connection.Authentication.Revision, InputSchema: operation.InputSchema, Behavior: behavior, ReviewRoute: route})
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
	digest := argumentsDigest(arguments)
	if reference != "" {
		cursor, _ := s.loadCursor(reference)
		if cursor.ConnectionID != current.ConnectionID || cursor.ConnectionRevision != current.ConnectionRevision || cursor.SemanticDigest != current.SemanticDigest || cursor.OperationID != current.OperationID || cursor.OperationDigest != current.OperationDigest || cursor.ArgumentsDigest != digest || time.Now().After(cursor.ExpiresAt) {
			return nil, false, errors.New("adapter continuation is stale")
		}
	}
	response, err := executeHTTP(ctx, request, operation.Retry == "transport_safe_read" && current.Behavior.RepeatSafe)
	if errors.Is(err, errOutcomeUncertain) {
		return nil, false, classifyHTTPOutcome(err, current.Behavior)
	}
	if err != nil {
		if errors.Is(err, errResponseInvalid) && !current.Behavior.ReadOnly {
			return nil, false, ErrOutcomeUncertain
		}
		return nil, false, err
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
			next := Cursor{Reference: randomHex(), ConnectionID: current.ConnectionID, ConnectionRevision: current.ConnectionRevision, SemanticDigest: current.SemanticDigest, OperationID: current.OperationID, OperationDigest: current.OperationDigest, ArgumentsDigest: digest, Token: nextToken, ExpiresAt: time.Now().Add(time.Hour)}
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
	return left.Name == right.Name && left.ConnectionID == right.ConnectionID && left.DefinitionID == right.DefinitionID && left.SemanticDigest == right.SemanticDigest && left.OperationID == right.OperationID && left.OperationDigest == right.OperationDigest && left.ConnectionRevision == right.ConnectionRevision && left.PolicyRevision == right.PolicyRevision && left.ToolPolicyRevision == right.ToolPolicyRevision && left.CredentialRevision == right.CredentialRevision && left.Behavior == right.Behavior && left.ReviewRoute == right.ReviewRoute && string(left.InputSchema) == string(right.InputSchema)
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
