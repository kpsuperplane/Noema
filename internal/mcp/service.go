package mcp

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/goccy/go-yaml"
	"github.com/google/jsonschema-go/jsonschema"
	"github.com/kpsuperplane/noema/internal/diagnostics"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	discoveryTimeout = 30 * time.Second
	callTimeout      = 60 * time.Second
)

var (
	// ErrUnknownOperation identifies a name that is absent from the current catalog.
	ErrUnknownOperation = errors.New("MCP tool is unavailable")
	// ErrUnknownInvoker identifies a dispatch request for an unregistered source.
	ErrUnknownInvoker = errors.New("MCP invoker is unavailable")
	// ErrAuthorityChanged identifies a stale binding or operation authority.
	ErrAuthorityChanged = errors.New("MCP call authority changed")
	// ErrInvalidArguments identifies arguments rejected by the source schema.
	ErrInvalidArguments = errors.New("MCP arguments do not match the source schema")
	// ErrDenied identifies an unreviewed dispatch of a reviewed binding.
	ErrDenied = errors.New("MCP capability dispatch denied")
)

// ReviewedAuthorization binds one reviewed dispatch to its durable proposal.
type ReviewedAuthorization struct {
	ActionID        string
	Revision        int
	ArgumentsSHA256 string
}

// SetupInput contains one GraphQL MCP setup request.
type SetupInput struct {
	DisplayName, TransportKind                        string
	Command                                           string
	Args                                              []string
	Cwd, URL                                          string
	Environment, Headers                              map[string]string
	Secrets                                           SecretMaterial
	AuthPreference                                    string
	DefinitionID, DefinitionRevision, ConnectionLabel string
}

// SetupResult is one guided setup projection.
type SetupResult struct {
	Server         *store.MCPServer
	Status         string
	Discovered     int
	Error          string
	OAuthSupported bool
	Scopes         []string
}

// Binding is one immutable model-visible MCP tool authority.
type Binding struct {
	Name, Description, ServerID, ToolID, SourceRevision string
	ConnectionRevision                                  string
	ServerPolicyRevision, ToolPolicyRevision            int
	InvokerKey, OperationToken                          string
	InputSchema                                         json.RawMessage
	// InputCheck is the source-owned executable-input rule. The remote schema
	// describes the provider contract, while this closure preserves any
	// source-specific admission rule before the invoker is reached.
	InputCheck        ToolInputCheck `json:"-"`
	Behavior          store.ActionBehavior
	ReviewRoute       store.ActionReviewRoute
	Destination       *store.CapabilityDestination `json:",omitempty"`
	PersistencePolicy BindingPersistencePolicy     `json:"persistence_policy,omitempty"`
}

// ToolInputCheck is the source-owned check for one binding's executable
// arguments. It is intentionally a closure because the source may enforce
// rules that are stricter than its provider-visible JSON schema.
type ToolInputCheck func(any) bool

// Accepts applies the source check. A binding without a source-specific rule
// remains governed by its provider-visible schema.
func (check ToolInputCheck) Accepts(arguments any) bool {
	return check == nil || check(arguments)
}

// GoString preserves the Rust authority names in diagnostic formatting used
// by the capability parity contract.
func (b Binding) GoString() string {
	return fmt.Sprintf("Binding{InvokerKey(%q), OperationToken(%q), Name(%q), ServerID(%q), ToolID(%q), SourceRevision(%q), ConnectionRevision(%q)}", b.InvokerKey, b.OperationToken, b.Name, b.ServerID, b.ToolID, b.SourceRevision, b.ConnectionRevision)
}

// Service owns MCP setup, credentials, catalogs, and calls.
type Service struct {
	database      *store.Store
	errors        *diagnostics.Writer
	secrets       *secretStore
	stdioEnabled  bool
	mu            sync.Mutex
	credentialMu  sync.Mutex
	attempts      map[string]*oauthAttempt
	oauthComplete func(string)
	oauthCallback string
	classify      func(context.Context, store.MCPTool) ([4]bool, error)
	classifyCtx   context.Context
	classifyStop  context.CancelFunc
	classifying   map[string]bool
	classifyWG    sync.WaitGroup
	router        *CapabilityRegistryRouter
}

// SetOAuthCompletionHandler binds completed call authentication attempts.
func (s *Service) SetOAuthCompletionHandler(handler func(string)) {
	s.mu.Lock()
	s.oauthComplete = handler
	s.mu.Unlock()
}

// SetToolClassifier starts bounded classification for missing remote hints.
func (s *Service) SetToolClassifier(classifier func(context.Context, store.MCPTool) ([4]bool, error)) {
	s.mu.Lock()
	s.classify = classifier
	s.mu.Unlock()
	servers, _ := s.database.MCPServers(s.classifyCtx)
	for _, server := range servers {
		s.scheduleClassification(server.ID)
	}
}

// Close stops background classification work.
func (s *Service) Close() {
	s.mu.Lock()
	s.classifyStop()
	s.mu.Unlock()
	s.classifyWG.Wait()
}

// NewService opens one MCP authority and removes abandoned transient OAuth material.
func NewService(paths home.Paths, database *store.Store, stdioEnabled bool, errorLog *diagnostics.Writer, callback ...string) (*Service, error) {
	if database == nil {
		return nil, errors.New("MCP store is unavailable")
	}
	secrets, err := newSecretStore(paths.Root())
	if err != nil {
		return nil, err
	}
	if err := secrets.cleanupAttempts(); err != nil {
		return nil, fmt.Errorf("recover MCP OAuth files: %w", err)
	}
	servers, err := database.MCPServers(context.Background())
	if err != nil {
		return nil, fmt.Errorf("recover MCP connections: %w", err)
	}
	validServers := make(map[string]struct{}, len(servers))
	for _, server := range servers {
		validServers[server.ID] = struct{}{}
		if server.SecretRevision == "" {
			continue
		}
		material, loadErr := secrets.loadConnection(server.ID)
		if loadErr != nil || material.Revision != server.SecretRevision {
			_ = database.MarkMCPUnavailable(context.Background(), server.ID, server.ConnectionRevision, "needs_auth", time.Now())
		}
	}
	if err := secrets.cleanupConnections(validServers); err != nil {
		return nil, fmt.Errorf("recover MCP credentials: %w", err)
	}
	oauthCallback := ""
	if len(callback) != 0 {
		parsed, callbackErr := validateOAuthCallback(callback[0])
		if callbackErr != nil {
			return nil, callbackErr
		}
		oauthCallback = parsed.String()
	}
	classifyCtx, classifyStop := context.WithCancel(context.Background())
	service := &Service{database: database, errors: errorLog, secrets: secrets, stdioEnabled: stdioEnabled, oauthCallback: oauthCallback,
		attempts: make(map[string]*oauthAttempt), classifyCtx: classifyCtx, classifyStop: classifyStop,
		classifying: make(map[string]bool)}
	router, err := NewCapabilityRegistryRouter(CapabilityInvokerRegistration{
		Key: "mcp", Invoker: serviceCapabilityInvoker{service: service},
	})
	if err != nil {
		classifyStop()
		return nil, fmt.Errorf("configure MCP capability router: %w", err)
	}
	service.router = router
	return service, nil
}

