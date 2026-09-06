package graphql

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	"github.com/kpsuperplane/noema/internal/store"
)

const localHumanID = "human:local"

func (r *Resolver) requireMCP(ctx context.Context) (*noemamcp.Service, error) {
	_, browser := auth.BrowserSessionHash(ctx)
	if (!browser && !auth.DesktopAccess(ctx) && auth.ClientID(ctx) == "") || r.MCP == nil {
		return nil, errors.New("authenticated MCP service is unavailable")
	}
	return r.MCP, nil
}

func setupInput(input model.CreateMcpServerInput) (noemamcp.SetupInput, error) {
	value := noemamcp.SetupInput{DisplayName: input.DisplayName, TransportKind: input.TransportKind}
	if input.AuthPreference != nil {
		value.AuthPreference = input.AuthPreference.String()
	}
	switch input.TransportKind {
	case "stdio":
		if input.Stdio == nil || input.HTTP != nil {
			return value, errors.New("invalid MCP stdio setup input")
		}
		value.Command, value.Args = input.Stdio.Command, input.Stdio.Args
		if input.Stdio.Cwd != nil {
			value.Cwd = *input.Stdio.Cwd
		}
		var err error
		if value.Environment, err = stringMap(input.Stdio.Env); err != nil {
			return value, err
		}
		if value.Secrets.Environment, err = stringMap(input.Stdio.SecretEnv); err != nil {
			return value, err
		}
	case "streamable_http":
		if input.HTTP == nil || input.Stdio != nil {
			return value, errors.New("invalid MCP HTTP setup input")
		}
		value.URL = input.HTTP.URL
		var err error
		if value.Headers, err = stringMap(input.HTTP.Headers); err != nil {
			return value, err
		}
		if value.Secrets.Headers, err = stringMap(input.HTTP.SecretHeaders); err != nil {
			return value, err
		}
		value.Secrets.Client, err = oauthClient(input.HTTP.OauthClientCredentials)
		if err != nil {
			return value, err
		}
	default:
		return value, errors.New("invalid MCP transport kind")
	}
	return value, nil
}

func stringMap(input map[string]any) (map[string]string, error) {
	result := make(map[string]string, len(input))
	for key, raw := range input {
		value, ok := raw.(string)
		if !ok {
			return nil, errors.New("MCP configuration values must be strings")
		}
		result[key] = value
	}
	return result, nil
}

func oauthClient(input *model.McpOAuthClientCredentialsInput) (*noemamcp.OAuthClient, error) {
	if input == nil {
		return nil, nil
	}
	if strings.TrimSpace(input.ClientID) == "" || strings.TrimSpace(input.ClientSecret) == "" {
		return nil, errors.New("MCP OAuth client credentials are invalid")
	}
	return &noemamcp.OAuthClient{ClientID: input.ClientID, ClientSecret: input.ClientSecret, Scopes: input.Scopes}, nil
}

func (r *Resolver) createMCPServer(ctx context.Context, input model.CreateMcpServerInput) (*model.McpServerSetupResult, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	value, err := setupInput(input)
	if err != nil {
		return nil, err
	}
	result, err := service.Create(ctx, value)
	if err != nil {
		return nil, err
	}
	return mcpSetupModel(result), nil
}

func (r *Resolver) addMCPConnection(ctx context.Context, input model.AddMcpConnectionInput) (*model.McpServerSetupResult, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	secretEnv, err := stringMap(input.SecretEnv)
	if err != nil {
		return nil, err
	}
	secretHeaders, err := stringMap(input.SecretHeaders)
	if err != nil {
		return nil, err
	}
	client, err := oauthClient(input.OauthClientCredentials)
	if err != nil {
		return nil, err
	}
	value := noemamcp.SetupInput{DefinitionID: input.McpDefinitionID, DefinitionRevision: input.ExpectedDefinitionRevision,
		Secrets: noemamcp.SecretMaterial{Environment: secretEnv, Headers: secretHeaders, Client: client}}
	if input.ConnectionLabel != nil {
		value.ConnectionLabel = *input.ConnectionLabel
	}
	if input.AuthPreference != nil {
		value.AuthPreference = input.AuthPreference.String()
	}
	result, err := service.AddConnection(ctx, value)
	if err != nil {
		return nil, err
	}
	return mcpSetupModel(result), nil
}

