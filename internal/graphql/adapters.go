package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"sort"
	"strconv"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) requireAdapters(ctx context.Context) (*adapter.Service, error) {
	_, browser := auth.BrowserSessionHash(ctx)
	if (!browser && !auth.DesktopAccess(ctx) && auth.ClientID(ctx) == "") || r.Adapters == nil {
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
	} else if value.Manifest.Authentication.Kind == "oauth2_authorization_code_pkce" {
		result.OauthProfileDigest = &value.Manifest.Authentication.ProfileDigest
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
			transform = &model.AdapterResponseTransform{Language: "luau", SourceDigest: hex.EncodeToString(digest[:]), Source: operation.Response.Transform.Source,
				AcceptedContentTypes: operation.Response.AcceptedContentTypes, OutputSchemaJSON: string(out)}
		}
		readOnly, repeatSafe, destructive, openWorld := operation.Behavior.ReadOnly, operation.Behavior.RepeatSafe, operation.Behavior.Destructive, operation.Behavior.OpenWorld
		scopeSets := []*model.AdapterScopeSet{}
		for _, set := range operation.Authorization.AcceptedScopeSets {
			scopeSets = append(scopeSets, &model.AdapterScopeSet{Scopes: set})
			result.Scopes = append(result.Scopes, set...)
		}
		result.Operations = append(result.Operations, &model.AdapterOperation{OperationID: operation.OperationID, Method: operation.Method,
			Path: operation.Path, ReadOnly: &readOnly, Idempotent: &repeatSafe,
			Destructive: &destructive, OpenWorld: &openWorld, ArgumentNames: arguments,
			ResponseTransform: transform, AcceptedScopeSets: scopeSets})
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
	sort.Strings(result.Scopes)
	result.Scopes = uniqueGraphQLStrings(result.Scopes)
	if !value.Manifest.Reviewed && !value.Superseded {
		result.NextAction = &model.AdapterNextAction{Kind: "review_definition", SemanticDigest: value.SemanticDigest, OperationIds: []string{}, MissingScopes: []string{}}
	}
	return result
}
func uniqueGraphQLStrings(values []string) []string {
	if len(values) == 0 {
		return values
	}
	out := values[:1]
	for _, v := range values[1:] {
		if v != out[len(out)-1] {
			out = append(out, v)
		}
	}
	return out
}

func credentialSetupModel(auth adapter.Authentication) *model.AdapterCredentialSetup {
	setup := auth.Setup
	return credentialSetupValue(setup, auth.RequestAuth)
}
func credentialSetupValue(setup *adapter.CredentialSetup, request *adapter.Transform) *model.AdapterCredentialSetup {
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
		return &model.AdapterCredentialTransform{Language: "luau", SourceDigest: hex.EncodeToString(digest[:]), Source: value.Source}
	}
	if setup.Input.Kind == "document" {
		media := setup.Input.MediaType
		result.DocumentMediaType = &media
		result.NormalizationTransform = transform(setup.Input.Normalize)
	}
	result.RequestAuthTransform = transform(request)
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
	service, snapshot, err := r.adapterSnapshot(ctx)
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
		view := definitionModel(value, snapshot)
		if value.Manifest.Authentication.Kind == "oauth2_authorization_code_pkce" {
			oauth, oauthErr := service.OAuthSnapshot()
			if oauthErr != nil {
				return nil, oauthErr
			}
			projectOAuthDefinition(view, value, snapshot, oauth)
		}
		result = append(result, view)
	}
	return result, nil
}

func (r *Resolver) adapterDefinition(ctx context.Context, digest string) (*model.AdapterDefinition, error) {
	definitions, err := r.adapterDefinitions(ctx)
	if err != nil {
		return nil, err
	}
	for _, definition := range definitions {
		if definition.SemanticDigest == digest {
			return definition, nil
		}
	}
	return nil, errors.New("adapter definition is unavailable")
}

