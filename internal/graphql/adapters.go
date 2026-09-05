package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
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
	var previousAuthentication *adapter.Authentication
	for _, digest := range value.Replaces {
		for _, prior := range snapshot.Definitions {
			if prior.SemanticDigest == digest {
				copy := prior.Manifest.Authentication
				previousAuthentication = &copy
				for _, operation := range prior.Operations {
					previous[operation.OperationID] = operation.Digest
				}
			}
		}
	}
	if previousAuthentication != nil {
		left, _ := json.Marshal(*previousAuthentication)
		right, _ := json.Marshal(value.Manifest.Authentication)
		transition.AuthenticationChanged = string(left) != string(right)
		if transition.AuthenticationChanged {
			transition.AuthenticationRequiredConnections = len(value.AffectedConnections)
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
		SourceReference: value.SourceReference, Origin: value.Manifest.Origin, AuthenticationMode: value.Manifest.Authentication.Kind, Scopes: []string{},
		Transition:   transition,
		ManifestJSON: string(manifest), Reviewed: value.Manifest.Reviewed, Superseded: value.Superseded,
		Connections: []*model.AdapterConnection{}, ConnectionActions: []*model.AdapterNextAction{}}
	if value.Manifest.Authentication.Kind == "credential" {
		result.CredentialSetup = credentialSetupModel(value.Manifest.Authentication)
	}
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
			if connection.Status == "authentication_required" && value.Manifest.Authentication.Kind == "credential" {
				revision := connection.ConnectionRevision
				action := &model.AdapterNextAction{Kind: "set_up_credential", SemanticDigest: value.SemanticDigest, ConnectionID: &connection.ConnectionID, ExpectedConnectionRevision: &revision, OperationIds: []string{}, MissingScopes: []string{}}
				result.ConnectionActions = append(result.ConnectionActions, action)
				result.NextAction = action
			}
			if connection.DataSharingPolicy == "" || connection.UnsafeActionPolicy == "" {
				revision, policy := connection.ConnectionRevision, connection.PolicyRevision
				result.ConnectionActions = append(result.ConnectionActions, &model.AdapterNextAction{Kind: "review_connection_policy", SemanticDigest: value.SemanticDigest, ConnectionID: &connection.ConnectionID, ExpectedConnectionRevision: &revision, ExpectedPolicyRevision: &policy, OperationIds: []string{}, MissingScopes: []string{}})
			}
		}
	}
	if value.Manifest.Reviewed && !value.Superseded && value.Manifest.Authentication.Kind == "credential" && len(result.Connections) == 0 {
		result.NextAction = &model.AdapterNextAction{Kind: "set_up_credential", SemanticDigest: value.SemanticDigest, OperationIds: []string{}, MissingScopes: []string{}}
	}
	result.ConnectionCount = len(result.Connections)
	if !value.Manifest.Reviewed && !value.Superseded {
		result.NextAction = &model.AdapterNextAction{Kind: "review_definition", SemanticDigest: value.SemanticDigest, OperationIds: []string{}, MissingScopes: []string{}}
	}
	return result
}

func credentialSetupModel(auth adapter.Authentication) *model.AdapterCredentialSetup {
	setup := auth.Setup
	if setup == nil {
		return nil
	}
	result := &model.AdapterCredentialSetup{CredentialType: setup.CredentialType, SetupURL: setup.SetupURL, Instructions: setup.Instructions, InputKind: setup.Input.Kind, Fields: []*model.AdapterCredentialField{}}
	for _, field := range setup.Input.Fields {
		result.Fields = append(result.Fields, &model.AdapterCredentialField{FieldID: field.ID, Label: field.Label})
	}
	transform := func(value *adapter.Transform) *model.AdapterCredentialTransform {
		if value == nil {
			return nil
		}
		digest := sha256.Sum256([]byte(value.Source))
		return &model.AdapterCredentialTransform{Language: "lua", SourceDigest: hex.EncodeToString(digest[:]), Source: value.Source}
	}
	if setup.Input.Kind == "document" {
		media := setup.Input.MediaType
		result.DocumentMediaType = &media
		result.NormalizationTransform = transform(setup.Input.Normalize)
	}
	result.RequestAuthTransform = transform(auth.RequestAuth)
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
		CredentialRevision: credentialRevision(value), PolicyConfigured: value.DataSharingPolicy != "" && value.UnsafeActionPolicy != "", OperationAccess: access}
}

func credentialRevision(value adapter.Connection) *int {
	if value.Authentication.Kind != "credential" || value.Authentication.Revision == 0 {
		return nil
	}
	revision := value.Authentication.Revision
	return &revision
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
		HealthStatus: "unknown", AuthStatus: adapterAuthStatus(value), CredentialRevision: credentialRevision(value), DataSharingPolicy: sharing, UnsafeActionPolicy: unsafe,
		ToolCount: len(def.Operations), AvailableToolCount: available, DisabledToolCount: disabled, SourceDetails: map[string]any{"origin": def.Manifest.Origin}}
}

