package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/auth"
)

// requestHumanID returns the authenticated human subject at the GraphQL
// boundary. Existing transports bind to the local human unless a subject is
// supplied explicitly by an authenticated transport.
func requestHumanID(ctx context.Context) string {
	if subject := auth.HumanPrincipal(ctx); subject != "" {
		return subject
	}
	if _, ok := auth.BrowserSessionHash(ctx); ok || auth.DesktopAccess(ctx) || auth.ClientID(ctx) != "" {
		return localHumanID
	}
	return ""
}

func requireRequestPrincipal(ctx context.Context) error {
	if requestHumanID(ctx) == "" {
		return errors.New("request is unauthenticated")
	}
	return nil
}
