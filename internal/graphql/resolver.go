package graphql

import (
	"os"
	"sync"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

// Resolver owns GraphQL access to the Go server store.
type Resolver struct {
	Store *store.Store
	home  *os.Root

	subscriptionsMu sync.Mutex
	subscriptions   map[string]map[chan *model.TasksEvent]struct{}
}

// NewResolver creates a GraphQL resolver for one open store.
func NewResolver(taskStore *store.Store, homeRoot *os.Root) *Resolver {
	return &Resolver{
		Store:         taskStore,
		home:          homeRoot,
		subscriptions: make(map[string]map[chan *model.TasksEvent]struct{}),
	}
}