// StdioEnabled reads the startup double-opt-in setting. It defaults to false.
func StdioEnabled(paths home.Paths) (bool, error) {
	value := false
	data, err := home.ReadPrivateFile(paths.Config(), 1<<20)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return false, err
	}
	if len(data) != 0 {
		var document map[string]any
		if yaml.Unmarshal(data, &document) != nil {
			return false, errors.New("config.yaml is not valid YAML")
		}
		if raw, exists := document["mcp"]; exists {
			section, ok := raw.(map[string]any)
			if !ok {
				return false, errors.New("config.yaml field mcp must be a mapping")
			}
			if rawEnabled, exists := section["stdio_enabled"]; exists {
				var ok bool
				value, ok = rawEnabled.(bool)
				if !ok {
					return false, errors.New("config.yaml field mcp.stdio_enabled must be true or false")
				}
			}
		}
	}
	environmentValue, exists := os.LookupEnv("NOEMA_MCP__STDIO_ENABLED")
	if !exists {
		return value, nil
	}
	switch strings.ToLower(strings.TrimSpace(environmentValue)) {
	case "true", "1":
		return true, nil
	case "false", "0":
		return false, nil
	}
	return false, errors.New("NOEMA_MCP__STDIO_ENABLED must be true or false")
}

// Create discovers and publishes one new MCP connection.
func (s *Service) Create(ctx context.Context, input SetupInput) (SetupResult, error) {
	definition, err := definitionFromSetup(input)
	if err != nil {
		return SetupResult{}, err
	}
	return s.create(ctx, input, definition, false)
}

// AddConnection discovers a fresh connection against one exact definition.
func (s *Service) AddConnection(ctx context.Context, input SetupInput) (SetupResult, error) {
	definition, err := s.database.MCPDefinition(ctx, input.DefinitionID)
	if err != nil || definition.Revision != input.DefinitionRevision {
		return SetupResult{}, errors.New("MCP definition revision changed")
	}
	input.DisplayName, input.TransportKind = definition.DisplayName, definition.TransportKind
	if err := json.Unmarshal(definition.SafeConfig, safeTarget(&input)); err != nil {
		return SetupResult{}, errors.New("stored MCP definition is invalid")
	}
	return s.create(ctx, input, definition, true)
}

func (s *Service) create(ctx context.Context, input SetupInput, definition store.MCPDefinition, reuse bool) (SetupResult, error) {
	if input.TransportKind == "stdio" && !s.stdioEnabled {
		return SetupResult{}, errors.New("MCP stdio transport is disabled")
	}
	if err := validateSetup(input); err != nil {
		return SetupResult{}, err
	}
	discoveryContext, cancel := context.WithTimeout(ctx, discoveryTimeout)
	defer cancel()
	discovery, err := Discover(discoveryContext, configFromInput(input, definition.SafeConfig))
	if err != nil {
		if errors.Is(err, ErrAuthenticationRequired) && input.TransportKind == "streamable_http" {
			return SetupResult{Status: "needs_auth", OAuthSupported: true,
				Error: "this MCP server requires authentication before Noema can list tools"}, nil
		}
		return SetupResult{Status: "unavailable", Error: "Noema could not connect to this MCP server"}, nil
	}
	if input.TransportKind == "streamable_http" && input.AuthPreference != "USE_ANONYMOUS" &&
		!hasSecretMaterial(input.Secrets) && oauthAvailable(ctx, input.URL) {
		return SetupResult{Status: "authentication_available", Discovered: len(discovery.Tools), OAuthSupported: true}, nil
	}
	serverID, err := newPrefixedID("mcp_server:")
	if err != nil {
		return SetupResult{}, errors.New("MCP identity generation failed")
	}
	revision, err := newPrefixedID("mcp_connection_revision:")
	if err != nil {
		return SetupResult{}, errors.New("MCP identity generation failed")
	}
	secretRevision := ""
	if hasSecretMaterial(input.Secrets) {
		secretRevision, err = randomHex()
		if err != nil {
			return SetupResult{}, err
		}
		input.Secrets.Revision = secretRevision
		if err := s.secrets.writeConnection(serverID, input.Secrets); err != nil {
			return SetupResult{}, errors.New("MCP credentials could not be stored")
		}
	}
	tools, err := storedTools(serverID, discovery.Tools)
	if err != nil {
		_ = s.secrets.removeConnection(serverID)
		return SetupResult{}, err
	}
	server, err := s.database.CommitMCPConnection(ctx, store.NewMCPConnection{
		Definition: definition, ReuseDefinition: reuse, ServerID: serverID,
		ConnectionLabel: input.ConnectionLabel, ConnectionRevision: revision,
		SecretRevision: secretRevision, ServiceDescription: discovery.ServiceDescription,
		AuthStatus: authStatus(input.Secrets), Tools: tools,
	}, time.Now())
	if err != nil {
		_ = s.secrets.removeConnection(serverID)
		return SetupResult{}, err
	}
	s.scheduleClassification(server.ID)
	return SetupResult{Server: &server, Status: "ready_for_policy", Discovered: len(tools)}, nil
}