func adapterAuthStatus(value adapter.Connection) string {
	if value.Authentication.Kind == "none" {
		return "not_required"
	}
	if value.Status == "authentication_required" {
		return "authentication_required"
	}
	return "active"
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

func (r *Resolver) setupAdapterConnection(ctx context.Context, input model.SetupAdapterConnectionInput) (*model.AdapterDefinition, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	if len(input.FieldValues) > 16 {
		return nil, errors.New("adapter credential fields are invalid")
	}
	fields := make(map[string]string, len(input.FieldValues))
	seen := make(map[string]bool, len(input.FieldValues))
	for _, field := range input.FieldValues {
		if field == nil || seen[field.FieldID] {
			return nil, errors.New("adapter credential fields are invalid")
		}
		seen[field.FieldID] = true
		fields[field.FieldID] = field.Value
	}
	var document []byte
	if input.DocumentBase64 != nil {
		if len(*input.DocumentBase64) > 176<<10 {
			return nil, errors.New("adapter credential document is too large")
		}
		document, err = base64.StdEncoding.DecodeString(*input.DocumentBase64)
		if err != nil {
			return nil, errors.New("adapter credential document is invalid")
		}
	}
	replacement := ""
	if input.ReplacementConnectionID != nil {
		replacement = *input.ReplacementConnectionID
	}
	definition, _, err := service.SetupCredentialConnection(ctx, input.SemanticDigest, replacement, adapter.CredentialInputValue{FieldValues: fields, Document: document})
	if err != nil {
		return nil, err
	}
	if replacement != "" {
		var resumeErr error
		if r.TaskExecution != nil {
			_, taskErr := r.TaskExecution.ResumeAdapterAuthentication(ctx, replacement)
			resumeErr = errors.Join(resumeErr, taskErr)
		}
		if r.Chat != nil {
			resumeErr = errors.Join(resumeErr, r.Chat.ResumeAdapterAuthentication(ctx, replacement))
		}
		if resumeErr != nil {
			return nil, resumeErr
		}
	}
	snapshot, err := service.Snapshot()
	if err != nil {
		return nil, err
	}
	return definitionModel(definition, snapshot), nil
}

func adapterAuthenticationModel(value store.MCPAuthRequest, displayName string) *model.AdapterAuthenticationIntervention {
	states := map[string]model.McpAuthenticationRequestState{"awaiting_user": model.McpAuthenticationRequestStateAwaitingUser,
		"authorizing": model.McpAuthenticationRequestStateAuthorizing, "resuming": model.McpAuthenticationRequestStateResuming,
		"completed": model.McpAuthenticationRequestStateCompleted, "cancelled": model.McpAuthenticationRequestStateCancelled,
		"superseded": model.McpAuthenticationRequestStateSuperseded}
	result := &model.AdapterAuthenticationIntervention{RequestID: value.ID, Revision: value.Revision, ServiceDisplayName: displayName, CapabilityName: value.CapabilityName, State: states[value.State]}
	if value.TaskID != "" {
		result.TaskID = &value.TaskID
	}
	if value.Failure != "" {
		result.FailureCode = &value.Failure
	}
	return result
}

func (r *Resolver) pendingAdapterAuthentications(ctx context.Context, conversationID, taskID *string, limit int) ([]model.HumanIntervention, error) {
	values, err := r.Store.PendingAdapterAuthRequests(ctx, localHumanID, conversationID, taskID, limit)
	if err != nil {
		return nil, err
	}
	snapshot, err := r.Adapters.Snapshot()
	if err != nil {
		return nil, err
	}
	names := map[string]string{}
	for _, connection := range snapshot.Connections {
		for _, definition := range snapshot.Definitions {
			if definition.SemanticDigest == connection.SemanticDigest {
				names[connection.ConnectionID] = adapter.DisplayName(definition)
			}
		}
	}
	result := make([]model.HumanIntervention, 0, len(values))
	for _, value := range values {
		display := names[value.AuthorityID]
		if display == "" {
			display = value.AuthorityID
		}
		result = append(result, adapterAuthenticationModel(value, display))
	}
	return result, nil
}

func (r *Resolver) skipAdapterAuthentication(ctx context.Context, input model.SkipAdapterAuthenticationInput) (*model.AdapterAuthenticationIntervention, error) {
	if _, err := r.requireAdapters(ctx); err != nil {
		return nil, err
	}
	request, err := r.Store.MCPAuthRequest(ctx, input.RequestID, input.ExpectedRevision)
	if err != nil || request.AuthorityKind != "adapter_connection" || request.OwnerHumanID != localHumanID {
		return nil, errors.New("adapter authentication request is unavailable")
	}
	if request.TaskID != "" {
		if r.TaskExecution == nil {
			return nil, errors.New("Task execution runtime is unavailable")
		}
		request, err = r.TaskExecution.SkipMCPAuthentication(ctx, request)
	} else {
		if r.Chat == nil {
			return nil, errors.New("Chat runtime is unavailable")
		}
		request, err = r.Chat.SkipAdapterAuthentication(ctx, request.ID, request.Revision)
	}
	if err != nil {
		return nil, err
	}
	display := request.AuthorityID
	if snapshot, loadErr := r.Adapters.Snapshot(); loadErr == nil {
		for _, connection := range snapshot.Connections {
			if connection.ConnectionID == request.AuthorityID {
				for _, definition := range snapshot.Definitions {
					if definition.SemanticDigest == connection.SemanticDigest {
						display = adapter.DisplayName(definition)
					}
				}
			}
		}
	}
	return adapterAuthenticationModel(request, display), nil
}

func (r *Resolver) startAdapterAuthentication(ctx context.Context, input model.StartAdapterAuthenticationInput) (*model.AdapterOauthSetupAttempt, error) {
	if _, err := r.requireAdapters(ctx); err != nil {
		return nil, err
	}
	request, err := r.Store.MCPAuthRequest(ctx, input.RequestID, input.ExpectedRevision)
	if err != nil || request.AuthorityKind != "adapter_connection" || request.OwnerHumanID != localHumanID {
		return nil, errors.New("adapter authentication request is unavailable")
	}
	return nil, errors.New("direct adapter credentials must be replaced")
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
