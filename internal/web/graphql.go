package web

import (
	"io/fs"
	"net/http"
)

const graphiQLPolicy = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"

// NewGraphQLHandler serves GraphQL, its schema, and the optional local GraphiQL page.
func NewGraphQLHandler(graphql http.Handler, schema []byte, graphiQL bool) http.Handler {
	assets := assetFileSystem()
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/graphql/schema.graphql":
			if r.Method != http.MethodGet {
				WriteNotFound(w, r)
				return
			}
			w.Header().Set("Content-Type", "text/plain; charset=utf-8")
			w.Header().Set("Cache-Control", "no-store")
			_, _ = w.Write(schema)
		case r.URL.Path == "/graphql" && r.Method == http.MethodGet:
			if !graphiQL {
				WriteNotFound(w, r)
				return
			}
			body, err := fs.ReadFile(assets, "graphiql.html")
			if err != nil {
				http.Error(w, "GraphiQL asset is unavailable", http.StatusServiceUnavailable)
				return
			}
			w.Header().Set("Content-Type", "text/html; charset=utf-8")
			w.Header().Set("Cache-Control", "no-store")
			w.Header().Set("Content-Security-Policy", graphiQLPolicy)
			_, _ = w.Write(body)
		case r.URL.Path == "/graphql" && r.Method == http.MethodPost,
			r.URL.Path == "/graphql/ws" && r.Method == http.MethodGet:
			graphql.ServeHTTP(w, r)
		default:
			WriteNotFound(w, r)
		}
	})
}