// Continue merges protected credentials and rechecks one existing catalog.
func (s *Service) Continue(ctx context.Context, serverID string, replacement SecretMaterial) (SetupResult, error) {
	s.credentialMu.Lock()
	defer s.credentialMu.Unlock()
	server, err := s.database.MCPServer(ctx, serverID)
	if err != nil {
		return SetupResult{}, err
	}
	current := SecretMaterial{}
	if server.SecretRevision != "" {
		current, err = s.secrets.loadConnection(serverID)
		if err != nil {
			return SetupResult{}, errors.New("MCP credentials are unavailable")
		}
	}
	merged := mergeSecrets(current, replacement)
	revision, err := randomHex()
	if err != nil {
		return SetupResult{}, err
	}
	merged.Revision = revision
	backup := current
	if err := s.secrets.writeConnection(serverID, merged); err != nil {
		return SetupResult{}, errors.New("MCP credentials could not be stored")
	}
	discoveryContext, cancel := context.WithTimeout(ctx, discoveryTimeout)
	defer cancel()
	discovery, err := Discover(discoveryContext, Config{TransportKind: server.TransportKind, SafeConfig: server.SafeConfig, Secrets: merged})
	if err != nil {
		_ = restoreConnectionSecrets(s.secrets, serverID, backup)
		_ = s.database.MarkMCPUnavailable(ctx, serverID, server.ConnectionRevision, "needs_auth", time.Now())
		return SetupResult{Server: &server, Status: "needs_auth", Error: "this MCP server requires authentication before Noema can list tools", OAuthSupported: server.TransportKind == "streamable_http"}, nil
	}
	tools, err := storedTools(serverID, discovery.Tools)
	if err != nil {
		_ = restoreConnectionSecrets(s.secrets, serverID, backup)
		return SetupResult{}, err
	}
	server, err = s.database.ReconcileMCPConnection(ctx, serverID, server.ConnectionRevision, revision, authStatus(merged), tools, time.Now())
	if err != nil {
		_ = restoreConnectionSecrets(s.secrets, serverID, backup)
		return SetupResult{}, err
	}
	s.scheduleClassification(server.ID)
	return SetupResult{Server: &server, Status: "ready_for_policy", Discovered: len(tools)}, nil
}

func (s *Service) scheduleClassification(serverID string) {
	s.mu.Lock()
	if s.classify == nil || s.classifying[serverID] || s.classifyCtx.Err() != nil {
		s.mu.Unlock()
		return
	}
	s.classifying[serverID] = true
	s.classifyWG.Add(1)
	s.mu.Unlock()
	go func() {
		defer func() {
			s.classifyWG.Done()
			s.mu.Lock()
			delete(s.classifying, serverID)
			s.mu.Unlock()
		}()
		server, err := s.database.MCPServer(s.classifyCtx, serverID)
		if err != nil {
			return
		}
		tools, err := s.database.MCPTools(s.classifyCtx, serverID)
		if err != nil {
			return
		}
		for _, tool := range tools {
			if tool.Status != "defaulted" {
				continue
			}
			s.mu.Lock()
			classifier := s.classify
			s.mu.Unlock()
			ctx, cancel := context.WithTimeout(s.classifyCtx, discoveryTimeout)
			behavior, classifyErr := classifier(ctx, tool)
			cancel()
			if classifyErr == nil {
				_, _ = s.database.ClassifyMCPTool(s.classifyCtx, server.ID, server.ConnectionRevision,
					tool.ID, tool.SourceRevision, tool.PolicyRevision, behavior, time.Now())
			}
		}
	}()
}

// Servers returns all current safe connection metadata.
func (s *Service) Servers(ctx context.Context) ([]store.MCPServer, error) {
	return s.database.MCPServers(ctx)
}
func (s *Service) Server(ctx context.Context, id string) (store.MCPServer, error) {
	return s.database.MCPServer(ctx, id)
}
func (s *Service) Tools(ctx context.Context, id string) ([]store.MCPTool, error) {
	return s.database.MCPTools(ctx, id)
}

// SaveConnectionPolicy applies one exact human policy.
func (s *Service) SaveConnectionPolicy(ctx context.Context, id, revision string, expected int, sharing, unsafe string) (store.MCPServer, error) {
	return s.database.SaveMCPConnectionPolicy(ctx, id, revision, expected, sharing, unsafe, time.Now())
}

// SaveConnectionLabel applies one exact human label.
func (s *Service) SaveConnectionLabel(ctx context.Context, id, revision string, expected, replacement *string) (store.MCPServer, error) {
	return s.database.SaveMCPConnectionLabel(ctx, id, revision, expected, replacement, time.Now())
}

// SaveToolBehavior stores one complete exact human override.
func (s *Service) SaveToolBehavior(ctx context.Context, serverID, revision, toolID, source string, expected int, behavior [4]bool) (store.MCPTool, error) {
	return s.database.SaveMCPToolPolicy(ctx, serverID, revision, toolID, source, expected, nil, &behavior, time.Now())
}

// ClassifyToolResponse validates and applies one exact model classification
// response at the MCP authority boundary.
func (s *Service) ClassifyToolResponse(ctx context.Context, serverID, connectionRevision, toolID, sourceRevision string, expected int, raw string) (store.MCPTool, error) {
	tools, err := s.database.MCPTools(ctx, serverID)
	if err != nil {
		return store.MCPTool{}, err
	}
	var current store.MCPTool
	for _, tool := range tools {
		if tool.ID == toolID {
			current = tool
			break
		}
	}
	if current.ID == "" {
		return store.MCPTool{}, errors.New("MCP tool was not found")
	}
	completion, err := ParseToolClassificationResponse(raw, current)
	if err != nil {
		return store.MCPTool{}, err
	}
	merged := ApplyToolClassification(current, completion)
	return s.database.ClassifyMCPTool(ctx, serverID, connectionRevision, toolID, sourceRevision, expected,
		[4]bool{*merged.ReadOnly.Value, *merged.Idempotent.Value, *merged.Destructive.Value, *merged.OpenWorld.Value}, time.Now())
}

// SetToolEnabled changes only one exact tool availability state.
func (s *Service) SetToolEnabled(ctx context.Context, serverID, revision, toolID, source string, expected int, enabled bool) (store.MCPTool, error) {
	return s.database.SaveMCPToolPolicy(ctx, serverID, revision, toolID, source, expected, &enabled, nil, time.Now())
}

