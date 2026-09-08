package webtool

import (
	"os"
	"strings"
	"testing"
)

func TestSummaryPromptMatchesRustExactly(t *testing.T) {
	// Extracted from Rust 4d29f6ba, noema-capabilities/src/web/fetch.rs.
	raw, err := os.ReadFile("testdata/rust_summary_prompt.txt")
	if err != nil {
		t.Fatal(err)
	}
	want := strings.NewReplacer("{url}", "https://example.com", "{title}", "Example", "{max_chars}", "4000", "{markdown}", "Source text").Replace(string(raw))
	if got := SummaryPrompt("https://example.com", "Example", "Source text", 4000); got != want {
		t.Fatalf("summary prompt differs from Rust: %q", got)
	}
	if got := SummaryPrompt("https://example.com", "", "Source text", 4000); got != strings.Replace(want, "Source title: Example", "Source title: ", 1) {
		t.Fatal("missing source title differs from Rust None")
	}

}