func projectOAuthDefinition(view *model.AdapterDefinition, definition adapter.Definition, snapshot adapter.ServiceSnapshot, oauth adapter.OAuthSnapshot) {
	for _, profile := range oauth.Profiles {
		if profile.ProfileDigest == definition.Manifest.Authentication.ProfileDigest && len(profile.Setups) != 0 {
			setup := profile.Setups[0].Setup
			view.CredentialSetup = credentialSetupValue(&setup, nil)
			break
		}
	}
	apps := map[string]adapter.OAuthApplication{}
	for _, app := range oauth.Applications {
		if app.ProfileDigest == definition.Manifest.Authentication.ProfileDigest && app.Status == "active" {
			apps[app.ApplicationID] = app
		}
	}
	grants := map[string]adapter.OAuthGrant{}
	for _, grant := range oauth.Grants {
		grants[grant.GrantID] = grant
	}
	connected := map[string]bool{}
	var reconnectAction, accessAction *model.AdapterNextAction
	for _, connection := range snapshot.Connections {
		if connection.SemanticDigest != definition.SemanticDigest {
			continue
		}
		if connection.Status == "authentication_required" && connection.Authentication.GrantID == "" {
			for _, app := range oauth.Applications {
				if app.ProfileDigest != definition.Manifest.Authentication.ProfileDigest || app.Status != "active" {
					continue
				}
				appID, appRevision, connectionRevision := app.ApplicationID, app.Revision, connection.ConnectionRevision
				action := &model.AdapterNextAction{Kind: "reconnect_account", SemanticDigest: definition.SemanticDigest, ApplicationID: &appID, ExpectedApplicationRevision: &appRevision, ConnectionID: &connection.ConnectionID, ExpectedConnectionRevision: &connectionRevision, OperationIds: operationIDs(definition), MissingScopes: []string{}}
				view.ConnectionActions = append(view.ConnectionActions, action)
				if reconnectAction == nil {
					reconnectAction = action
				}
				break
			}
			continue
		}
		connected[connection.Authentication.GrantID] = true
		grant, ok := grants[connection.Authentication.GrantID]
		if !ok {
			continue
		}
		for _, projected := range view.Connections {
			if projected.ConnectionID == connection.ConnectionID {
				missingOperations, missingScopes := []string{}, []string{}
				projected.GrantID = &grant.GrantID
				projected.GrantRevision = &grant.AuthorityRevision
				projected.AccountID = grant.AccountID
				projected.GrantedScopes = grant.GrantedScopes
				for _, access := range projected.OperationAccess {
					if grant.Status != "active" {
						continue
					}
					for _, op := range definition.Operations {
						if op.OperationID == access.OperationID {
							target, valid := definition.ScopeTarget([]string{op.OperationID}, grant.GrantedScopes)
							if !valid {
								continue
							}
							missing := []string{}
							present := map[string]bool{}
							for _, scope := range grant.GrantedScopes {
								present[scope] = true
							}
							for _, scope := range target {
								if !present[scope] {
									missing = append(missing, scope)
								}
							}
							access.MissingScopes = missing
							if len(missing) > 0 {
								access.Status = "additional_authorization_required"
								missingOperations = append(missingOperations, op.OperationID)
								missingScopes = append(missingScopes, missing...)
							}
						}
					}
				}
				if len(missingOperations) > 0 {
					appID, grantRevision, connectionRevision := grant.ApplicationID, grant.AuthorityRevision, connection.ConnectionRevision
					sort.Strings(missingScopes)
					action := &model.AdapterNextAction{Kind: "add_access", SemanticDigest: definition.SemanticDigest, ApplicationID: &appID, GrantID: &grant.GrantID, ExpectedGrantRevision: &grantRevision, ConnectionID: &connection.ConnectionID, ExpectedConnectionRevision: &connectionRevision, OperationIds: missingOperations, MissingScopes: uniqueGraphQLStrings(missingScopes)}
					if app, ok := apps[grant.ApplicationID]; ok {
						applicationRevision := app.Revision
						action.ExpectedApplicationRevision = &applicationRevision
					}
					view.ConnectionActions = append(view.ConnectionActions, action)
					if accessAction == nil {
						accessAction = action
					}
				}
			}
		}
		if connection.Status == "authentication_required" || grant.Status != "active" {
			appID := grant.ApplicationID
			revision := grant.AuthorityRevision
			action := &model.AdapterNextAction{Kind: "reconnect_account", SemanticDigest: definition.SemanticDigest, ApplicationID: &appID, GrantID: &grant.GrantID, ExpectedGrantRevision: &revision, OperationIds: connection.AllowedOperations, MissingScopes: []string{}}
			if app, ok := apps[grant.ApplicationID]; ok {
				applicationRevision := app.Revision
				action.ExpectedApplicationRevision = &applicationRevision
			}
			view.ConnectionActions = append(view.ConnectionActions, action)
			if reconnectAction == nil {
				reconnectAction = action
			}
		}
	}
	if !definition.Manifest.Reviewed || definition.Superseded {
		return
	}
	if reconnectAction != nil {
		view.NextAction = reconnectAction
		return
	}
	if accessAction != nil {
		view.NextAction = accessAction
		return
	}
	if len(apps) == 0 {
		view.NextAction = &model.AdapterNextAction{Kind: "import_application", SemanticDigest: definition.SemanticDigest, OperationIds: []string{}, MissingScopes: []string{}}
		return
	}
	if len(view.Connections) != 0 {
		return
	}
	operations := operationIDs(definition)
	var attach, expand, reconnect []*model.AdapterNextAction
	for _, grant := range oauth.Grants {
		app, selected := apps[grant.ApplicationID]
		if !selected || connected[grant.GrantID] {
			continue
		}
		appID, appRevision, grantRevision := app.ApplicationID, app.Revision, grant.AuthorityRevision
		action := &model.AdapterNextAction{SemanticDigest: definition.SemanticDigest, ApplicationID: &appID, ExpectedApplicationRevision: &appRevision, GrantID: &grant.GrantID, ExpectedGrantRevision: &grantRevision, OperationIds: operations, MissingScopes: []string{}}
		if grant.Status != "active" {
			action.Kind = "reconnect_account"
			reconnect = append(reconnect, action)
			continue
		}
		sufficient := true
		for _, operation := range definition.Operations {
			target, valid := definition.ScopeTarget([]string{operation.OperationID}, grant.GrantedScopes)
			sufficient = sufficient && valid && len(target) == len(grant.GrantedScopes)
		}
		if sufficient {
			action.Kind = "attach_account"
			attach = append(attach, action)
			continue
		}
		target, valid := definition.ScopeTarget(operations, grant.GrantedScopes)
		if !valid {
			continue
		}
		present := map[string]bool{}
		for _, scope := range grant.GrantedScopes {
			present[scope] = true
		}
		for _, scope := range target {
			if !present[scope] {
				action.MissingScopes = append(action.MissingScopes, scope)
			}
		}
		action.Kind = "add_access"
		expand = append(expand, action)
	}
	actions := append(attach, expand...)
	actions = append(actions, reconnect...)
	for _, app := range oauth.Applications {
		selected, ok := apps[app.ApplicationID]
		if !ok {
			continue
		}
		appID, revision := selected.ApplicationID, selected.Revision
		actions = append(actions, &model.AdapterNextAction{Kind: "add_account", SemanticDigest: definition.SemanticDigest, ApplicationID: &appID, ExpectedApplicationRevision: &revision, OperationIds: operations, MissingScopes: view.Scopes})
	}
	if len(actions) != 0 {
		view.ConnectionActions = append(view.ConnectionActions, actions...)
		view.NextAction = actions[0]
	}
}
func operationIDs(definition adapter.Definition) []string {
	result := make([]string, len(definition.Operations))
	for i, value := range definition.Operations {
		result[i] = value.OperationID
	}
	return result
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
	oauth, err := r.adapterOauthState(ctx)
	if err != nil {
		return nil, err
	}
	return &model.AdapterManagement{Definitions: definitions, OauthState: oauth, Integrations: integrations}, nil
}

