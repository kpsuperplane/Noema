package mcp

import (
	"net/http/httptest"
	"testing"
)

func TestAbsoluteCallbackUsesForwardedTLS(t *testing.T) {
	request := httptest.NewRequest("GET", "/mcp/oauth/callback?attemptId=ordinary", nil)
	request.Host = "noema.example"
	request.Header.Set("X-Forwarded-Proto", "https")
	if got, want := absoluteCallback(request), "https://noema.example/mcp/oauth/callback?attemptId=ordinary"; got != want {
		t.Fatalf("forwarded callback = %q, want %q", got, want)
	}
}
