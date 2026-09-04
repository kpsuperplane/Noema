package provider

// Capability describes one provider feature exposed to clients and routing.
type Capability struct {
	ID                  string
	Status              string
	ReliabilityContract string
	DataFlowClass       string
	Features            CapabilityFeatures
}

// CapabilityFeatures describes stable result behavior.
type CapabilityFeatures struct {
	Citations            bool
	DirectURLFetch       bool
	JSRendering          bool
	AuthenticatedContext bool
	ResultPersistence    string
}

// Capabilities derives current provider features from safe account state.
func Capabilities(account Account) []Capability {
	status := "unavailable"
	if account.Status == StatusAuthenticated {
		status = "available"
	} else if account.Status == StatusUnknown || account.Status == StatusChecking {
		status = "account_dependent"
	}

	switch account.ProviderKind {
	case "openai", "codex", "openrouter", "foundation_local":
		return modelCapabilities(status, "hosted_provider", "model_provider_prompt")
	case "local_models":
		return modelCapabilities(status, "first_party", "local_inference")
	case "duckduckgo_public":
		return []Capability{webCapability("web.search", "available", "best_effort_public", false, false)}
	case "direct_http":
		return []Capability{webCapability("web.fetch", "available", "first_party", false, false)}
	case "obscura":
		return []Capability{webCapability("web.browse", "available", "first_party", false, true)}
	case "kernel":
		return []Capability{webCapability("web.browse", status, "hosted_provider", false, true)}
	case "exa":
		return hostedWebCapabilities(status, false)
	case "tinyfish", "firecrawl":
		return hostedWebCapabilities(status, true)
	default:
		return nil
	}
}

func modelCapabilities(status string, reliability string, dataFlow string) []Capability {
	features := CapabilityFeatures{ResultPersistence: "compact_metadata"}
	return []Capability{
		{ID: "model.generate", Status: status, ReliabilityContract: reliability, DataFlowClass: dataFlow, Features: features},
		{ID: "model.classify", Status: status, ReliabilityContract: reliability, DataFlowClass: dataFlow, Features: features},
	}
}

func hostedWebCapabilities(status string, jsFetch bool) []Capability {
	return []Capability{
		webCapability("web.search", status, "hosted_provider", true, false),
		webCapability("web.fetch", status, "hosted_provider", true, jsFetch),
	}
}

func webCapability(id string, status string, reliability string, citations bool, jsRendering bool) Capability {
	dataFlow := "external_web_browse"
	directURL := true
	persistence := "compact_metadata"
	if id == "web.search" {
		dataFlow = "trusted_external_search_query"
		directURL = false
	} else if id == "web.fetch" {
		dataFlow = "external_web_fetch"
		persistence = "compact_content"
	}
	return Capability{
		ID: id, Status: status, ReliabilityContract: reliability, DataFlowClass: dataFlow,
		Features: CapabilityFeatures{
			Citations: citations, DirectURLFetch: directURL, JSRendering: jsRendering,
			AuthenticatedContext: id == "web.browse", ResultPersistence: persistence,
		},
	}
}
