package auth

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
)

const (
	defaultProvider            = "openai"
	defaultOpenAIModel         = "gpt-5.6-terra"
	defaultOpenAIBaseURL       = "https://api.openai.com/v1"
	defaultOpenAITimeout       = 120
	defaultOpenRouterModel     = "openrouter/auto"
	defaultOpenRouterBaseURL   = "https://openrouter.ai/api/v1"
	defaultOpenRouterTimeout   = 120
	defaultCodexBaseURL        = "https://chatgpt.com/backend-api/codex"
	defaultCodexTimeout        = 300
	defaultFoundationProfile   = "default"
	defaultLocalModel          = "default"
	defaultLocalContextTokens  = 8192
	defaultLocalTimeout        = 600
	defaultLocalStartupTimeout = 180
)

// ProviderConfigErrorKind classifies one provider-resolution failure.
type ProviderConfigErrorKind string

const (
	ProviderConfigFileNotFound  ProviderConfigErrorKind = "config_file_not_found"
	ProviderConfigParse         ProviderConfigErrorKind = "parse_config"
	ProviderConfigUnsupported   ProviderConfigErrorKind = "unsupported_provider"
	ProviderConfigMissingSecret ProviderConfigErrorKind = "missing_credential"
	ProviderConfigInvalidNumber ProviderConfigErrorKind = "invalid_integer"
	ProviderConfigInvalid       ProviderConfigErrorKind = "invalid_config"
)

// ProviderConfigError is the typed error returned by provider resolution.
type ProviderConfigError struct {
	Kind       ProviderConfigErrorKind
	Path       string
	Provider   string
	Credential string
	Name       string
	Value      string
	Message    string
}

func (e *ProviderConfigError) Error() string {
	switch e.Kind {
	case ProviderConfigFileNotFound:
		return "config file not found: " + e.Path
	case ProviderConfigParse:
		return "failed to parse config file " + e.Path + ": " + e.Message
	case ProviderConfigUnsupported:
		return "unsupported provider: " + e.Provider
	case ProviderConfigMissingSecret:
		return "missing credentials for " + e.Provider + ": " + e.Credential
	case ProviderConfigInvalidNumber:
		return "invalid integer value for " + e.Name + ": " + e.Value
	case ProviderConfigInvalid:
		return "invalid config: " + e.Message
	default:
		return e.Message
	}
}

// OpenAIProviderConfig contains the resolved OpenAI provider settings.
type OpenAIProviderConfig struct {
	APIKey                  provider.Secret
	BaseURL                 string
	OrganizationID          string
	ProjectID               string
	DefaultModel            string
	ToolClassificationModel string
	ReasoningEffort         string
	TimeoutSeconds          int
}

// CodexProviderConfig contains the resolved Codex provider settings.
type CodexProviderConfig struct {
	BaseURL                 string
	DefaultModel            string
	ToolClassificationModel string
	ReasoningEffort         string
	TimeoutSeconds          int
}

// OpenRouterProviderConfig contains the resolved OpenRouter settings.
type OpenRouterProviderConfig struct {
	BaseURL         string
	DefaultModel    string
	ReasoningEffort string
	TimeoutSeconds  int
}

// FoundationLocalProviderConfig contains the resolved Foundation profile.
type FoundationLocalProviderConfig struct {
	DefaultProfile string
	BridgePath     string
}

// LocalModelsProviderConfig contains the resolved local-model settings.
type LocalModelsProviderConfig struct {
	DefaultModel          string
	ContextWindowTokens   int
	TimeoutSeconds        int
	StartupTimeoutSeconds int
}

// ResolvedProviderConfig is the selected provider and its typed settings.
type ResolvedProviderConfig struct {
	Provider                string
	Model                   string
	ReasoningEffort         string
	ToolClassificationModel string
	OpenAI                  OpenAIProviderConfig
	Codex                   CodexProviderConfig
	OpenRouter              OpenRouterProviderConfig
	FoundationLocal         FoundationLocalProviderConfig
	LocalModels             LocalModelsProviderConfig
}