func (r *Resolver) continueMCP(ctx context.Context, input model.ContinueMcpServerSetupInput) (*model.McpServerSetupResult, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	environment, err := stringMap(input.SecretEnv)
	if err != nil {
		return nil, err
	}
	headers, err := stringMap(input.SecretHeaders)
	if err != nil {
		return nil, err
	}
	client, err := oauthClient(input.OauthClientCredentials)
	if err != nil {
		return nil, err
	}
	result, err := service.Continue(ctx, input.McpServerID, noemamcp.SecretMaterial{Environment: environment, Headers: headers, Client: client})
	if err != nil {
		return nil, err
	}
	return mcpSetupModel(result), nil
}

func (r *Resolver) startMCPCreateOAuth(ctx context.Context, input model.StartMcpServerOAuthSetupInput) (*model.McpOAuthSetupAttempt, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	if input.Server == nil {
		return nil, errors.New("MCP server input is required")
	}
	value, err := setupInput(*input.Server)
	if err != nil {
		return nil, err
	}
	attempt, err := service.StartOAuthCreate(ctx, localHumanID, value, input.RedirectURI)
	if err != nil {
		return nil, err
	}
	return mcpAttemptModel(attempt), nil
}

func (r *Resolver) startMCPReauthOAuth(ctx context.Context, input model.StartMcpServerReauthenticationOAuthSetupInput) (*model.McpOAuthSetupAttempt, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	attempt, err := service.StartOAuthReauthentication(ctx, localHumanID, input.McpServerID, input.RedirectURI)
	if err != nil {
		return nil, err
	}
	return mcpAttemptModel(attempt), nil
}

func mcpSetupModel(value noemamcp.SetupResult) *model.McpServerSetupResult {
	result := &model.McpServerSetupResult{SetupStatus: value.Status, DiscoveredToolCount: value.Discovered}
	if value.Server != nil {
		result.Server = mcpServerModel(*value.Server)
	}
	if value.Error != "" {
		issue := value.Error
		result.SetupError = &issue
	}
	if value.OAuthSupported {
		result.Auth = &model.McpSetupAuthDetails{OauthAuthorizationSupported: true, OauthClientCredentialsSupported: true, Scopes: value.Scopes}
	}
	return result
}

func mcpAttemptModel(value noemamcp.OAuthAttempt) *model.McpOAuthSetupAttempt {
	result := &model.McpOAuthSetupAttempt{AttemptID: value.ID, Status: value.Status}
	if value.AuthorizationURL != "" {
		result.AuthorizationURL = &value.AuthorizationURL
	}
	if value.Error != "" {
		result.ErrorMessage = &value.Error
	}
	if value.Result != nil {
		result.SetupResult = mcpSetupModel(*value.Result)
	}
	return result
}

func mcpServerModel(value store.MCPServer) *model.McpServer {
	return &model.McpServer{McpServerID: value.ID, ConnectionRevision: value.ConnectionRevision,
		PolicyRevision: value.PolicyRevision, DisplayName: value.DisplayName, TransportKind: value.TransportKind,
		HealthStatus: value.HealthStatus, AuthStatus: value.AuthStatus, ToolCount: value.ToolCount,
		PendingToolCount: value.PendingToolCount, BrowserOauthReauthenticationSupported: value.TransportKind == "streamable_http"}
}

func mcpConnectionModel(value store.MCPServer) *model.CapabilityConnection {
	name := value.DisplayName
	var label *string
	if value.ConnectionLabel != "" {
		copy := value.ConnectionLabel
		label = &copy
		name = copy
	}
	var sharing, unsafe *string
	if value.DataSharingPolicy != "" {
		copy := value.DataSharingPolicy
		sharing = &copy
	}
	if value.UnsafeActionPolicy != "" {
		copy := value.UnsafeActionPolicy
		unsafe = &copy
	}
	status := "setup_required"
	if value.Enabled {
		status = "ready"
	}
	return &model.CapabilityConnection{Kind: model.CapabilityIntegrationKindMcp, DefinitionID: value.DefinitionID,
		ConnectionID: value.ID, Name: name, ConnectionLabel: label, SourceRevision: value.DefinitionRevision,
		ConnectionRevision: value.ConnectionRevision, PolicyRevision: value.PolicyRevision, Status: status,
		HealthStatus: value.HealthStatus, AuthStatus: value.AuthStatus, DataSharingPolicy: sharing,
		UnsafeActionPolicy: unsafe, ToolCount: value.ToolCount, AvailableToolCount: value.AvailableToolCount,
		PendingToolCount: value.PendingToolCount, DefaultedToolCount: value.DefaultedToolCount,
		DisabledToolCount: value.DisabledToolCount, SourceDetails: map[string]any{"transportKind": value.TransportKind}}
}

