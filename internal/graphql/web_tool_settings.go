package graphql

import (
	"context"
	"errors"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) webToolSettings(ctx context.Context) (*model.WebToolSettings, error) {
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	accounts, err := r.ProviderAccounts.Accounts(ctx)
	if err != nil {
		return nil, err
	}
	native := r.nativeWebAccount(ctx, accounts)
	override, err := r.Store.HasWebProviderOverride(ctx)
	if err != nil {
		return nil, err
	}
	search, err := r.webBindingSettings(ctx, "web.search", accounts, native, !override)
	if err != nil {
		return nil, err
	}
	fetch, err := r.webBindingSettings(ctx, "web.fetch", accounts, native, !override)
	if err != nil {
		return nil, err
	}
	browse, err := r.webBindingSettings(ctx, "web.browse", accounts, nil, false)
	if err != nil {
		return nil, err
	}
	return &model.WebToolSettings{Search: search, Fetch: fetch, Browse: browse}, nil
}

func (r *Resolver) saveWebToolProviderBinding(ctx context.Context, input model.SaveWebToolProviderBindingInput) (*model.WebToolBindingSettings, error) {
	if input.ToolName != input.CapabilityID || (input.ToolName != "web.search" && input.ToolName != "web.fetch" && input.ToolName != "web.browse") {
		return nil, errors.New("tool and capability do not match")
	}
	settings, err := r.webToolSettings(ctx)
	if err != nil {
		return nil, err
	}
	selected := settings.Search
	if input.ToolName == "web.fetch" {
		selected = settings.Fetch
	} else if input.ToolName == "web.browse" {
		selected = settings.Browse
	}
	valid := false
	for _, option := range selected.ProviderOptions {
		valid = valid || option.ProviderAccountID == input.ProviderAccountID
	}
	if !valid {
		return nil, errors.New("provider account does not supply the requested capability")
	}
	accounts, _ := r.ProviderAccounts.Accounts(ctx)
	native := r.nativeWebAccount(ctx, accounts)
	if input.ToolName != "web.browse" && native != nil && native.ID == input.ProviderAccountID {
		err = r.Store.ClearWebProviderOverrides(ctx)
	} else if input.ToolName == "web.browse" {
		err = r.Store.SaveBrowserProviderRoute(ctx, []string{input.ProviderAccountID}, time.Now())
	} else {
		err = r.Store.SaveWebProviderBinding(ctx, input.ToolName, input.ProviderAccountID, time.Now())
	}
	if err != nil {
		return nil, err
	}
	updated, err := r.webToolSettings(ctx)
	if err != nil {
		return nil, err
	}
	if input.ToolName == "web.search" {
		return updated.Search, nil
	}
	if input.ToolName == "web.fetch" {
		return updated.Fetch, nil
	}
	return updated.Browse, nil
}

func (r *Resolver) saveBrowserProviderRoute(ctx context.Context, input model.SaveBrowserProviderRouteInput) (*model.WebToolBindingSettings, error) {
	settings, err := r.webToolSettings(ctx)
	if err != nil {
		return nil, err
	}
	available := map[string]bool{}
	for _, option := range settings.Browse.ProviderOptions {
		available[option.ProviderAccountID] = true
	}
	for _, id := range input.ProviderAccountIds {
		if !available[id] {
			return nil, errors.New("provider account does not supply interactive browsing")
		}
	}
	if err := r.Store.SaveBrowserProviderRoute(ctx, input.ProviderAccountIds, time.Now()); err != nil {
		return nil, err
	}
	updated, err := r.webToolSettings(ctx)
	if err != nil {
		return nil, err
	}
	return updated.Browse, nil
}

func (r *Resolver) nativeWebAccount(ctx context.Context, accounts []provider.Account) *provider.Account {
	assignments, err := r.Store.HostedModelAssignments(ctx)
	if err != nil {
		return nil
	}
	for _, assignment := range assignments {
		if assignment.Role != store.HostedModelNoema || !nativeWebProvider(assignment.ProviderKind) {
			continue
		}
		for index := range accounts {
			if accounts[index].ID == assignment.ProviderAccountID && accounts[index].IsActive && accounts[index].Status == provider.StatusAuthenticated {
				return &accounts[index]
			}
		}
	}
	return nil
}

func (r *Resolver) webBindingSettings(ctx context.Context, name string, accounts []provider.Account, native *provider.Account, useNative bool) (*model.WebToolBindingSettings, error) {
	options := make([]*model.WebToolProviderOption, 0)
	if native != nil && name != "web.browse" {
		options = append(options, &model.WebToolProviderOption{ProviderAccountID: native.ID, ProviderKind: native.ProviderKind,
			AccountKey: native.AccountKey, DisplayName: native.DisplayName, CapabilityID: name,
			ReliabilityContract: "hosted_provider", DataFlowClass: "model_provider_prompt", Citations: true, DirectURLFetch: name == "web.fetch"})
	}
	for _, account := range accounts {
		if !account.IsActive {
			continue
		}
		for _, capability := range provider.Capabilities(account) {
			if capability.ID != name || capability.Status != "available" {
				continue
			}
			options = append(options, &model.WebToolProviderOption{ProviderAccountID: account.ID, ProviderKind: account.ProviderKind,
				AccountKey: account.AccountKey, DisplayName: account.DisplayName, CapabilityID: capability.ID,
				ReliabilityContract: capability.ReliabilityContract, DataFlowClass: capability.DataFlowClass,
				Citations: capability.Features.Citations, DirectURLFetch: capability.Features.DirectURLFetch,
				JsRendering: capability.Features.JSRendering, AuthenticatedContext: capability.Features.AuthenticatedContext})
		}
	}
	defaultID := map[string]string{"web.search": "provider_account:duckduckgo_public:system", "web.fetch": "provider_account:direct_http:system", "web.browse": "provider_account:obscura:system"}[name]
	route, err := r.Store.WebProviderRoute(ctx, name)
	if err != nil {
		return nil, err
	}
	ids := make([]string, 0, len(route))
	for _, binding := range route {
		for _, option := range options {
			if option.ProviderAccountID == binding.ProviderAccountID {
				ids = append(ids, binding.ProviderAccountID)
				break
			}
		}
	}
	active := defaultID
	if len(ids) != 0 {
		active = ids[0]
	}
	if useNative && native != nil {
		active, ids = native.ID, []string{native.ID}
	} else if len(ids) == 0 {
		ids = []string{active}
	}
	return &model.WebToolBindingSettings{ToolName: name, CapabilityID: name, ActiveProviderAccountID: active,
		ProviderRouteAccountIds: ids, ProviderOptions: options}, nil
}

func nativeWebProvider(kind string) bool {
	return kind == "openai" || kind == "codex" || kind == "openrouter"
}
