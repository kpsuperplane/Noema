package runtime

import (
	"fmt"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestRustCitationStructuredAnnotationResolvesPrivateMarker(t *testing.T) {
	prefix := "😀 claim "
	text := fmt.Sprintf("%s\ue200cite\ue202turn5search4\ue201", prefix)
	start, end := utf16CodeUnitCount(prefix), utf16CodeUnitCount(text)
	result := normalizeProviderText(text, []provider.Citation{{Title: "Official source", URL: "https://one.example/a", StartIndex: &start, EndIndex: &end}})
	if result.Text != prefix || len(result.Citations) != 1 || result.Citations[0].Title != "Official source" ||
		result.Citations[0].EndIndex == nil || *result.Citations[0].EndIndex != 9 || result.UnresolvedMarkers != 0 {
		t.Fatalf("structured citation = %#v", result)
	}
}

func TestRustCitationPreservesUnresolvedMarkerWithoutInventingSource(t *testing.T) {
	marker := "\ue200cite\ue202turn1view1\ue201"
	result := normalizeProviderText("claim "+marker+" end", nil)
	if result.Text != "claim "+marker+" end" || len(result.Citations) != 0 || result.UnresolvedMarkers != 1 {
		t.Fatalf("unresolved citation = %#v", result)
	}
}

func TestRustCitationResolvesDirectHTTPSMarkerWithoutFetching(t *testing.T) {
	result := normalizeProviderText("claim \ue200cite\ue202https://example.com/news?id=1\ue201", nil)
	if result.Text != "claim " || len(result.Citations) != 1 || result.Citations[0].Title != "example.com" ||
		result.Citations[0].URL != "https://example.com/news?id=1" || result.Citations[0].EndIndex == nil ||
		*result.Citations[0].EndIndex != 6 || result.UnresolvedMarkers != 0 {
		t.Fatalf("direct HTTPS citation = %#v", result)
	}
}

func TestRustCitationAnnotationRangeAdjustsAfterPrivateMarkerRemoval(t *testing.T) {
	marker := "\ue200cite\ue202turn1view1\ue201"
	text := "claim" + marker + " tail"
	start, end := 0, utf16CodeUnitCount(text)
	result := normalizeProviderText(text, []provider.Citation{{Title: "Existing", URL: "https://existing.example", StartIndex: &start, EndIndex: &end}})
	if result.Text != "claim tail" || len(result.Citations) != 1 || result.Citations[0].URL != "https://existing.example" ||
		result.Citations[0].EndIndex == nil || *result.Citations[0].EndIndex != 10 {
		t.Fatalf("adjusted annotation = %#v", result)
	}
}