func mcpToolModel(server store.MCPServer, value store.MCPTool) *model.CapabilityManagedTool {
	description := (*string)(nil)
	if value.Description != "" {
		copy := value.Description
		description = &copy
	}
	result := &model.CapabilityManagedTool{Kind: model.CapabilityIntegrationKindMcp, ConnectionID: server.ID,
		ToolID: value.ID, Name: value.Name, Description: description, Enabled: value.Status != "disabled",
		ReadOnly: hintModel(value.ReadOnly), Idempotent: hintModel(value.Idempotent), Destructive: hintModel(value.Destructive),
		OpenWorld: hintModel(value.OpenWorld), Status: value.Status, PolicyRevision: value.PolicyRevision,
		SourceRevision: value.SourceRevision, SourceDetails: map[string]any{}}
	if server.DataSharingPolicy != "" && server.UnsafeActionPolicy != "" && value.ReadOnly.Value != nil &&
		value.Idempotent.Value != nil && value.Destructive.Value != nil && value.OpenWorld.Value != nil {
		decision := "EXECUTE_IMMEDIATELY"
		risky := (!*value.ReadOnly.Value && (*value.Destructive.Value || *value.OpenWorld.Value)) || server.DataSharingPolicy == "review_every_call"
		if risky && server.UnsafeActionPolicy == "always_ask" {
			decision = "HUMAN_REVIEW"
		}
		if risky && server.UnsafeActionPolicy == "reviewer_may_approve" {
			decision = "LLM_REVIEW"
		}
		result.DecisionPreview = &decision
	}
	return result
}

func hintModel(value store.MCPHint) *model.CapabilityManagedToolHint {
	var source *string
	if value.Source != "" {
		copy := value.Source
		source = &copy
	}
	return &model.CapabilityManagedToolHint{Value: value.Value, Source: source}
}

func (r *Resolver) mcpIntegrations(ctx context.Context) ([]*model.CapabilityIntegration, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	servers, err := service.Servers(ctx)
	if err != nil {
		return nil, err
	}
	byDefinition := make(map[string]*model.CapabilityIntegration)
	order := make([]string, 0)
	for _, server := range servers {
		integration := byDefinition[server.DefinitionID]
		if integration == nil {
			integration = &model.CapabilityIntegration{Kind: model.CapabilityIntegrationKindMcp,
				DefinitionID: server.DefinitionID, Name: server.DisplayName, SourceRevision: server.DefinitionRevision,
				Reviewed: true, SourceSummary: server.TransportKind}
			byDefinition[server.DefinitionID] = integration
			order = append(order, server.DefinitionID)
		}
		integration.Connections = append(integration.Connections, mcpConnectionModel(server))
	}
	result := make([]*model.CapabilityIntegration, 0, len(order))
	for _, id := range order {
		result = append(result, byDefinition[id])
	}
	return result, nil
}

func (r *Resolver) mcpConnection(ctx context.Context, ref model.CapabilityConnectionRefInput) (*model.CapabilityConnection, error) {
	if ref.Kind != model.CapabilityIntegrationKindMcp {
		return nil, errors.New("capability integration is unavailable")
	}
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	server, err := service.Server(ctx, ref.ConnectionID)
	if err != nil {
		return nil, err
	}
	return mcpConnectionModel(server), nil
}

