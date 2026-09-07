package auth

import "context"

type browserSessionContextKey struct{}
type desktopAccessContextKey struct{}
type humanPrincipalContextKey struct{}

// BrowserSessionHash returns the authenticated browser binding for this request.
func BrowserSessionHash(ctx context.Context) ([32]byte, bool) {
	value, ok := ctx.Value(browserSessionContextKey{}).([32]byte)
	return value, ok
}

func withBrowserSession(ctx context.Context, digest [32]byte) context.Context {
	return context.WithValue(ctx, browserSessionContextKey{}, digest)
}

func DesktopAccess(ctx context.Context) bool {
	access, _ := ctx.Value(desktopAccessContextKey{}).(bool)
	return access
}

func WithDesktopAccess(ctx context.Context) context.Context {
	return context.WithValue(ctx, desktopAccessContextKey{}, true)
}

// HumanPrincipal identifies the human subject that authenticated a request.
func HumanPrincipal(ctx context.Context) string {
	value, _ := ctx.Value(humanPrincipalContextKey{}).(string)
	return value
}

// WithHumanPrincipal attaches an authenticated human subject to a request.
func WithHumanPrincipal(ctx context.Context, subject string) context.Context {
	return context.WithValue(ctx, humanPrincipalContextKey{}, subject)
}
