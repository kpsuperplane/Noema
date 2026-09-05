package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"sort"
	"strconv"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) requireAdapters(ctx context.Context) (*adapter.Service, error) {
	if _, ok := auth.BrowserSessionHash(ctx); !ok || r.Adapters == nil {
		return nil, errors.New("adapter service is unavailable")
	}
	return r.Adapters, nil
}

func definitionModel(value adapter.Definition, snapshot adapter.ServiceSnapshot) *model.AdapterDefinition {
	manifest, _ := json.Marshal(value.Manifest)
	transition := &model.AdapterDefinitionTransition{AddedOperations: []string{}, ChangedOperations: []string{}, RemovedOperations: []string{}}
	transition.AffectedConnections = len(value.AffectedConnections)
	previous := map[string]string{}
	for _, digest := range value.Replaces {
		for _, prior := range snapshot.Definitions {
			if prior.SemanticDigest == digest {
				for _, operation := range prior.Operations {
					previous[operation.OperationID] = operation.Digest
				}
			}
		}
	}
	for _, operation := range value.Operations {
		if digest, found := previous[operation.OperationID]; !found {
			transition.AddedOperations = append(transition.AddedOperations, operation.OperationID)
		} else if digest != operation.Digest {
			transition.ChangedOperations = append(transition.ChangedOperations, operation.OperationID)
		}
		delete(previous, operation.OperationID)
	}
	for operation := range previous {
		transition.RemovedOperations = append(transition.RemovedOperations, operation)
	}
	sort.Strings(transition.AddedOperations)
	sort.Strings(transition.ChangedOperations)
	sort.Strings(transition.RemovedOperations)
	result := &model.AdapterDefinition{SemanticDigest: value.SemanticDigest, DefinitionID: value.Manifest.DefinitionID,
		AdapterID: value.Manifest.AdapterID, DisplayName: adapter.DisplayName(value), DefinitionRevision: value.Manifest.DefinitionRevision,
		SourceReference: value.SourceReference, Origin: value.Manifest.Origin, AuthenticationMode: "none", Scopes: []string{},
		Transition:   transition,
		ManifestJSON: string(manifest), Reviewed: value.Manifest.Reviewed, Superseded: value.Superseded,
		Connections: []*model.AdapterConnection{}, ConnectionActions: []*model.AdapterNextAction{}}
	for _, operation := range value.Operations {
		arguments := make([]string, len(operation.Arguments))
		for i := range operation.Arguments {
			arguments[i] = operation.Arguments[i].Name
		}
		var transform *model.AdapterResponseTransform
		if operation.Response.Transform != nil {
			digest := sha256.Sum256([]byte(operation.Response.Transform.Source))
			out, _ := json.Marshal(operation.Response.OutputSchema)
			transform = &model.AdapterResponseTransform{Language: "lua", SourceDigest: hex.EncodeToString(digest[:]), Source: operation.Response.Transform.Source,
				AcceptedContentTypes: operation.Response.AcceptedContentTypes, OutputSchemaJSON: string(out)}
		}
		readOnly, repeatSafe, destructive, openWorld := operation.Behavior.ReadOnly, operation.Behavior.RepeatSafe, operation.Behavior.Destructive, operation.Behavior.OpenWorld
		result.Operations = append(result.Operations, &model.AdapterOperation{OperationID: operation.OperationID, Method: operation.Method,
			Path: operation.Path, ReadOnly: &readOnly, Idempotent: &repeatSafe,
			Destructive: &destructive, OpenWorld: &openWorld, ArgumentNames: arguments,
			ResponseTransform: transform, AcceptedScopeSets: []*model.AdapterScopeSet{}})
	}
	for _, connection := range snapshot.Connections {
		if connection.SemanticDigest == value.SemanticDigest {
			result.Connections = append(result.Connections, adapterDefinitionConnection(value, connection))
			if connection.DataSharingPolicy == "" || connection.UnsafeActionPolicy == "" {
				revision, policy := connection.ConnectionRevision, connection.PolicyRevision
				result.ConnectionActions = append(result.ConnectionActions, &model.AdapterNextAction{Kind: "review_connection_policy", SemanticDigest: value.SemanticDigest, ConnectionID: &connection.ConnectionID, ExpectedConnectionRevision: &revision, ExpectedPolicyRevision: &policy, OperationIds: []string{}, MissingScopes: []string{}})
			}
		}
	}
	result.ConnectionCount = len(result.Connections)
	if !value.Manifest.Reviewed && !value.Superseded {
		result.NextAction = &model.AdapterNextAction{Kind: "review_definition", SemanticDigest: value.SemanticDigest, OperationIds: []string{}, MissingScopes: []string{}}
	}
	return result
}

