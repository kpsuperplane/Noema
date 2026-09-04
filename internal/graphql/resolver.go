package graphql

import (
	"context"
	"os"
	"sync"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

type providerAuthService interface {
	StartAuth(context.Context, string, string, provider.AuthMethod) (provider.AuthAttempt, error)
	Attempt(string) (provider.AuthAttempt, bool)
	Cancel(string) (provider.AuthAttempt, bool)
	Subscribe(context.Context, string) (<-chan provider.AuthAttempt, error)
}

// Resolver owns GraphQL access to the Go server store.
type Resolver struct {
	Store            *store.Store
	Auth             *auth.Server
	home             *os.Root
	ProviderAccounts *provider.AccountService
	OpenRouter       *provider.OpenRouterService
	Chat             *runtime.Chat
	Artifacts        *artifact.Service
	providerAuth     map[string]providerAuthService
	projectMu        sync.Mutex
}

// NewResolver creates a GraphQL resolver for one open store.
func NewResolver(
	taskStore *store.Store,
	homeRoot *os.Root,
	authentication *auth.Server,
	providerAccounts *provider.AccountService,
	openRouter *provider.OpenRouterService,
	chat *runtime.Chat,
	codex *provider.CodexService,
	artifacts *artifact.Service,
) *Resolver {
	resolver := &Resolver{
		Store:            taskStore,
		Auth:             authentication,
		home:             homeRoot,
		ProviderAccounts: providerAccounts,
		OpenRouter:       openRouter,
		Chat:             chat,
		Artifacts:        artifacts,
		providerAuth:     make(map[string]providerAuthService),
	}
	if openRouter != nil {
		resolver.providerAuth["openrouter"] = openRouter
	}
	if codex != nil {
		resolver.providerAuth["codex"] = codex
	}
	return resolver
}