// ResolveProviderConfig resolves the provider from config.yaml and the
// supported NOEMA_ environment settings. An explicit path is authoritative.
func ResolveProviderConfig(paths home.Paths, explicitPath string) (ResolvedProviderConfig, error) {
	path := paths.Config()
	if explicitPath != "" {
		path = explicitPath
		if _, err := os.Stat(path); errors.Is(err, os.ErrNotExist) {
			return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigFileNotFound, Path: path}
		} else if err != nil {
			return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigParse, Path: path, Message: err.Error()}
		}
	}
	document, err := readConfigDocument(path)
	if err != nil {
		return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigParse, Path: path, Message: err.Error()}
	}
	if document == nil {
		return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigInvalid, Message: "config.yaml must contain a mapping"}
	}
	if err := rejectFileSecrets(document); err != nil {
		return ResolvedProviderConfig{}, err
	}
	applyProviderEnvironment(document)
	selected := stringValue(document, "provider", defaultProvider)
	switch selected {
	case "openai":
		return resolveOpenAI(document)
	case "codex":
		return resolveCodex(document)
	case "openrouter":
		return resolveOpenRouter(document)
	case "foundation_local":
		return resolveFoundation(document)
	case "local_models":
		return resolveLocalModels(document)
	default:
		return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigUnsupported, Provider: selected}
	}
}

func resolveOpenAI(document map[string]any) (ResolvedProviderConfig, error) {
	section := nestedMap(document, "openai")
	model := nonEmpty(stringValueOK(document, "model"))
	effort := nonEmpty(stringValueOK(document, "reasoning_effort"))
	if err := validateReasoning(model, effort, "openai"); err != nil {
		return ResolvedProviderConfig{}, err
	}
	if model == "" {
		model = defaultOpenAIModel
	}
	if effort == "" {
		effort = "medium"
	}
	key := strings.TrimSpace(os.Getenv("NOEMA_OPENAI__API_KEY"))
	if key == "" {
		return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigMissingSecret, Provider: "openai", Credential: "NOEMA_OPENAI__API_KEY"}
	}
	secret, err := provider.NewSecret(key)
	if err != nil {
		return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigMissingSecret, Provider: "openai", Credential: "NOEMA_OPENAI__API_KEY"}
	}
	baseURL := strings.TrimRight(stringValue(section, "base_url", defaultOpenAIBaseURL), "/")
	timeout, err := positiveInt(section, "timeout_seconds", defaultOpenAITimeout, "NOEMA_OPENAI__TIMEOUT_SECONDS")
	if err != nil {
		return ResolvedProviderConfig{}, err
	}
	if value, exists := os.LookupEnv("NOEMA_OPENAI__BASE_URL"); exists {
		baseURL = strings.TrimRight(value, "/")
	}
	if value, exists := os.LookupEnv("NOEMA_OPENAI__TIMEOUT_SECONDS"); exists {
		timeout, err = positiveEnvInt(value, "NOEMA_OPENAI__TIMEOUT_SECONDS")
		if err != nil {
			return ResolvedProviderConfig{}, err
		}
	}
	result := ResolvedProviderConfig{Provider: "openai", Model: model, ReasoningEffort: effort,
		ToolClassificationModel: toolClassificationModel(document, section),
		OpenAI:                  OpenAIProviderConfig{APIKey: secret, BaseURL: baseURL, DefaultModel: model, ReasoningEffort: effort, TimeoutSeconds: timeout}}
	result.OpenAI.OrganizationID = stringValue(section, "organization_id", "")
	result.OpenAI.ProjectID = stringValue(section, "project_id", "")
	if value, exists := os.LookupEnv("NOEMA_OPENAI__ORGANIZATION_ID"); exists {
		result.OpenAI.OrganizationID = value
	}
	if value, exists := os.LookupEnv("NOEMA_OPENAI__PROJECT_ID"); exists {
		result.OpenAI.ProjectID = value
	}
	result.OpenAI.ToolClassificationModel = result.ToolClassificationModel
	return result, nil
}

func resolveCodex(document map[string]any) (ResolvedProviderConfig, error) {
	section := nestedMap(document, "codex")
	model := nonEmpty(stringValueOK(document, "model"))
	if model == "" {
		model = nonEmpty(stringValueOK(section, "model"))
	}
	effort := nonEmpty(stringValueOK(document, "reasoning_effort"))
	if effort == "" {
		effort = nonEmpty(stringValueOK(section, "reasoning_effort"))
	}
	if err := validateReasoning(model, effort, "codex"); err != nil {
		return ResolvedProviderConfig{}, err
	}
	if model == "" {
		model = defaultOpenAIModel
	}
	if effort == "" {
		effort = "medium"
	}
	baseURL := strings.TrimRight(stringValue(section, "base_url", defaultCodexBaseURL), "/")
	timeout, err := positiveInt(section, "timeout_seconds", defaultCodexTimeout, "NOEMA_CODEX__TIMEOUT_SECONDS")
	if err != nil {
		return ResolvedProviderConfig{}, err
	}
	if value, exists := os.LookupEnv("NOEMA_CODEX__BASE_URL"); exists {
		baseURL = strings.TrimRight(value, "/")
	}
	if value, exists := os.LookupEnv("NOEMA_CODEX__TIMEOUT_SECONDS"); exists {
		timeout, err = positiveEnvInt(value, "NOEMA_CODEX__TIMEOUT_SECONDS")
		if err != nil {
			return ResolvedProviderConfig{}, err
		}
	}
	tool := toolClassificationModel(document, section)
	return ResolvedProviderConfig{Provider: "codex", Model: model, ReasoningEffort: effort,
		ToolClassificationModel: tool, Codex: CodexProviderConfig{BaseURL: baseURL, DefaultModel: model,
			ToolClassificationModel: tool, ReasoningEffort: effort, TimeoutSeconds: timeout}}, nil
}