func adapterDefinitionConnection(def adapter.Definition, value adapter.Connection) *model.AdapterConnection {
	access := make([]*model.AdapterOperationAccess, 0, len(def.Operations))
	allowed := map[string]bool{}
	for _, id := range value.AllowedOperations {
		allowed[id] = true
	}
	for _, operation := range def.Operations {
		status := "available"
		if value.Status == "suspended" || !allowed[operation.OperationID] {
			status = "disabled"
		}
		if override, ok := value.Overrides[operation.OperationID]; ok && !override.Enabled {
			status = "disabled"
		}
		access = append(access, &model.AdapterOperationAccess{OperationID: operation.OperationID, Status: status, MissingScopes: []string{}})
	}
	return &model.AdapterConnection{ConnectionID: value.ConnectionID, Status: value.Status, ConnectionRevision: value.ConnectionRevision,
		PolicyRevision: value.PolicyRevision, GrantedScopes: []string{}, AllowedOperations: value.AllowedOperations,
		PolicyConfigured: value.DataSharingPolicy != "" && value.UnsafeActionPolicy != "", OperationAccess: access}
}

func adapterCapabilityConnection(def adapter.Definition, value adapter.Connection) *model.CapabilityConnection {
	name := def.Manifest.DisplayName
	var label, sharing, unsafe *string
	if value.ConnectionLabel != "" {
		copy := value.ConnectionLabel
		label, name = &copy, copy
	}
	if value.DataSharingPolicy != "" {
		copy := value.DataSharingPolicy
		sharing = &copy
	}
	if value.UnsafeActionPolicy != "" {
		copy := value.UnsafeActionPolicy
		unsafe = &copy
	}
	available, disabled := 0, 0
	for _, op := range def.Operations {
		if override, ok := value.Overrides[op.OperationID]; ok && !override.Enabled {
			disabled++
		} else {
			available++
		}
	}
	status := "setup_required"
	if value.Status == "active" && sharing != nil && unsafe != nil {
		status = "ready"
	}
	return &model.CapabilityConnection{Kind: model.CapabilityIntegrationKindAPI, DefinitionID: def.Manifest.DefinitionID,
		ConnectionID: value.ConnectionID, Name: name, ConnectionLabel: label, SourceRevision: def.SemanticDigest,
		ConnectionRevision: strconv.Itoa(value.ConnectionRevision), PolicyRevision: value.PolicyRevision, Status: status,
		HealthStatus: "unknown", AuthStatus: "not_required", DataSharingPolicy: sharing, UnsafeActionPolicy: unsafe,
		ToolCount: len(def.Operations), AvailableToolCount: available, DisabledToolCount: disabled, SourceDetails: map[string]any{"origin": def.Manifest.Origin}}
}

func adapterToolModel(def adapter.Definition, connection adapter.Connection, op adapter.CompiledOperation) *model.CapabilityManagedTool {
	behavior, enabled, revision := op.Behavior, true, 1
	if override, ok := connection.Overrides[op.OperationID]; ok {
		enabled, revision = override.Enabled, override.PolicyRevision
		if override.Behavior != nil {
			behavior = *override.Behavior
		}
	}
	source := "definition"
	hint := func(v bool) *model.CapabilityManagedToolHint {
		return &model.CapabilityManagedToolHint{Value: &v, Source: &source}
	}
	description := op.Description
	status := "available"
	if !enabled {
		status = "disabled"
	}
	result := &model.CapabilityManagedTool{Kind: model.CapabilityIntegrationKindAPI, ConnectionID: connection.ConnectionID,
		ToolID: op.OperationID, Name: op.OperationID, Description: &description, Enabled: enabled, ReadOnly: hint(behavior.ReadOnly),
		Idempotent: hint(behavior.RepeatSafe), Destructive: hint(behavior.Destructive), OpenWorld: hint(behavior.OpenWorld),
		Status: status, PolicyRevision: revision, SourceRevision: op.Digest, SourceDetails: map[string]any{"method": op.Method, "path": op.Path}}
	if connection.DataSharingPolicy != "" && connection.UnsafeActionPolicy != "" {
		decision := "EXECUTE_IMMEDIATELY"
		risky := (!behavior.ReadOnly && (behavior.Destructive || behavior.OpenWorld)) || connection.DataSharingPolicy == "review_every_call"
		if risky && connection.UnsafeActionPolicy == "always_ask" {
			decision = "HUMAN_REVIEW"
		} else if risky && connection.UnsafeActionPolicy == "reviewer_may_approve" {
			decision = "LLM_REVIEW"
		}
		result.DecisionPreview = &decision
	}
	return result
}

