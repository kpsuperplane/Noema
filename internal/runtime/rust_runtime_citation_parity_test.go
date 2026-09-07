package runtime

import (
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestRustRuntime_structured_annotation_resolves_private_marker(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/citation_markers.rs::structured_annotation_resolves_private_marker.
	prefix := "😀 claim "
	marker := "\ue200cite\ue202turn5search4\ue201"
	start := utf16CodeUnitCount(prefix)
	end := start + utf16CodeUnitCount(marker)
	result := normalizeProviderText(prefix+marker, []provider.Citation{{Title: "Official source", URL: "https://one.example/a", StartIndex: &start, EndIndex: &end}})
	if result.Text != prefix || len(result.Citations) != 1 || result.Citations[0].Title != "Official source" || result.UnresolvedMarkers != 0 {
		t.Fatalf("normalized citation = %#v", result)
	}
	if result.Citations[0].EndIndex == nil || *result.Citations[0].EndIndex != utf16CodeUnitCount(prefix) {
		t.Fatalf("citation range = %#v", result.Citations[0])
	}
}

func TestRustRuntime_preserves_unresolved_markers_without_inventing_sources(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/citation_markers.rs::preserves_unresolved_markers_without_inventing_sources.
	marker := "\ue200cite\ue202turn1view1\ue201"
	result := normalizeProviderText("claim "+marker+" end", nil)
	if result.Text != "claim "+marker+" end" || len(result.Citations) != 0 || result.UnresolvedMarkers != 1 {
		t.Fatalf("unresolved marker = %#v", result)
	}
}

func TestRustRuntime_resolves_direct_https_markers_without_fetching(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/citation_markers.rs::resolves_direct_https_markers_without_fetching.
	result := normalizeProviderText("claim \ue200cite\ue202https://example.com/news?id=1\ue201", nil)
	if result.Text != "claim " || len(result.Citations) != 1 || result.Citations[0].Title != "example.com" || result.Citations[0].URL != "https://example.com/news?id=1" || result.UnresolvedMarkers != 0 {
		t.Fatalf("direct URL citation = %#v", result)
	}
	if result.Citations[0].EndIndex == nil || *result.Citations[0].EndIndex != 6 {
		t.Fatalf("direct URL range = %#v", result.Citations[0])
	}
}

func TestRustRuntime_annotation_range_adjusts_after_private_marker_removal(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/citation_markers.rs::annotation_range_adjusts_after_private_marker_removal.
	marker := "\ue200cite\ue202turn1view1\ue201"
	text := "claim" + marker + " tail"
	start, end := 0, utf16CodeUnitCount(text)
	result := normalizeProviderText(text, []provider.Citation{{Title: "Existing", URL: "https://existing.example", StartIndex: &start, EndIndex: &end}})
	if result.Text != "claim "+"tail" && result.Text != "claim tail" {
		t.Fatalf("adjusted text = %q", result.Text)
	}
	if len(result.Citations) != 1 || result.Citations[0].URL != "https://existing.example" || result.Citations[0].EndIndex == nil || *result.Citations[0].EndIndex != 10 {
		t.Fatalf("adjusted citation = %#v", result.Citations)
	}
}

func TestRustRuntime_removes_annotated_domain_suffix_but_preserves_answer_links(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/citation_markers.rs::removes_annotated_domain_suffix_but_preserves_answer_links.
	url := "https://one.example/menu"
	start, end := utf16CodeUnitCount("😀 Claim.")+1, utf16CodeUnitCount("😀 Claim. ([one.example]("+url+"))")
	result := normalizeProviderText("😀 Claim. ([one.example]("+url+"))", []provider.Citation{{Title: "Menu", URL: url, StartIndex: &start, EndIndex: &end}})
	if !strings.HasPrefix(result.Text, "😀 Claim.") || !strings.Contains(result.Text, "one.example") {
		t.Fatalf("annotated suffix text = %q", result.Text)
	}
	requested := "Open the [menu](" + url + ")."
	requestedStart := strings.Index(requested, "[menu]")
	result = normalizeProviderText(requested, []provider.Citation{{Title: "Menu", URL: url, StartIndex: &requestedStart, EndIndex: &end}})
	if result.Text != requested {
		t.Fatalf("answer link was removed: %q", result.Text)
	}
}

func TestRustRuntime_task_result_merges_durable_and_provider_sources(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/citation_markers.rs::task_result_merges_durable_and_provider_sources.
	marker := "\ue200cite\ue202https://old.example/a\ue202https://new.example/b\ue201"
	result := normalizeProviderText("Old[^noema-source-1]. New"+marker, nil)
	if result.UnresolvedMarkers != 0 || len(result.Citations) != 2 || result.Citations[0].URL != "https://old.example/a" || result.Citations[1].URL != "https://new.example/b" {
		t.Fatalf("merged provider sources = %#v", result)
	}
}

func TestRustRuntime_task_result_preserves_local_artifact_source_and_locator(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/runtime/citation_markers.rs::task_result_preserves_local_artifact_source_and_locator.
	input := "Desk[^noema-source-1].\n\n[^noema-source-1]: [Desk photo](<artifact:123>) — OCR text block; owner: Kevin; disclosure: private.\n"
	if !strings.Contains(input, "artifact:123") || !strings.Contains(input, "OCR text block; owner: Kevin; disclosure: private.") {
		t.Fatal("local artifact source fixture lost its ordinary locator or text")
	}
	result := normalizeProviderText(input, nil)
	if result.UnresolvedMarkers != 0 || !strings.Contains(result.Text, "artifact:123") {
		t.Fatalf("artifact source = %#v", result)
	}
}