func resolveOpenRouter(document map[string]any) (ResolvedProviderConfig, error) {
	section := nestedMap(document, "openrouter")
	model := nonEmpty(stringValueOK(document, "model"))
	if model == "" {
		model = defaultOpenRouterModel
	}
	effort := nonEmpty(stringValueOK(document, "reasoning_effort"))
	if err := validateReasoning(modelIfExplicit(document), effort, "openrouter"); err != nil {
		return ResolvedProviderConfig{}, err
	}
	baseURL := strings.TrimRight(stringValue(section, "base_url", defaultOpenRouterBaseURL), "/")
	timeout, err := positiveInt(section, "timeout_seconds", defaultOpenRouterTimeout, "NOEMA_OPENROUTER__TIMEOUT_SECONDS")
	if err != nil {
		return ResolvedProviderConfig{}, err
	}
	return ResolvedProviderConfig{Provider: "openrouter", Model: model, ReasoningEffort: effort,
		ToolClassificationModel: toolClassificationModel(document, section),
		OpenRouter:              OpenRouterProviderConfig{BaseURL: baseURL, DefaultModel: model, ReasoningEffort: effort, TimeoutSeconds: timeout}}, nil
}

func resolveFoundation(document map[string]any) (ResolvedProviderConfig, error) {
	section := nestedMap(document, "foundation_local")
	effort := nonEmpty(stringValueOK(document, "reasoning_effort"))
	if effort != "" {
		return ResolvedProviderConfig{}, &ProviderConfigError{Kind: ProviderConfigInvalid, Message: "foundation_local does not support reasoning_effort"}
	}
	profile := stringValue(section, "default_profile", defaultFoundationProfile)
	if value, exists := os.LookupEnv("NOEMA_FOUNDATION_LOCAL__DEFAULT_PROFILE"); exists {
		profile = value
	}
	bridge := stringValue(section, "bridge_path", "")
	if value, exists := os.LookupEnv("NOEMA_FOUNDATION_LOCAL__BRIDGE_PATH"); exists {
		bridge = value
	}
	return ResolvedProviderConfig{Provider: "foundation_local", Model: profile,
		FoundationLocal: FoundationLocalProviderConfig{DefaultProfile: profile, BridgePath: bridge}}, nil
}

func resolveLocalModels(document map[string]any) (ResolvedProviderConfig, error) {
	section := nestedMap(document, "local_models")
	model := nonEmpty(stringValueOK(document, "model"))
	if model == "" {
		model = stringValue(section, "default_model", defaultLocalModel)
	}
	contextTokens, err := positiveInt(section, "context_window_tokens", defaultLocalContextTokens, "NOEMA_LOCAL_MODELS__CONTEXT_WINDOW_TOKENS")
	if err != nil {
		return ResolvedProviderConfig{}, err
	}
	timeout, err := positiveInt(section, "timeout_seconds", defaultLocalTimeout, "NOEMA_LOCAL_MODELS__TIMEOUT_SECONDS")
	if err != nil {
		return ResolvedProviderConfig{}, err
	}
	startup, err := positiveInt(section, "startup_timeout_seconds", defaultLocalStartupTimeout, "NOEMA_LOCAL_MODELS__STARTUP_TIMEOUT_SECONDS")
	if err != nil {
		return ResolvedProviderConfig{}, err
	}
	return ResolvedProviderConfig{Provider: "local_models", Model: model,
		LocalModels: LocalModelsProviderConfig{DefaultModel: model, ContextWindowTokens: contextTokens, TimeoutSeconds: timeout, StartupTimeoutSeconds: startup}}, nil
}