func (r *Resolver) adapterSnapshot(ctx context.Context) (*adapter.Service, adapter.ServiceSnapshot, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, adapter.ServiceSnapshot{}, err
	}
	snapshot, err := service.Snapshot()
	return service, snapshot, err
}
func findAdapter(snapshot adapter.ServiceSnapshot, connectionID string) (adapter.Definition, adapter.Connection, error) {
	for _, connection := range snapshot.Connections {
		if connection.ConnectionID == connectionID {
			for _, def := range snapshot.Definitions {
				if def.SemanticDigest == connection.SemanticDigest {
					return def, connection, nil
				}
			}
		}
	}
	return adapter.Definition{}, adapter.Connection{}, errors.New("adapter connection is unavailable")
}
func (r *Resolver) adapterDefinitions(ctx context.Context) ([]*model.AdapterDefinition, error) {
	_, snapshot, err := r.adapterSnapshot(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]*model.AdapterDefinition, len(snapshot.Definitions))
	result = result[:0]
	for _, value := range snapshot.Definitions {
		hasConnection := false
		for _, connection := range snapshot.Connections {
			hasConnection = hasConnection || connection.SemanticDigest == value.SemanticDigest
		}
		if value.Superseded && !hasConnection {
			continue
		}
		result = append(result, definitionModel(value, snapshot))
	}
	return result, nil
}
func (r *Resolver) adapterIntegrations(ctx context.Context) ([]*model.CapabilityIntegration, error) {
	_, snapshot, err := r.adapterSnapshot(ctx)
	if err != nil {
		return nil, err
	}
	result := []*model.CapabilityIntegration{}
	for _, def := range snapshot.Definitions {
		if !def.Manifest.Reviewed || def.Superseded {
			continue
		}
		item := &model.CapabilityIntegration{Kind: model.CapabilityIntegrationKindAPI, DefinitionID: def.Manifest.DefinitionID, Name: def.Manifest.DisplayName, SourceRevision: def.SemanticDigest, Reviewed: true, SourceSummary: def.Manifest.Origin}
		for _, c := range snapshot.Connections {
			if c.SemanticDigest == def.SemanticDigest {
				item.Connections = append(item.Connections, adapterCapabilityConnection(def, c))
			}
		}
		result = append(result, item)
	}
	return result, nil
}
func (r *Resolver) adapterConnection(ctx context.Context, ref model.CapabilityConnectionRefInput) (*model.CapabilityConnection, error) {
	_, snapshot, err := r.adapterSnapshot(ctx)
	if err != nil {
		return nil, err
	}
	def, c, err := findAdapter(snapshot, ref.ConnectionID)
	if err != nil {
		return nil, err
	}
	return adapterCapabilityConnection(def, c), nil
}
func (r *Resolver) adapterTools(ctx context.Context, ref model.CapabilityConnectionRefInput) ([]*model.CapabilityManagedTool, error) {
	_, snapshot, err := r.adapterSnapshot(ctx)
	if err != nil {
		return nil, err
	}
	def, c, err := findAdapter(snapshot, ref.ConnectionID)
	if err != nil {
		return nil, err
	}
	result := make([]*model.CapabilityManagedTool, len(def.Operations))
	for i, op := range def.Operations {
		result[i] = adapterToolModel(def, c, op)
	}
	return result, nil
}
func (r *Resolver) adapterManagement(ctx context.Context) (*model.AdapterManagement, error) {
	definitions, err := r.adapterDefinitions(ctx)
	if err != nil {
		return nil, err
	}
	integrations, err := r.adapterIntegrations(ctx)
	if err != nil {
		return nil, err
	}
	return &model.AdapterManagement{Definitions: definitions, OauthState: &model.AdapterOauthState{Profiles: []*model.AdapterOauthProfile{}, Applications: []*model.AdapterOauthApplication{}, Accounts: []*model.AdapterExternalAccount{}, Grants: []*model.AdapterAuthorizationGrant{}}, Integrations: integrations}, nil
}

func (r *Resolver) adapterOauthState(ctx context.Context) (*model.AdapterOauthState, error) {
	if _, err := r.requireAdapters(ctx); err != nil {
		return nil, err
	}
	return &model.AdapterOauthState{Profiles: []*model.AdapterOauthProfile{}, Applications: []*model.AdapterOauthApplication{}, Accounts: []*model.AdapterExternalAccount{}, Grants: []*model.AdapterAuthorizationGrant{}}, nil
}