// ResetToolPolicy restores the current source annotations and safe defaults.
func (s *Service) ResetToolPolicy(ctx context.Context, serverID, revision, toolID, source string, expected int) (store.MCPTool, error) {
	tools, err := s.database.MCPTools(ctx, serverID)
	if err != nil {
		return store.MCPTool{}, err
	}
	for _, tool := range tools {
		if tool.ID != toolID || tool.SourceRevision != source {
			continue
		}
		var annotations struct {
			ReadOnly, Idempotent bool  `json:"-"`
			Destructive          *bool `json:"destructiveHint"`
			OpenWorld            *bool `json:"openWorldHint"`
		}
		var raw map[string]json.RawMessage
		_ = json.Unmarshal(tool.Annotations, &raw)
		var readOnly, idempotent *bool
		if value, ok := raw["readOnlyHint"]; ok {
			var parsed bool
			if json.Unmarshal(value, &parsed) == nil {
				readOnly = &parsed
			}
		}
		if value, ok := raw["idempotentHint"]; ok {
			var parsed bool
			if json.Unmarshal(value, &parsed) == nil {
				idempotent = &parsed
			}
		}
		_ = json.Unmarshal(tool.Annotations, &annotations)
		values := []*bool{readOnly, idempotent, annotations.Destructive, annotations.OpenWorld}
		fallback := [4]bool{false, false, true, true}
		hints := [4]store.MCPHint{}
		status := "ready"
		for index, value := range values {
			if value == nil {
				v := fallback[index]
				value = &v
				hints[index].Source = "safe_default"
				status = "defaulted"
			} else {
				hints[index].Source = "annotation"
			}
			hints[index].Value = value
		}
		return s.database.ResetMCPToolPolicy(ctx, serverID, revision, toolID, source, expected, hints, status, time.Now())
	}
	return store.MCPTool{}, errors.New("MCP tool was not found")
}

// Bindings returns only tools callable by primary Chat now.
func (s *Service) Bindings(ctx context.Context) ([]Binding, error) {
	catalog, err := s.Catalog(ctx)
	if err != nil {
		return nil, err
	}
	return catalog.Bindings, nil
}

// Catalog returns the live model-visible MCP catalog and the availability
// notices for configured connections that cannot publish bindings yet.
func (s *Service) Catalog(ctx context.Context) (BindingCatalogResult, error) {
	servers, err := s.database.MCPServers(ctx)
	if err != nil {
		return BindingCatalogResult{}, err
	}
	builder := NewBindingCatalogBuilder()
	if err := builder.Add(connectServiceBinding()); err != nil {
		return BindingCatalogResult{}, ErrInvalidBindingSource
	}
	result := BindingCatalogResult{}
	for _, server := range servers {
		if !server.Enabled || server.HealthStatus != "healthy" || (server.AuthStatus != "none" && server.AuthStatus != "authenticated") {
			capability := "mcp." + server.ID
			status := BindingUnavailable
			if server.AuthStatus != "none" && server.AuthStatus != "authenticated" {
				status = BindingAuthenticationRequired
			}
			result.AvailabilityNotices = append(result.AvailabilityNotices, BindingAvailabilityNotice{Capability: &capability, Status: status})
			continue
		}
		tools, err := s.database.MCPTools(ctx, server.ID)
		if err != nil {
			return BindingCatalogResult{}, err
		}
		for _, tool := range tools {
			if tool.Status == "disabled" {
				if tool.ReadOnly.Value == nil || tool.Idempotent.Value == nil || tool.Destructive.Value == nil || tool.OpenWorld.Value == nil {
					continue
				}
				destination, destinationErr := store.NewCapabilityDestination("mcp", server.ID, nil, server.ConnectionRevision)
				if destinationErr != nil {
					return BindingCatalogResult{}, ErrInvalidBindingSource
				}
				disabledName := "mcp." + server.ID + "." + tool.Name
				enablement := Binding{
					Name:        "enable." + disabledName,
					Description: fmt.Sprintf("Ask the human to enable the disabled %s tool (%s). Use this only when that tool is required for the current request.", disabledName, strings.TrimSpace(tool.Description)),
					ServerID:    server.ID, ToolID: tool.ID, SourceRevision: tool.SourceRevision,
					ConnectionRevision: server.ConnectionRevision, ServerPolicyRevision: server.PolicyRevision,
					ToolPolicyRevision: tool.PolicyRevision, InvokerKey: "mcp", OperationToken: tool.Name,
					InputSchema: json.RawMessage(`{"type":"object","properties":{},"required":[],"additionalProperties":false}`),
					Behavior:    store.ActionBehavior{RepeatSafe: true}, ReviewRoute: store.ActionHumanReview,
					Destination: &destination, PersistencePolicy: BindingPersistenceRedacted,
				}
				enablement.InputCheck = func(value any) bool {
					object, ok := value.(map[string]any)
					return ok && len(object) == 0
				}
				if err := builder.Add(enablement); err != nil {
					return BindingCatalogResult{}, ErrInvalidBindingSource
				}
				continue
			}
			if tool.ReadOnly.Value == nil || tool.Idempotent.Value == nil || tool.Destructive.Value == nil || tool.OpenWorld.Value == nil {
				continue
			}
			behavior := store.ActionBehavior{ReadOnly: *tool.ReadOnly.Value, RepeatSafe: *tool.Idempotent.Value,
				Destructive: *tool.Destructive.Value, OpenWorld: *tool.OpenWorld.Value}
			route := reviewRoute(server, behavior)
			description := strings.TrimSpace(tool.Description)
			if description == "" {
				description = "MCP tool"
			}
			destination, destinationErr := store.NewCapabilityDestination(
				"mcp", server.ID, nil, server.ConnectionRevision,
			)
			if destinationErr != nil {
				return BindingCatalogResult{}, ErrInvalidBindingSource
			}
			inputCheck, inputCheckErr := compileMCPInputCheck(tool.InputSchema)
			if inputCheckErr != nil {
				return BindingCatalogResult{}, ErrInvalidBindingSource
			}
			binding := Binding{Name: "mcp." + server.ID + "." + tool.Name, Description: description,
				ServerID: server.ID, ToolID: tool.ID, SourceRevision: tool.SourceRevision,
				ConnectionRevision: server.ConnectionRevision, ServerPolicyRevision: server.PolicyRevision,
				ToolPolicyRevision: tool.PolicyRevision, InputSchema: append(json.RawMessage(nil), tool.InputSchema...),
				Behavior: behavior, ReviewRoute: route, InvokerKey: "mcp", OperationToken: tool.Name,
				Destination: &destination, InputCheck: inputCheck,
				PersistencePolicy: BindingPersistenceRedacted}
			if err := builder.Add(binding); err != nil {
				return BindingCatalogResult{}, ErrInvalidBindingSource
			}
		}
	}
	result.Bindings = builder.Build()
	sort.SliceStable(result.Bindings, func(i, j int) bool { return result.Bindings[i].Name < result.Bindings[j].Name })
	return result, nil
}