func (r *Resolver) mcpTools(ctx context.Context, ref model.CapabilityConnectionRefInput) ([]*model.CapabilityManagedTool, error) {
	if ref.Kind != model.CapabilityIntegrationKindMcp {
		return nil, errors.New("capability integration is unavailable")
	}
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	server, err := service.Server(ctx, ref.ConnectionID)
	if err != nil {
		return nil, err
	}
	tools, err := service.Tools(ctx, ref.ConnectionID)
	if err != nil {
		return nil, err
	}
	result := make([]*model.CapabilityManagedTool, len(tools))
	for index, tool := range tools {
		result[index] = mcpToolModel(server, tool)
	}
	return result, nil
}

func (r *Resolver) saveMCPConnectionPolicy(ctx context.Context, input model.SaveCapabilityConnectionPolicyInput) (*model.CapabilityConnection, error) {
	if input.Kind != model.CapabilityIntegrationKindMcp {
		return nil, errors.New("capability integration is unavailable")
	}
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	server, err := service.SaveConnectionPolicy(ctx, input.ConnectionID, input.ExpectedConnectionRevision,
		input.ExpectedPolicyRevision, input.DataSharingPolicy, input.UnsafeActionPolicy)
	if err != nil {
		return nil, err
	}
	_ = r.Store.RecordCapabilityReady(ctx, "mcp", server.DisplayName, server.ID,
		server.ConnectionRevision, server.AvailableToolCount, time.Now())
	return mcpConnectionModel(server), nil
}

func (r *Resolver) saveMCPConnectionLabel(ctx context.Context, input model.SaveCapabilityConnectionLabelInput) (*model.CapabilityConnection, error) {
	if input.Kind != model.CapabilityIntegrationKindMcp {
		return nil, errors.New("capability integration is unavailable")
	}
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	server, err := service.SaveConnectionLabel(ctx, input.ConnectionID, input.ExpectedConnectionRevision,
		input.ExpectedConnectionLabel, input.ConnectionLabel)
	if err != nil {
		return nil, err
	}
	return mcpConnectionModel(server), nil
}

func (r *Resolver) changeMCPTool(ctx context.Context, input any) (*model.CapabilityManagedTool, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	var kind model.CapabilityIntegrationKind
	var connection, revision, toolID, source string
	var expected int
	var tool store.MCPTool
	switch value := input.(type) {
	case model.SaveCapabilityToolOverrideInput:
		kind, connection, revision, toolID, source, expected = value.Kind, value.ConnectionID, value.ExpectedConnectionRevision, value.ToolID, value.SourceRevision, value.ExpectedPolicyRevision
		tool, err = service.SaveToolBehavior(ctx, connection, revision, toolID, source, expected, [4]bool{value.ReadOnly, value.Idempotent, value.Destructive, value.OpenWorld})
	case model.SetCapabilityToolEnabledInput:
		kind, connection, revision, toolID, source, expected = value.Kind, value.ConnectionID, value.ExpectedConnectionRevision, value.ToolID, value.SourceRevision, value.ExpectedPolicyRevision
		tool, err = service.SetToolEnabled(ctx, connection, revision, toolID, source, expected, value.Enabled)
	case model.ResetCapabilityToolPolicyInput:
		kind, connection, revision, toolID, source, expected = value.Kind, value.ConnectionID, value.ExpectedConnectionRevision, value.ToolID, value.SourceRevision, value.ExpectedPolicyRevision
		tool, err = service.ResetToolPolicy(ctx, connection, revision, toolID, source, expected)
	}
	if kind != model.CapabilityIntegrationKindMcp {
		return nil, errors.New("capability integration is unavailable")
	}
	if err != nil {
		return nil, err
	}
	server, err := service.Server(ctx, connection)
	if err != nil {
		return nil, err
	}
	return mcpToolModel(server, tool), nil
}

func (r *Resolver) mcpServers(ctx context.Context) ([]*model.McpServer, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	values, err := service.Servers(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]*model.McpServer, len(values))
	for index, value := range values {
		result[index] = mcpServerModel(value)
	}
	return result, nil
}

func (r *Resolver) mcpAttempt(ctx context.Context, id string) (*model.McpOAuthSetupAttempt, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	value, err := service.Attempt(ctx, id, localHumanID)
	if err != nil {
		return nil, err
	}
	if value.Status == "completed" {
		handled := false
		if r.TaskExecution != nil {
			handled, err = r.TaskExecution.ResumeMCPAuthentication(ctx, id)
		}
		if err == nil && !handled && r.Chat != nil {
			err = r.Chat.ResumeMCPAuthentication(ctx, id)
		}
		if err != nil {
			return nil, err
		}
	}
	return mcpAttemptModel(value), nil
}

