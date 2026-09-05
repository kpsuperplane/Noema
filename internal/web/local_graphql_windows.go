//go:build windows

package web

import (
	"context"
	"errors"
	"net/http"
)

// LocalGraphQLServer is unavailable on Windows.
type LocalGraphQLServer struct{}

// NewLocalGraphQLServer rejects the unsupported Windows configuration.
func NewLocalGraphQLServer(_ string, _ http.Handler) (*LocalGraphQLServer, error) {
	return nil, errors.New("web.local_graphql_socket is not supported on Windows")
}

// Serve cannot run on Windows.
func (s *LocalGraphQLServer) Serve(context.Context) error { return nil }
