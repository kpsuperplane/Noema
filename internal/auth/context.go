package auth

import "context"

type browserSessionContextKey struct{}

// BrowserSessionHash returns the authenticated browser binding for this request.
func BrowserSessionHash(ctx context.Context) ([32]byte, bool) {
	value, ok := ctx.Value(browserSessionContextKey{}).([32]byte)
	return value, ok
}

func withBrowserSession(ctx context.Context, digest [32]byte) context.Context {
	return context.WithValue(ctx, browserSessionContextKey{}, digest)
}