func (r *Resolver) adapterOauthState(ctx context.Context) (*model.AdapterOauthState, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	snapshot, err := service.OAuthSnapshot()
	if err != nil {
		return nil, err
	}
	result := &model.AdapterOauthState{Profiles: []*model.AdapterOauthProfile{}, Applications: []*model.AdapterOauthApplication{}, Accounts: []*model.AdapterExternalAccount{}, Grants: []*model.AdapterAuthorizationGrant{}}
	profileNames := map[string]string{}
	callbackMode, callbackURI := service.OAuthCallback()
	for _, value := range snapshot.Profiles {
		profileNames[value.ProfileDigest] = value.DisplayName
		var setup *adapter.CredentialSetup
		for i := range value.Setups {
			if value.Setups[i].CallbackMode == callbackMode {
				setup = &value.Setups[i].Setup
			}
		}
		credentialSetup := credentialSetupValue(setup, nil)
		if credentialSetup != nil && callbackMode == "hosted" {
			credentialSetup.RedirectURI = &callbackURI
		}
		result.Profiles = append(result.Profiles, &model.AdapterOauthProfile{ProfileDigest: value.ProfileDigest, ProfileID: value.ProfileID, DisplayName: value.DisplayName, GrantAudience: value.GrantAudience, CredentialSetup: credentialSetup})
	}
	grantCount, accountCount := map[string]int{}, map[string]int{}
	for _, grant := range snapshot.Grants {
		grantCount[grant.ApplicationID]++
		if grant.AccountID != nil {
			accountCount[grant.ApplicationID]++
		}
	}
	for _, value := range snapshot.Applications {
		result.Applications = append(result.Applications, &model.AdapterOauthApplication{ApplicationID: value.ApplicationID, ProfileDigest: value.ProfileDigest, ProviderDisplayName: profileNames[value.ProfileDigest], CallbackMode: value.CallbackMode, ClientID: value.ClientID, ProjectLabel: value.ProjectLabel, Revision: value.Revision, Status: value.Status, GrantCount: grantCount[value.ApplicationID], AccountCount: accountCount[value.ApplicationID]})
	}
	for _, value := range snapshot.Accounts {
		ids := []string{}
		for _, grant := range snapshot.Grants {
			if grant.AccountID != nil && *grant.AccountID == value.AccountID {
				ids = append(ids, grant.GrantID)
			}
		}
		result.Accounts = append(result.Accounts, &model.AdapterExternalAccount{AccountID: value.AccountID, ProfileDigest: value.ProfileDigest, AccountLabel: value.AccountLabel, Revision: value.Revision, GrantIds: ids})
	}
	connections, err := service.Snapshot()
	if err != nil {
		return nil, err
	}
	for _, value := range snapshot.Grants {
		ids := []string{}
		for _, connection := range connections.Connections {
			if connection.Authentication.GrantID == value.GrantID {
				ids = append(ids, connection.ConnectionID)
			}
		}
		name := "Google"
		result.Grants = append(result.Grants, &model.AdapterAuthorizationGrant{GrantID: value.GrantID, ApplicationID: value.ApplicationID, AccountID: value.AccountID, AccountLabel: value.AccountLabel, ProviderDisplayName: name, Audience: value.Audience, DesiredScopes: value.DesiredScopes, GrantedScopes: value.GrantedScopes, AuthorityRevision: value.AuthorityRevision, TokenRevision: value.TokenRevision, Status: value.Status, ConnectionIds: ids})
	}
	return result, nil
}

