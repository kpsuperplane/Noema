package graphql

import (
	"context"
	"net/http"
	"strings"
	"time"

	gqlgen "github.com/99designs/gqlgen/graphql"
	"github.com/99designs/gqlgen/graphql/handler"
	"github.com/99designs/gqlgen/graphql/handler/extension"
	"github.com/99designs/gqlgen/graphql/handler/lru"
	"github.com/99designs/gqlgen/graphql/handler/transport"
	"github.com/vektah/gqlparser/v2/ast"
)

// NewHandler creates the HTTP and graphql-transport-ws GraphQL handler.
func NewHandler(resolver *Resolver) http.Handler {
	server := handler.New(NewExecutableSchema(Config{Resolvers: resolver}))
	server.AddTransport(transport.Websocket{
		KeepAlivePingInterval: 10 * time.Second,
		InitTimeout:           10 * time.Second,
	})
	server.AddTransport(transport.Options{})
	server.AddTransport(transport.GET{})
	server.AddTransport(transport.POST{})
	server.SetQueryCache(lru.New[*ast.QueryDocument](1000))
	server.Use(extension.Introspection{})
	server.SetRecoverFunc(func(ctx context.Context, recovered any) error {
		if err, ok := recovered.(error); ok && strings.HasPrefix(err.Error(), "not implemented:") {
			return err
		}
		return gqlgen.DefaultRecover(ctx, recovered)
	})
	return server
}