// Binding returns the current binding with one exact canonical name.
func (s *Service) Binding(ctx context.Context, name string) (Binding, error) {
	bindings, err := s.Bindings(ctx)
	if err != nil {
		return Binding{}, err
	}
	for _, binding := range bindings {
		if binding.Name == name {
			return binding, nil
		}
	}
	return Binding{}, errors.New("MCP tool is unavailable")
}

// ValidateArguments checks strict JSON and the exact source schema.
func ValidateArguments(schemaBytes, arguments json.RawMessage) error {
	var raw any
	if json.Unmarshal(schemaBytes, &raw) != nil {
		return errors.New("MCP input schema is invalid")
	}
	if err := validateMCPInputSchema(raw); err != nil {
		return err
	}
	var schema jsonschema.Schema
	if json.Unmarshal(schemaBytes, &schema) != nil {
		return errors.New("MCP input schema is invalid")
	}
	resolved, err := schema.Resolve(nil)
	if err != nil {
		return errors.New("MCP input schema is unsupported")
	}
	var value map[string]any
	if json.Unmarshal(arguments, &value) != nil {
		return fmt.Errorf("%w: invalid JSON", ErrInvalidArguments)
	}
	if err := resolved.Validate(value); err != nil {
		return ErrInvalidArguments
	}
	return nil
}

func compileMCPInputCheck(schemaBytes json.RawMessage) (ToolInputCheck, error) {
	var raw any
	if json.Unmarshal(schemaBytes, &raw) != nil {
		return nil, errors.New("MCP input schema is invalid")
	}
	if err := validateMCPInputSchema(raw); err != nil {
		return nil, err
	}
	var schema jsonschema.Schema
	if json.Unmarshal(schemaBytes, &schema) != nil {
		return nil, errors.New("MCP input schema is invalid")
	}
	if _, err := schema.Resolve(nil); err != nil {
		return nil, errors.New("MCP input schema is unsupported")
	}
	return func(value any) bool {
		encoded, err := json.Marshal(value)
		return err == nil && ValidateArguments(schemaBytes, encoded) == nil
	}, nil
}

var mcpSchemaKeywords = func() map[string]struct{} {
	values := strings.Fields(`$comment $defs $id $ref $schema $anchor $dynamicAnchor $dynamicRef $recursiveAnchor $recursiveRef $vocabulary additionalItems additionalProperties allOf anyOf default definitions dependencies dependentRequired dependentSchemas deprecated description else enum const examples exclusiveMaximum exclusiveMinimum format if items prefixItems contains maxContains minContains maxItems maxLength maxProperties maximum minItems minLength minProperties minimum multipleOf not oneOf pattern patternProperties properties propertyNames readOnly required then title type uniqueItems unevaluatedItems unevaluatedProperties writeOnly`)
	result := make(map[string]struct{}, len(values))
	for _, value := range values {
		result[value] = struct{}{}
	}
	return result
}()

func validateMCPInputSchema(value any) error {
	object, ok := value.(map[string]any)
	if !ok {
		return errors.New("MCP input schema is invalid")
	}
	if declared, exists := object["$schema"]; exists {
		name, ok := declared.(string)
		supported := map[string]struct{}{
			"http://json-schema.org/draft-04/schema": {}, "https://json-schema.org/draft-04/schema": {},
			"http://json-schema.org/draft-06/schema": {}, "https://json-schema.org/draft-06/schema": {},
			"http://json-schema.org/draft-07/schema": {}, "https://json-schema.org/draft-07/schema": {},
			"https://json-schema.org/draft/2019-09/schema": {}, "https://json-schema.org/draft/2020-12/schema": {},
		}
		if !ok {
			return errors.New("MCP input schema is unsupported")
		}
		if _, ok := supported[strings.TrimSuffix(name, "#")]; !ok {
			return errors.New("MCP input schema is unsupported")
		}
	}
	if err := validateMCPInputSchemaObject(object); err != nil {
		return errors.New("MCP input schema is unsupported")
	}
	return nil
}

func validateMCPInputSchemaObject(object map[string]any) error {
	for keyword, value := range object {
		if _, ok := mcpSchemaKeywords[keyword]; !ok && !strings.HasPrefix(keyword, "x-") {
			return errors.New("unsupported schema keyword")
		}
		switch keyword {
		case "properties", "patternProperties", "$defs", "definitions", "dependentSchemas":
			children, ok := value.(map[string]any)
			if !ok {
				return errors.New("schema child map is invalid")
			}
			for _, child := range children {
				if childObject, ok := child.(map[string]any); !ok || validateMCPInputSchemaObject(childObject) != nil {
					return errors.New("schema child is invalid")
				}
			}
		case "additionalProperties", "additionalItems", "contains", "items", "not", "if", "then", "else", "propertyNames", "unevaluatedItems", "unevaluatedProperties":
			switch children := value.(type) {
			case map[string]any:
				if validateMCPInputSchemaObject(children) != nil {
					return errors.New("schema child is invalid")
				}
			case []any:
				for _, child := range children {
					childObject, ok := child.(map[string]any)
					if !ok || validateMCPInputSchemaObject(childObject) != nil {
						return errors.New("schema child is invalid")
					}
				}
			case nil, bool:
			default:
				return errors.New("schema child is invalid")
			}
		case "allOf", "anyOf", "oneOf", "prefixItems":
			children, ok := value.([]any)
			if !ok {
				return errors.New("schema child list is invalid")
			}
			for _, child := range children {
				childObject, ok := child.(map[string]any)
				if !ok || validateMCPInputSchemaObject(childObject) != nil {
					return errors.New("schema child is invalid")
				}
			}
		case "dependencies":
			children, ok := value.(map[string]any)
			if !ok {
				return errors.New("schema dependency map is invalid")
			}
			for _, child := range children {
				if childObject, ok := child.(map[string]any); ok && validateMCPInputSchemaObject(childObject) != nil {
					return errors.New("schema child is invalid")
				}
			}
		}
	}
	return nil
}