func (r *Resolver) startMCPCallAuthentication(ctx context.Context, input model.StartMcpAuthenticationInput) (*model.McpOAuthSetupAttempt, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return nil, err
	}
	request, err := r.Store.MCPAuthRequest(ctx, input.RequestID, input.ExpectedRevision)
	if err != nil || request.OwnerHumanID != localHumanID {
		return nil, errors.New("MCP authentication request is unavailable")
	}
	attempt, err := service.StartOAuthReauthentication(ctx, localHumanID, request.ServerID, input.RedirectURI)
	if err != nil {
		return nil, err
	}
	if _, err := r.Store.BeginMCPAuthentication(ctx, request.ID, request.Revision, localHumanID, attempt.ID, time.Now()); err != nil {
		service.CancelOAuth(attempt.ID)
		return nil, err
	}
	return mcpAttemptModel(attempt), nil
}

func (r *Resolver) skipMCPCallAuthentication(ctx context.Context, input model.SkipMcpAuthenticationInput) (*model.McpAuthenticationIntervention, error) {
	if _, err := r.requireMCP(ctx); err != nil {
		return nil, err
	}
	request, err := r.Store.MCPAuthRequest(ctx, input.RequestID, input.ExpectedRevision)
	if err != nil {
		return nil, err
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
		request, err = r.Chat.SkipMCPAuthentication(ctx, input.RequestID, input.ExpectedRevision)
	}
	if err != nil {
		return nil, err
	}
	return mcpAuthenticationModel(request), nil
}

type storedMCPSetup struct {
	Status, ServiceURL, DisplayName, Description, EndpointURL string
	Discovered                                                int
}

func setupFromItem(item store.ConversationItem) (storedMCPSetup, bool) {
	metadata, _ := item.Payload["metadata"].(map[string]any)
	action, _ := metadata["action"].(map[string]any)
	payload, _ := action["payload"].(map[string]any)
	if action["name"] != noemamcp.ConnectServiceToolName || action["success"] != true || payload["intervention_resolution"] != nil {
		return storedMCPSetup{}, false
	}
	value := storedMCPSetup{Status: textField(payload, "status", ""), ServiceURL: textField(payload, "service_url", ""),
		DisplayName: textField(payload, "display_name", ""), Description: textField(payload, "description", ""),
		EndpointURL: textField(payload, "endpoint_url", "")}
	setup, _ := payload["setup_result"].(map[string]any)
	value.Discovered = int(numberValue(setup["discovered_tool_count"]))
	valid := value.Status == "needs_auth" || value.Status == "authentication_available" || value.Status == "ready_for_policy"
	return value, valid && value.DisplayName != "" && value.ServiceURL != "" && value.EndpointURL != ""
}

func numberValue(value any) float64 { result, _ := value.(float64); return result }

func serverMatchesSetup(server store.MCPServer, setup storedMCPSetup) bool {
	var config struct {
		URL string `json:"url"`
	}
	return server.TransportKind == "streamable_http" && json.Unmarshal(server.SafeConfig, &config) == nil && config.URL == setup.EndpointURL
}

func (r *Resolver) pendingMCPSetups(ctx context.Context, conversationID *string, first *int) ([]model.HumanIntervention, error) {
	if conversationID == nil {
		return nil, nil
	}
	limit := 50
	if first != nil {
		limit = *first
	}
	items, err := r.Store.PendingMCPSetupItems(ctx, *conversationID, limit)
	if err != nil {
		return nil, err
	}
	servers, err := r.Store.MCPServers(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]model.HumanIntervention, 0, len(items))
	for _, item := range items {
		setup, ok := setupFromItem(item)
		if !ok {
			continue
		}
		var match *store.MCPServer
		for index := range servers {
			if serverMatchesSetup(servers[index], setup) {
				match = &servers[index]
				break
			}
		}
		if match != nil && match.DataSharingPolicy != "" && match.UnsafeActionPolicy != "" {
			continue
		}
		if setup.Status == "ready_for_policy" && match == nil {
			continue
		}
		value := &model.McpSetupIntervention{ItemID: item.ID, SetupStatus: setup.Status, DisplayName: setup.DisplayName,
			ServiceURL: setup.ServiceURL, EndpointURL: setup.EndpointURL, OauthSupported: setup.Status != "ready_for_policy",
			DiscoveredToolCount: setup.Discovered}
		if setup.Description != "" {
			value.Description = &setup.Description
		}
		if match != nil {
			value.SetupStatus, value.McpServerID, value.ConnectionRevision = "ready_for_policy", &match.ID, &match.ConnectionRevision
			value.PolicyRevision, value.ToolCount = &match.PolicyRevision, &match.ToolCount
		}
		result = append(result, value)
	}
	return result, nil
}

