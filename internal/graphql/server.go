package graphql

import (
	"bytes"
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
	"github.com/vektah/gqlparser/v2/formatter"
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

// Schema returns the schema served by the executable GraphQL server.
func Schema() []byte {
	var schema bytes.Buffer
	formatter.NewFormatter(&schema).FormatSchema(NewExecutableSchema(Config{}).Schema())
	return canonicalSchemaSDL(schema.Bytes())
}

// canonicalSchemaSDL keeps the served introspection document byte-for-byte
// aligned with graphql/schema.graphql. gqlgen adds built-in directives and a
// schema declaration at the beginning, while the checked-in contract keeps
// those declarations at the end.
func canonicalSchemaSDL(raw []byte) []byte {
	lines := strings.Split(strings.TrimSuffix(string(raw), "\n"), "\n")
	start := -1
	for index := 0; index+1 < len(lines); index++ {
		if lines[index] == `"""` && lines[index+1] == "Validated durable A2UI surface revision." {
			start = index
			break
		}
	}
	if start < 0 {
		return raw
	}
	trailing := -1
	for index := start; index+1 < len(lines); index++ {
		if lines[index] == `"""` && strings.HasPrefix(lines[index+1], "Directs the executor to include") {
			trailing = index
			break
		}
	}
	var body, suffix []string
	if trailing < 0 {
		body, suffix = lines[start:], append(append([]string{}, lines[5:start]...), lines[:5]...)
	} else {
		body, suffix = lines[start:trailing], lines[trailing:]
	}
	canonical := make([]string, 0, len(body)+1)
	depth := 0
	for index, line := range body {
		canonical = append(canonical, line)
		trimmed := strings.TrimSpace(line)
		switch {
		case strings.HasSuffix(trimmed, "{"):
			depth++
		case trimmed == "}":
			depth--
			if depth == 0 && index+1 < len(body) && body[index+1] != "" {
				canonical = append(canonical, "")
			}
		case depth == 0 && (strings.HasPrefix(trimmed, "union ") || strings.HasPrefix(trimmed, "scalar ")) && index+1 < len(body) && body[index+1] != "":
			canonical = append(canonical, "")
		}
	}
	canonical = append(canonical, "")
	canonical = append(canonical, suffix...)
	return []byte(strings.Join(canonical, "\n") + "\n")
}