// Call executes one binding after every durable and remote revision check.
func (s *Service) Call(ctx context.Context, authority Binding, arguments json.RawMessage) (json.RawMessage, bool, error) {
	dispatch, failure := s.dispatch(ctx, authority, arguments, nil)
	if failure.Error != nil {
		return nil, false, mapCapabilityCallError(failure.Error)
	}
	payload, err := json.Marshal(dispatch.Output.Payload)
	if err != nil {
		return nil, false, errors.New("MCP tool result is invalid")
	}
	return payload, dispatch.Output.Success, nil
}

// CallReviewed executes one reviewed binding only when its exact durable
// authorization and argument digest are supplied.
func (s *Service) CallReviewed(ctx context.Context, authority Binding, arguments json.RawMessage, authorization ReviewedAuthorization) (json.RawMessage, bool, error) {
	if authority.ReviewRoute == "" || authorization.ActionID == "" || authorization.Revision != 1 || authorization.ArgumentsSHA256 == "" {
		return nil, false, ErrDenied
	}
	dispatch, failure := s.dispatch(ctx, authority, arguments, &authorization)
	if failure.Error != nil {
		return nil, false, mapCapabilityCallError(failure.Error)
	}
	payload, err := json.Marshal(dispatch.Output.Payload)
	if err != nil {
		return nil, false, errors.New("MCP tool result is invalid")
	}
	return payload, dispatch.Output.Success, nil
}

// dispatch is the production MCP capability boundary. It captures the live
// catalog, fences the caller's durable authority, then routes the call through
// the same registry used by capability families outside MCP.
func (s *Service) dispatch(ctx context.Context, authority Binding, arguments json.RawMessage, reviewed *ReviewedAuthorization) (CapabilityDispatch, CapabilityDispatchFailure) {
	bindings, err := s.Bindings(ctx)
	if err != nil {
		return CapabilityDispatch{}, CapabilityDispatchFailure{Error: err}
	}
	builder := NewBindingCatalogBuilder()
	for _, binding := range bindings {
		if err := builder.Add(binding); err != nil {
			return CapabilityDispatch{}, CapabilityDispatchFailure{Error: ErrInvalidBindingSource}
		}
	}
	snapshot := builder.BuildSnapshot()
	current, exists := snapshot.Resolve(authority.Name)
	if !exists {
		if s.router == nil {
			return CapabilityDispatch{}, CapabilityDispatchFailure{Error: ErrCapabilityUnknownInvoker}
		}
		return s.router.Dispatch(ctx, snapshot, authority.Name, json.RawMessage(append([]byte(nil), arguments...)))
	}
	target := current
	if authority.InvokerKey != "" {
		target.InvokerKey = authority.InvokerKey
	} else {
		authority.InvokerKey = current.InvokerKey
	}
	if authority.OperationToken != "" {
		target.OperationToken = authority.OperationToken
	} else {
		authority.OperationToken = current.OperationToken
	}
	if !sameBindingAuthority(target, authority) {
		return CapabilityDispatch{}, capabilityFailure(ErrAuthorityChanged, current, json.RawMessage(append([]byte(nil), arguments...)))
	}
	if s.router == nil {
		return CapabilityDispatch{}, capabilityFailure(ErrCapabilityUnknownInvoker, current, json.RawMessage(append([]byte(nil), arguments...)))
	}
	if target.InvokerKey != current.InvokerKey || target.OperationToken != current.OperationToken {
		entries := snapshot.Bindings()
		for index := range entries {
			if entries[index].Name == target.Name {
				entries[index] = target
				break
			}
		}
		builder = NewBindingCatalogBuilder()
		for _, binding := range entries {
			if err := builder.Add(binding); err != nil {
				return CapabilityDispatch{}, CapabilityDispatchFailure{Error: ErrInvalidBindingSource}
			}
		}
		snapshot = builder.BuildSnapshot()
	}
	value := json.RawMessage(append([]byte(nil), arguments...))
	if reviewed == nil {
		return s.router.Dispatch(ctx, snapshot, authority.Name, value)
	}
	return s.router.DispatchReviewed(ctx, snapshot, authority.Name, value, *reviewed)
}

func mapCapabilityCallError(err error) error {
	switch {
	case errors.Is(err, ErrCapabilityUnknownInvoker):
		return ErrUnknownInvoker
	case errors.Is(err, ErrCapabilityUnknownOperation):
		return ErrUnknownOperation
	case errors.Is(err, ErrCapabilityInvalidArguments):
		return ErrInvalidArguments
	case errors.Is(err, ErrCapabilityDenied):
		return ErrDenied
	case errors.Is(err, ErrCapabilityAuthenticationRequired):
		return ErrAuthenticationRequired
	default:
		return err
	}
}

type serviceCapabilityInvoker struct {
	service *Service
}