func (r *Resolver) resolveMCPSetupIntervention(ctx context.Context, input model.ResolveMcpSetupInterventionInput) (bool, error) {
	if _, err := r.requireMCP(ctx); err != nil {
		return false, err
	}
	item, err := r.Store.VisibleConversationItem(ctx, input.ItemID)
	if err != nil || item == nil {
		return false, errors.New("MCP setup intervention is unavailable")
	}
	metadata, _ := item.Payload["metadata"].(map[string]any)
	action, _ := metadata["action"].(map[string]any)
	payload, _ := action["payload"].(map[string]any)
	resolution, _ := payload["intervention_resolution"].(map[string]any)
	if textField(resolution, "mcp_server_id", "") == input.McpServerID {
		return true, nil
	}
	setup, ok := setupFromItem(*item)
	if !ok {
		return false, errors.New("MCP setup intervention is unavailable")
	}
	server, err := r.Store.MCPServer(ctx, input.McpServerID)
	if err != nil || !serverMatchesSetup(server, setup) || server.DataSharingPolicy == "" || server.UnsafeActionPolicy == "" {
		return false, errors.New("MCP setup is not configured")
	}
	changed, err := r.Store.ResolveMCPSetupItem(ctx, item.ConversationID, item.ID, server.ID)
	if err == nil && changed && r.Chat != nil {
		r.Chat.NotifyHumanInterventionsChanged(item.ConversationID)
	}
	return changed, err
}

func mcpAuthenticationModel(value store.MCPAuthRequest) *model.McpAuthenticationIntervention {
	states := map[string]model.McpAuthenticationRequestState{"awaiting_user": model.McpAuthenticationRequestStateAwaitingUser,
		"authorizing": model.McpAuthenticationRequestStateAuthorizing, "resuming": model.McpAuthenticationRequestStateResuming,
		"completed": model.McpAuthenticationRequestStateCompleted, "cancelled": model.McpAuthenticationRequestStateCancelled,
		"superseded": model.McpAuthenticationRequestStateSuperseded}
	result := &model.McpAuthenticationIntervention{RequestID: value.ID, Revision: value.Revision,
		ServerDisplayName: value.ServerID, CapabilityName: value.CapabilityName, State: states[value.State]}
	if value.Failure != "" {
		result.FailureCode = &value.Failure
	}
	if value.TaskID != "" {
		result.TaskID = &value.TaskID
	}
	return result
}

func (r *Resolver) pendingMCPAuthentications(ctx context.Context, conversationID, taskID *string, first *int) ([]model.HumanIntervention, error) {
	limit := 50
	if first != nil {
		limit = *first
	}
	if limit < 1 || limit > 100 {
		return nil, errors.New("pendingHumanInterventions first must be within 1..100")
	}
	values, err := r.Store.PendingMCPAuthRequests(ctx, localHumanID, conversationID, taskID, limit)
	if err != nil {
		return nil, err
	}
	result := make([]model.HumanIntervention, len(values))
	for index, value := range values {
		server, loadErr := r.Store.MCPServer(ctx, value.ServerID)
		if loadErr == nil {
			value.ServerID = server.DisplayName
		}
		result[index] = mcpAuthenticationModel(value)
	}
	return result, nil
}

func (r *Resolver) deleteMCP(ctx context.Context, id string) (bool, error) {
	service, err := r.requireMCP(ctx)
	if err != nil {
		return false, err
	}
	return service.Delete(ctx, id)
}

var _ = time.Now
