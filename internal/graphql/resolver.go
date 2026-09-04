package graphql

import (
	"os"
	"sync"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

// Resolver owns GraphQL access to the Go server store.
type Resolver struct {
	Store            *store.Store
	Auth             *auth.Server
	home             *os.Root
	ProviderAccounts *provider.AccountService
	OpenRouter       *provider.OpenRouterService
	Chat             *runtime.Chat

	subscriptionsMu sync.Mutex
	subscriptions   map[string]map[chan *model.TasksEvent]struct{}
}

// NewResolver creates a GraphQL resolver for one open store.
func NewResolver(
	taskStore *store.Store,
	homeRoot *os.Root,
	authentication *auth.Server,
	providerAccounts *provider.AccountService,
	openRouter *provider.OpenRouterService,
	chat *runtime.Chat,
) *Resolver {
	return &Resolver{
		Store:            taskStore,
		Auth:             authentication,
		home:             homeRoot,
		ProviderAccounts: providerAccounts,
		OpenRouter:       openRouter,
		Chat:             chat,
		subscriptions:    make(map[string]map[chan *model.TasksEvent]struct{}),
	}
}