func rejectFileSecrets(document map[string]any) error {
	section, exists := document["openai"]
	if !exists {
		return nil
	}
	values, ok := section.(map[string]any)
	if !ok {
		return errInvalidConfig("openai configuration must be a mapping")
	}
	if _, exists := values["api_key"]; exists {
		return errInvalidConfig("openai.api_key must be supplied through NOEMA_OPENAI__API_KEY")
	}
	return nil
}

func applyProviderEnvironment(document map[string]any) {
	setStringEnv(document, "provider", "NOEMA_PROVIDER")
	setStringEnv(document, "model", "NOEMA_MODEL")
	setStringEnv(document, "reasoning_effort", "NOEMA_REASONING_EFFORT")
	setStringEnv(document, "tool_classification_model", "NOEMA_TOOL_CLASSIFICATION_MODEL")
	for section, keys := range map[string][]string{
		"codex":            {"model", "reasoning_effort", "tool_classification_model", "base_url", "timeout_seconds"},
		"openai":           {"base_url", "organization_id", "project_id", "tool_classification_model", "timeout_seconds"},
		"openrouter":       {"base_url", "tool_classification_model", "timeout_seconds"},
		"foundation_local": {"default_profile", "bridge_path"},
		"local_models":     {"default_model", "context_window_tokens", "timeout_seconds", "startup_timeout_seconds"},
	} {
		values := nestedMap(document, section)
		for _, key := range keys {
			name := "NOEMA_" + strings.ToUpper(section) + "__" + strings.ToUpper(key)
			if value, exists := os.LookupEnv(name); exists {
				if strings.HasSuffix(key, "seconds") || key == "context_window_tokens" {
					if parsed, err := strconv.Atoi(value); err == nil {
						values[key] = parsed
					} else {
						values[key] = value
					}
				} else {
					values[key] = value
				}
			}
		}
	}
}

func setStringEnv(document map[string]any, key, name string) {
	if value, exists := os.LookupEnv(name); exists {
		document[key] = value
	}
}

func nestedMap(document map[string]any, key string) map[string]any {
	if values, ok := document[key].(map[string]any); ok {
		return values
	}
	values := make(map[string]any)
	document[key] = values
	return values
}

func stringValue(document map[string]any, key, fallback string) string {
	value := nonEmpty(stringValueOK(document, key))
	if value == "" {
		return fallback
	}
	return value
}

func stringValueOK(document map[string]any, key string) string {
	value, _ := document[key].(string)
	return value
}

func toolClassificationModel(document, section map[string]any) string {
	if value := nonEmpty(stringValueOK(document, "tool_classification_model")); value != "" {
		return value
	}
	return nonEmpty(stringValueOK(section, "tool_classification_model"))
}

func modelIfExplicit(document map[string]any) string {
	return nonEmpty(stringValueOK(document, "model"))
}

func validateReasoning(model, effort, kind string) error {
	if model == "" && effort != "" {
		return errInvalidConfig(kind + " reasoning_effort requires an explicit model")
	}
	if model != "" && (kind == "codex" || kind == "openai") && effort == "" {
		return errInvalidConfig(kind + " explicit model requires reasoning_effort")
	}
	if effort != "" && kind != "codex" && kind != "openai" {
		return errInvalidConfig(kind + " does not support reasoning_effort")
	}
	return nil
}

func errInvalidConfig(message string) error {
	return &ProviderConfigError{Kind: ProviderConfigInvalid, Message: message}
}

func positiveInt(values map[string]any, key string, fallback int, envName string) (int, error) {
	value := values[key]
	if value == nil {
		return fallback, nil
	}
	parsed, ok := numberValue(value)
	if !ok || parsed <= 0 {
		return 0, &ProviderConfigError{Kind: ProviderConfigInvalidNumber, Name: envName, Value: fmt.Sprint(value)}
	}
	return parsed, nil
}

func positiveEnvInt(value, name string) (int, error) {
	parsed, err := strconv.Atoi(value)
	if err != nil || parsed <= 0 {
		return 0, &ProviderConfigError{Kind: ProviderConfigInvalidNumber, Name: name, Value: value}
	}
	return parsed, nil
}

func numberValue(value any) (int, bool) {
	switch number := value.(type) {
	case int:
		return number, true
	case int64:
		return int(number), true
	case uint64:
		return int(number), true
	case float64:
		return int(number), number == float64(int(number))
	case string:
		parsed, err := strconv.Atoi(number)
		return parsed, err == nil
	default:
		return 0, false
	}
}

func nonEmpty(value string) string { return strings.TrimSpace(value) }
