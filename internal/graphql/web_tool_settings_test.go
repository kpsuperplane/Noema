package graphql

import (
	"context"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
)

func TestWebToolSettingsPersistExactBindingsAndRoutes(t *testing.T) {
	resolver := openProviderTestResolver(t)
	settings, err := resolver.webToolSettings(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if settings.Search.ActiveProviderAccountID != "provider_account:duckduckgo_public:system" ||
		settings.Fetch.ActiveProviderAccountID != "provider_account:direct_http:system" ||
		settings.Browse.ActiveProviderAccountID != "provider_account:obscura:system" {
		t.Fatalf("unexpected defaults: %#v", settings)
	}
	native := &provider.Account{ID: "provider_account:openai:default", ProviderKind: "openai", AccountKey: "default", DisplayName: "OpenAI"}
	nativeSearch, _ := resolver.webBindingSettings(context.Background(), "web.search", nil, native, true)
	nativeFetch, _ := resolver.webBindingSettings(context.Background(), "web.fetch", nil, native, true)
	if option := nativeSearch.ProviderOptions[0]; option.DataFlowClass != "trusted_external_search_query" || !option.Citations {
		t.Fatalf("native search metadata = %#v", option)
	}
	if option := nativeFetch.ProviderOptions[0]; option.DataFlowClass != "external_web_fetch" || option.Citations || !option.DirectURLFetch {
		t.Fatalf("native fetch metadata = %#v", option)
	}
	secret, _ := provider.NewSecret("exa-secret")
	exa, err := resolver.ProviderAccounts.CreateSecretAccount(context.Background(), "exa", "Exa", secret, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	saved, err := resolver.saveWebToolProviderBinding(context.Background(), model.SaveWebToolProviderBindingInput{
		ToolName: "web.search", CapabilityID: "web.search", ProviderAccountID: exa.ID,
	})
	if err != nil || saved.ActiveProviderAccountID != exa.ID {
		t.Fatalf("saved = %#v, %v", saved, err)
	}
	if _, err := resolver.saveWebToolProviderBinding(context.Background(), model.SaveWebToolProviderBindingInput{
		ToolName: "web.search", CapabilityID: "web.fetch", ProviderAccountID: exa.ID,
	}); err == nil {
		t.Fatal("mismatched tool and capability accepted")
	}
	browse, err := resolver.saveBrowserProviderRoute(context.Background(), model.SaveBrowserProviderRouteInput{
		ProviderAccountIds: []string{"provider_account:obscura:system"},
	})
	if err != nil || len(browse.ProviderRouteAccountIds) != 1 {
		t.Fatalf("browse = %#v, %v", browse, err)
	}
}