func decodeAdapterOAuthDocument(encoded string) ([]byte, error) {
	if len(encoded) > 176<<10 {
		return nil, errors.New("adapter OAuth client document is too large")
	}
	raw, err := base64.StdEncoding.DecodeString(encoded)
	if err != nil || len(raw) == 0 || len(raw) > 128<<10 {
		return nil, errors.New("adapter OAuth client document is invalid")
	}
	return raw, nil
}
func oauthApplicationModel(ctx context.Context, r *Resolver, id string) (*model.AdapterOauthApplication, error) {
	state, err := r.adapterOauthState(ctx)
	if err != nil {
		return nil, err
	}
	for _, value := range state.Applications {
		if value.ApplicationID == id {
			return value, nil
		}
	}
	return nil, errors.New("adapter OAuth application is unavailable")
}
func oauthGrantModel(ctx context.Context, r *Resolver, id string) (*model.AdapterAuthorizationGrant, error) {
	state, err := r.adapterOauthState(ctx)
	if err != nil {
		return nil, err
	}
	for _, value := range state.Grants {
		if value.GrantID == id {
			return value, nil
		}
	}
	return nil, errors.New("adapter OAuth grant is unavailable")
}
func adapterOAuthAttemptModel(value adapter.OAuthAttemptEvent) *model.AdapterOauthAttemptEvent {
	result := &model.AdapterOauthAttemptEvent{AttemptID: value.AttemptID, Status: value.Status}
	if value.SemanticDigest != "" {
		result.SemanticDigest = &value.SemanticDigest
	}
	if value.GrantID != "" {
		result.GrantID = &value.GrantID
	}
	if value.GrantRevision != 0 {
		result.GrantRevision = &value.GrantRevision
	}
	return result
}
func (r *Resolver) importAdapterOAuthApplication(ctx context.Context, input model.ImportAdapterOauthApplicationInput) (*model.AdapterOauthApplication, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	raw, err := decodeAdapterOAuthDocument(input.ClientDocumentBase64)
	if err != nil {
		return nil, err
	}
	var profileDocument []byte
	if input.ProfileDocumentJSON != nil {
		profileDocument = []byte(*input.ProfileDocumentJSON)
	}
	value, err := service.ImportOAuthApplication(input.ProfileDigest, input.ProjectLabel, raw, profileDocument)
	if err != nil {
		return nil, err
	}
	return oauthApplicationModel(ctx, r, value.ApplicationID)
}
func (r *Resolver) replaceAdapterOAuthApplication(ctx context.Context, input model.ReplaceAdapterOauthApplicationInput) (*model.AdapterOauthApplication, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	raw, err := decodeAdapterOAuthDocument(input.ClientDocumentBase64)
	if err != nil {
		return nil, err
	}
	value, err := service.ReplaceOAuthApplication(input.ApplicationID, input.ExpectedRevision, raw)
	if err != nil {
		return nil, err
	}
	return oauthApplicationModel(ctx, r, value.ApplicationID)
}
func (r *Resolver) startAdapterOAuthSetup(ctx context.Context, input model.StartAdapterOauthSetupInput) (*model.AdapterOauthSetupAttempt, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	start := adapter.OAuthStart{ApplicationID: input.ApplicationID, ExpectedApplicationRevision: input.ExpectedApplicationRevision, SemanticDigest: input.SemanticDigest, OperationIDs: input.OperationIds}
	if input.GrantID != nil {
		start.GrantID = *input.GrantID
	}
	if input.ExpectedGrantRevision != nil {
		start.ExpectedGrantRevision = *input.ExpectedGrantRevision
	}
	for _, extra := range input.AdditionalServices {
		if extra == nil {
			continue
		}
		start.Additional = append(start.Additional, adapter.OAuthServiceSelection{SemanticDigest: extra.SemanticDigest, OperationIDs: extra.OperationIds})
	}
	value, err := service.StartOAuth(start)
	if err != nil {
		return nil, err
	}
	return &model.AdapterOauthSetupAttempt{AttemptID: value.AttemptID, AuthorizationURL: value.AuthorizationURL, ExpiresAtEpochSeconds: int(value.ExpiresAt)}, nil
}
func (r *Resolver) attachAdapterOAuthConnection(ctx context.Context, input model.AttachAdapterOauthConnectionInput) (*model.AdapterDefinition, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	replacement := ""
	if input.ReplacementConnectionID != nil {
		replacement = *input.ReplacementConnectionID
	}
	definition, _, err := service.AttachOAuthConnection(ctx, input.SemanticDigest, input.GrantID, input.ExpectedGrantRevision, replacement)
	if err != nil {
		return nil, err
	}
	return r.adapterDefinition(ctx, definition.SemanticDigest)
}
func (r *Resolver) disconnectAdapterOAuthGrant(ctx context.Context, input model.DisconnectAdapterOauthGrantInput) (*model.AdapterAuthorizationGrant, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	value, err := service.DisconnectOAuthGrant(ctx, input.GrantID, input.ExpectedAuthorityRevision)
	if err != nil {
		return nil, err
	}
	return oauthGrantModel(ctx, r, value.GrantID)
}
func (r *Resolver) labelAdapterOAuthGrant(ctx context.Context, input model.SaveAdapterOauthGrantLabelInput) (*model.AdapterAuthorizationGrant, error) {
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	value, err := service.LabelOAuthGrant(input.GrantID, input.ExpectedAuthorityRevision, input.AccountLabel)
	if err != nil {
		return nil, err
	}
	return oauthGrantModel(ctx, r, value.GrantID)
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
	return r.adapterDefinition(ctx, def.SemanticDigest)
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
	return r.adapterDefinition(ctx, definition.SemanticDigest)
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
		connectionID := value.AuthorityID
		if value.AdapterConnectionID != "" {
			connectionID = value.AdapterConnectionID
		}
		display := names[connectionID]
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
	if err != nil || request.AuthorityKind != "adapter_connection" && request.AuthorityKind != "adapter_grant" || request.OwnerHumanID != localHumanID {
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
	service, err := r.requireAdapters(ctx)
	if err != nil {
		return nil, err
	}
	request, err := r.Store.MCPAuthRequest(ctx, input.RequestID, input.ExpectedRevision)
	if err != nil || request.OwnerHumanID != localHumanID {
		return nil, errors.New("adapter authentication request is unavailable")
	}
	if request.AuthorityKind == "adapter_connection" {
		return nil, errors.New("direct adapter credentials must be replaced")
	}
	if request.AuthorityKind != "adapter_grant" || request.State != "awaiting_user" {
		return nil, errors.New("adapter authentication request is unavailable")
	}
	oauth, err := service.OAuthSnapshot()
	if err != nil {
		return nil, err
	}
	var grant *adapter.OAuthGrant
	var app *adapter.OAuthApplication
	for i := range oauth.Grants {
		if oauth.Grants[i].GrantID == request.AuthorityID {
			grant = &oauth.Grants[i]
			break
		}
	}
	if grant == nil {
		return nil, errors.New("adapter OAuth grant is unavailable")
	}
	for i := range oauth.Applications {
		if oauth.Applications[i].ApplicationID == grant.ApplicationID {
			app = &oauth.Applications[i]
			break
		}
	}
	if app == nil {
		return nil, errors.New("adapter OAuth application is unavailable")
	}
	var authority struct {
		Binding adapter.Binding `json:"binding"`
	}
	if json.Unmarshal([]byte(request.BindingJSON), &authority) != nil {
		return nil, errors.New("adapter authentication authority is invalid")
	}
	started, err := service.StartOAuth(adapter.OAuthStart{ApplicationID: app.ApplicationID, ExpectedApplicationRevision: app.Revision, GrantID: grant.GrantID, ExpectedGrantRevision: grant.AuthorityRevision, SemanticDigest: request.AdapterSemanticDigest, OperationIDs: []string{authority.Binding.OperationID}})
	if err != nil {
		return nil, err
	}
	if _, err = r.Store.BeginAdapterOAuthAuthentication(ctx, request, started.AttemptID, time.Now()); err != nil {
		return nil, err
	}
	return &model.AdapterOauthSetupAttempt{AttemptID: started.AttemptID, AuthorizationURL: started.AuthorizationURL, ExpiresAtEpochSeconds: int(started.ExpiresAt)}, nil
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
	return r.adapterDefinition(ctx, c.SemanticDigest)
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
	if c.Status == "active" {
		_ = r.Store.RecordCapabilityReady(ctx, "api", adapter.DisplayName(def), c.ConnectionID,
			fmt.Sprint(c.ConnectionRevision), len(c.AllowedOperations), time.Now())
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