func (invoker serviceCapabilityInvoker) Invoke(ctx context.Context, invocation CapabilityInvocation) (CapabilityOutput, error) {
	current, err := invoker.service.Binding(ctx, invocation.Operation)
	if err != nil {
		return CapabilityOutput{}, ErrCapabilityUnknownOperation
	}
	if current.OperationToken != invocation.OperationToken {
		return CapabilityOutput{}, ErrCapabilityUnknownOperation
	}
	arguments, err := json.Marshal(invocation.Arguments)
	if err != nil {
		return CapabilityOutput{}, ErrCapabilityInvalidArguments
	}
	if current.Name == ConnectServiceToolName && current.OperationToken == connectServiceOperationToken {
		var input struct {
			ServiceURL string `json:"service_url"`
		}
		if err := json.Unmarshal(arguments, &input); err != nil || input.ServiceURL == "" {
			return CapabilityOutput{}, ErrCapabilityInvalidArguments
		}
		return CapabilityOutput{Success: true, Payload: invoker.service.ConnectService(ctx, input.ServiceURL).capabilityPayload()}, nil
	}
	if strings.HasPrefix(current.Name, "enable.mcp.") {
		var input map[string]any
		if err := json.Unmarshal(arguments, &input); err != nil || len(input) != 0 {
			return CapabilityOutput{}, ErrCapabilityInvalidArguments
		}
		if _, err := invoker.service.SetToolEnabled(ctx, current.ServerID, current.ConnectionRevision, current.ToolID, current.SourceRevision, current.ToolPolicyRevision, true); err != nil {
			return CapabilityOutput{}, ErrCapabilityUnknownOperation
		}
		return CapabilityOutput{Success: true, Payload: map[string]any{"enabled_capability": strings.TrimPrefix(current.Name, "enable.")}}, nil
	}
	result, success, err := invoker.service.executeBinding(ctx, current, arguments)
	if err != nil {
		switch {
		case errors.Is(err, ErrAuthenticationRequired):
			return CapabilityOutput{}, ErrCapabilityAuthenticationRequired
		case strings.Contains(err.Error(), "source revision changed"):
			return CapabilityOutput{}, ErrCapabilityUnknownOperation
		default:
			return CapabilityOutput{}, ErrCapabilityUnavailable
		}
	}
	var payload any
	if err := json.Unmarshal(result, &payload); err != nil {
		return CapabilityOutput{}, ErrCapabilityFailed
	}
	return CapabilityOutput{Success: success, Payload: payload}, nil
}

func (s *Service) executeBinding(ctx context.Context, current Binding, arguments json.RawMessage) (json.RawMessage, bool, error) {
	server, err := s.database.MCPServer(ctx, current.ServerID)
	if err != nil {
		return nil, false, err
	}
	secrets := SecretMaterial{}
	if server.SecretRevision != "" {
		secrets, err = s.secrets.loadConnection(server.ID)
		if err != nil || secrets.Revision != server.SecretRevision {
			return nil, false, errors.New("MCP credentials are unavailable")
		}
		secrets, err = s.refreshOAuth(ctx, server, secrets)
		if err != nil {
			_ = s.database.MarkMCPUnavailable(ctx, server.ID, server.ConnectionRevision, "needs_auth", time.Now())
			return nil, false, ErrAuthenticationRequired
		}
	}
	var object map[string]any
	if json.Unmarshal(arguments, &object) != nil {
		return nil, false, errors.New("MCP arguments are invalid")
	}
	callContext, cancel := context.WithTimeout(ctx, callTimeout)
	defer cancel()
	operation := current.OperationToken
	if operation == "" {
		operation = strings.TrimPrefix(current.Name, "mcp."+server.ID+".")
	}
	result, success, err := CallExact(callContext, Config{TransportKind: server.TransportKind, SafeConfig: server.SafeConfig, Secrets: secrets},
		operation, current.SourceRevision, object)
	if err != nil {
		if s.errors != nil {
			_ = s.errors.Write("mcp.call_failed", diagnostics.Text("server_id", server.ID),
				diagnostics.Text("tool_name", current.Name), diagnostics.Text("detail", err.Error()))
		}
		authStatus := server.AuthStatus
		if errors.Is(err, ErrAuthenticationRequired) {
			authStatus = "needs_auth"
		}
		_ = s.database.MarkMCPUnavailable(ctx, server.ID, server.ConnectionRevision, authStatus, time.Now())
		return nil, false, err
	}
	return result, success, nil
}

// Delete removes durable references before abandoned protected credentials.
func (s *Service) Delete(ctx context.Context, id string) (bool, error) {
	s.credentialMu.Lock()
	defer s.credentialMu.Unlock()
	server, err := s.database.MCPServer(ctx, id)
	if err != nil {
		return false, nil
	}
	if err := s.database.FenceMCPServer(ctx, id, server.ConnectionRevision, time.Now()); err != nil {
		return false, err
	}
	deleted, err := s.database.DeleteMCPServer(ctx, id)
	if err != nil || !deleted {
		return deleted, err
	}
	_ = s.secrets.removeConnection(id)
	return true, nil
}

func definitionFromSetup(input SetupInput) (store.MCPDefinition, error) {
	id, err := newPrefixedID("mcp_definition:")
	if err != nil {
		return store.MCPDefinition{}, err
	}
	revision, err := newPrefixedID("mcp_definition_revision:")
	if err != nil {
		return store.MCPDefinition{}, err
	}
	var safe any
	if input.TransportKind == "stdio" {
		safe = map[string]any{"command": input.Command, "args": input.Args, "cwd": emptyNil(input.Cwd), "env": input.Environment}
	} else {
		safe = map[string]any{"url": input.URL, "headers": input.Headers}
	}
	encoded, err := json.Marshal(safe)
	if err != nil || len(encoded) > 65536 {
		return store.MCPDefinition{}, errors.New("MCP safe configuration is invalid")
	}
	return store.MCPDefinition{ID: id, Revision: revision, DisplayName: strings.TrimSpace(input.DisplayName), TransportKind: input.TransportKind, SafeConfig: encoded}, nil
}

func safeTarget(input *SetupInput) any {
	if input.TransportKind == "stdio" {
		return &struct {
			Command *string            `json:"command"`
			Args    *[]string          `json:"args"`
			Cwd     *string            `json:"cwd"`
			Env     *map[string]string `json:"env"`
		}{&input.Command, &input.Args, &input.Cwd, &input.Environment}
	}
	return &struct {
		URL     *string            `json:"url"`
		Headers *map[string]string `json:"headers"`
	}{&input.URL, &input.Headers}
}

func configFromInput(input SetupInput, safe json.RawMessage) Config {
	return Config{TransportKind: input.TransportKind, SafeConfig: safe, Secrets: input.Secrets}
}