func (r *Resolver) approveAdapterDefinition(ctx context.Context, input model.ApproveAdapterDefinitionInput) (*model.AdapterDefinition, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	def, err := service.Approve(ctx, input.SemanticDigest)
	if err != nil {
		return nil, err
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		return nil, err
	}
	return definitionModel(def, snapshot), nil
}
func (r *Resolver) cancelAdapterDefinition(ctx context.Context, input model.CancelAdapterDefinitionInput) (bool, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return false, err
	}
	return service.Cancel(ctx, input.SemanticDigest)
}
func (r *Resolver) deleteAdapterConnection(ctx context.Context, input model.DeleteAdapterConnectionInput) (bool, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return false, err
	}
	return service.DeleteConnection(ctx, input.ConnectionID, input.ExpectedConnectionRevision)
}
func (r *Resolver) deleteAdapterService(ctx context.Context, input model.DeleteAdapterServiceInput) (bool, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return false, err
	}
	return service.DeleteService(ctx, input.DefinitionID, input.ExpectedSourceRevision)
}
func (r *Resolver) setAdapterConnectionActive(ctx context.Context, input model.SetAdapterConnectionActiveInput) (*model.AdapterDefinition, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	c, err := service.SetActive(ctx, input.ConnectionID, input.ExpectedConnectionRevision, input.Active)
	if err != nil {
		return nil, err
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		return nil, err
	}
	def, _, err := findAdapter(snapshot, c.ConnectionID)
	if err != nil {
		return nil, err
	}
	return definitionModel(def, snapshot), nil
}
func (r *Resolver) saveAdapterConnectionPolicy(ctx context.Context, input model.SaveCapabilityConnectionPolicyInput) (*model.CapabilityConnection, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	c, err := service.SaveConnectionPolicy(ctx, input.ConnectionID, input.ExpectedConnectionRevision, input.ExpectedPolicyRevision, input.DataSharingPolicy, input.UnsafeActionPolicy)
	if err != nil {
		return nil, err
	}
	snapshot, _ := service.Snapshot()
	def, _, err := findAdapter(snapshot, c.ConnectionID)
	if err != nil {
		return nil, err
	}
	return adapterCapabilityConnection(def, c), nil
}
func (r *Resolver) saveAdapterConnectionLabel(ctx context.Context, input model.SaveCapabilityConnectionLabelInput) (*model.CapabilityConnection, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	c, err := service.SaveConnectionLabel(ctx, input.ConnectionID, input.ExpectedConnectionRevision, input.ExpectedConnectionLabel, input.ConnectionLabel)
	if err != nil {
		return nil, err
	}
	snapshot, _ := service.Snapshot()
	def, _, err := findAdapter(snapshot, c.ConnectionID)
	if err != nil {
		return nil, err
	}
	return adapterCapabilityConnection(def, c), nil
}
func (r *Resolver) changeAdapterTool(ctx context.Context, input any) (*model.CapabilityManagedTool, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	var id, revision, tool, source string
	var expected int
	var enabled *bool
	var behavior *store.ActionBehavior
	reset := false
	switch v := input.(type) {
	case model.SaveCapabilityToolOverrideInput:
		id, revision, tool, source, expected = v.ConnectionID, v.ExpectedConnectionRevision, v.ToolID, v.SourceRevision, v.ExpectedPolicyRevision
		behavior = &store.ActionBehavior{ReadOnly: v.ReadOnly, RepeatSafe: v.Idempotent, Destructive: v.Destructive, OpenWorld: v.OpenWorld}
	case model.SetCapabilityToolEnabledInput:
		id, revision, tool, source, expected = v.ConnectionID, v.ExpectedConnectionRevision, v.ToolID, v.SourceRevision, v.ExpectedPolicyRevision
		enabled = &v.Enabled
	case model.ResetCapabilityToolPolicyInput:
		id, revision, tool, source, expected, reset = v.ConnectionID, v.ExpectedConnectionRevision, v.ToolID, v.SourceRevision, v.ExpectedPolicyRevision, true
	}
	if _, err = service.ChangeTool(ctx, id, revision, tool, source, expected, enabled, behavior, reset); err != nil {
		return nil, err
	}
	snapshot, _ := service.Snapshot()
	def, c, err := findAdapter(snapshot, id)
	if err != nil {
		return nil, err
	}
	for _, op := range def.Operations {
		if op.OperationID == tool {
			return adapterToolModel(def, c, op), nil
		}
	}
	return nil, errors.New("adapter operation is unavailable")
}
