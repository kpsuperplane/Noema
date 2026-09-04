package provider

import "testing"

func TestCapabilitiesPreserveProviderContracts(t *testing.T) {
	for _, test := range []struct {
		kind       string
		status     AccountStatus
		wantIDs    []string
		wantStatus string
	}{
		{"codex", StatusAuthenticated, []string{"model.generate", "model.classify"}, "available"},
		{"local_models", StatusUnknown, []string{"model.generate", "model.classify"}, "account_dependent"},
		{"duckduckgo_public", StatusUnavailable, []string{"web.search"}, "available"},
		{"direct_http", StatusUnavailable, []string{"web.fetch"}, "available"},
		{"obscura", StatusUnavailable, []string{"web.browse"}, "available"},
		{"kernel", StatusUnauthenticated, []string{"web.browse"}, "unavailable"},
		{"exa", StatusAuthenticated, []string{"web.search", "web.fetch"}, "available"},
		{"tinyfish", StatusAuthenticated, []string{"web.search", "web.fetch"}, "available"},
	} {
		t.Run(test.kind, func(t *testing.T) {
			capabilities := Capabilities(Account{ProviderKind: test.kind, Status: test.status})
			if len(capabilities) != len(test.wantIDs) {
				t.Fatalf("capability count = %d, want %d", len(capabilities), len(test.wantIDs))
			}
			for index, capability := range capabilities {
				if capability.ID != test.wantIDs[index] || capability.Status != test.wantStatus {
					t.Fatalf("capability %d = %#v", index, capability)
				}
			}
		})
	}

	browse := Capabilities(Account{ProviderKind: "obscura"})[0]
	if !browse.Features.AuthenticatedContext || !browse.Features.JSRendering || !browse.Features.DirectURLFetch {
		t.Fatalf("interactive browser features = %#v", browse.Features)
	}
	fetch := Capabilities(Account{ProviderKind: "tinyfish", Status: StatusAuthenticated})[1]
	if !fetch.Features.Citations || !fetch.Features.JSRendering || fetch.Features.ResultPersistence != "compact_content" {
		t.Fatalf("hosted fetch features = %#v", fetch.Features)
	}
}