func validateSetup(input SetupInput) error {
	if strings.TrimSpace(input.DisplayName) == "" || len(input.DisplayName) > 256 {
		return errors.New("MCP display name is invalid")
	}
	if input.TransportKind == "stdio" {
		if strings.TrimSpace(input.Command) == "" || input.URL != "" {
			return errors.New("MCP stdio configuration is invalid")
		}
	} else if input.TransportKind == "streamable_http" {
		if input.URL == "" || input.Command != "" {
			return errors.New("MCP HTTP configuration is invalid")
		}
	} else {
		return errors.New("MCP transport is unsupported")
	}
	for _, values := range []map[string]string{input.Environment, input.Headers, input.Secrets.Environment, input.Secrets.Headers} {
		if len(values) > 64 {
			return errors.New("MCP configuration has too many entries")
		}
		for key, value := range values {
			if strings.TrimSpace(key) == "" || len(key) > 256 || len(value) > 16384 {
				return errors.New("MCP configuration entry is invalid")
			}
		}
	}
	for key := range input.Headers {
		switch strings.ToLower(strings.TrimSpace(key)) {
		case "authorization", "cookie", "proxy-authorization":
			return errors.New("credential HTTP headers must use secretHeaders")
		}
	}
	return nil
}

func storedTools(serverID string, discovered []DiscoveredTool) ([]store.MCPTool, error) {
	result := make([]store.MCPTool, 0, len(discovered))
	seenNames := make(map[string]struct{}, len(discovered))
	for _, source := range discovered {
		if _, exists := seenNames[source.Name]; exists {
			return nil, errors.New("MCP tool catalog contains duplicate names")
		}
		seenNames[source.Name] = struct{}{}
		id, err := newPrefixedID("mcp_tool:")
		if err != nil {
			return nil, err
		}
		readOnly, readSource, defaulted := hint(source.ReadOnly, false)
		idempotent, idempotentSource, d := hint(source.Idempotent, false)
		defaulted = defaulted || d
		destructive, destructiveSource, d := hint(source.Destructive, true)
		defaulted = defaulted || d
		openWorld, openWorldSource, d := hint(source.OpenWorld, true)
		defaulted = defaulted || d
		status := "ready"
		if defaulted {
			status = "defaulted"
		}
		result = append(result, store.MCPTool{ID: id, ServerID: serverID, Name: source.Name,
			Description: source.Description, InputSchema: source.InputSchema, OutputSchema: source.OutputSchema,
			Annotations: source.Annotations, SourceRevision: source.SourceRevision,
			ReadOnly: store.MCPHint{Value: readOnly, Source: readSource}, Idempotent: store.MCPHint{Value: idempotent, Source: idempotentSource},
			Destructive: store.MCPHint{Value: destructive, Source: destructiveSource}, OpenWorld: store.MCPHint{Value: openWorld, Source: openWorldSource},
			Status: status, PolicyRevision: 1})
	}
	return result, nil
}

func hint(value *bool, fallback bool) (*bool, string, bool) {
	if value != nil {
		result := *value
		return &result, "annotation", false
	}
	return &fallback, "safe_default", true
}
func authStatus(value SecretMaterial) string {
	if len(value.Environment) != 0 || len(value.Headers) != 0 || value.OAuth != nil {
		return "authenticated"
	}
	return "none"
}
func hasSecretMaterial(v SecretMaterial) bool {
	return len(v.Environment) != 0 || len(v.Headers) != 0 || v.OAuth != nil || v.Client != nil
}
func mergeSecrets(current, replacement SecretMaterial) SecretMaterial {
	if current.Environment == nil {
		current.Environment = map[string]string{}
	}
	for k, v := range replacement.Environment {
		current.Environment[k] = v
	}
	if current.Headers == nil {
		current.Headers = map[string]string{}
	}
	for k, v := range replacement.Headers {
		current.Headers[k] = v
	}
	if replacement.Client != nil {
		current.Client = replacement.Client
	}
	if replacement.OAuth != nil {
		current.OAuth = replacement.OAuth
	}
	return current
}
func restoreConnectionSecrets(secrets *secretStore, id string, old SecretMaterial) error {
	if old.Revision == "" {
		return secrets.removeConnection(id)
	}
	return secrets.writeConnection(id, old)
}
func reviewRoute(server store.MCPServer, behavior store.ActionBehavior) store.ActionReviewRoute {
	risky := (!behavior.ReadOnly && (behavior.Destructive || behavior.OpenWorld)) || server.DataSharingPolicy == "review_every_call"
	if !risky || server.UnsafeActionPolicy == "never_ask" {
		return ""
	}
	if server.UnsafeActionPolicy == "always_ask" {
		return store.ActionHumanReview
	}
	return store.ActionLLMReview
}
func sameBindingAuthority(left, right Binding) bool {
	return left.Name == right.Name && left.ServerID == right.ServerID && left.ToolID == right.ToolID &&
		left.SourceRevision == right.SourceRevision && left.ConnectionRevision == right.ConnectionRevision &&
		left.ServerPolicyRevision == right.ServerPolicyRevision && left.ToolPolicyRevision == right.ToolPolicyRevision &&
		left.InvokerKey == right.InvokerKey && left.OperationToken == right.OperationToken &&
		left.PersistencePolicy == right.PersistencePolicy && left.Behavior == right.Behavior &&
		left.ReviewRoute == right.ReviewRoute && string(left.InputSchema) == string(right.InputSchema) &&
		sameCapabilityDestination(left.Destination, right.Destination)
}

func sameCapabilityDestination(left, right *store.CapabilityDestination) bool {
	if left == nil || right == nil {
		return left == right
	}
	return left.ServiceID == right.ServiceID && left.ConnectionID == right.ConnectionID && left.Revision == right.Revision &&
		(left.AccountID == nil && right.AccountID == nil || left.AccountID != nil && right.AccountID != nil && *left.AccountID == *right.AccountID) &&
		(left.AuthenticationRevision == nil && right.AuthenticationRevision == nil || left.AuthenticationRevision != nil && right.AuthenticationRevision != nil && *left.AuthenticationRevision == *right.AuthenticationRevision)
}
func emptyNil(value string) any {
	if value == "" {
		return nil
	}
	return value
}
func randomHex() (string, error) {
	var value [16]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", err
	}
	return hex.EncodeToString(value[:]), nil
}
func newPrefixedID(prefix string) (string, error) {
	value, err := randomHex()
	return prefix + value, err
}

// GenerationTools converts current bindings without changing source schemas.
func GenerationTools(bindings []Binding) []provider.GenerationTool {
	result := make([]provider.GenerationTool, len(bindings))
	for i, b := range bindings {
		result[i] = provider.GenerationTool{Name: b.Name, Description: b.Description, InputSchema: append(json.RawMessage(nil), b.InputSchema...)}
	}
	return result
}
