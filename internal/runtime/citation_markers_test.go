package runtime

import (
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestProviderCitationMarkersNormalizeDisplayAndDiagnostics(t *testing.T) {
	prefix := "😀 claim "
	marker := "\ue200cite\ue202turn5search4\ue201"
	raw := prefix + marker + "tail"
	start := utf16CodeUnitCount(prefix)
	end := start + utf16CodeUnitCount(marker)
	normalized := normalizeProviderText(raw, []provider.Citation{{
		Title: "Official", URL: "https://example.test", StartIndex: &start, EndIndex: &end,
	}})
	if normalized.Text != prefix+"tail" || len(normalized.Citations) != 1 ||
		normalized.Citations[0].StartIndex == nil || *normalized.Citations[0].StartIndex != start ||
		normalized.Citations[0].EndIndex == nil || *normalized.Citations[0].EndIndex != start ||
		normalized.UnresolvedMarkers != 0 {
		t.Fatalf("normalized citation text = %#v", normalized)
	}
	unknown := "\ue200cite\ue202credential-sentinel\ue201"
	unresolved := normalizeProviderText("claim "+unknown, nil)
	if unresolved.Text != "claim "+unknown || unresolved.UnresolvedMarkers != 1 {
		t.Fatalf("unresolved citation text = %#v", unresolved)
	}
}
