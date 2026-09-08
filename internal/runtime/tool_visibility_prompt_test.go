package runtime

import (
	"github.com/kpsuperplane/noema/internal/provider"
	"os"
	"strings"
	"testing"
)

// Expected outputs were rendered from Rust 4d29f6ba model_context.rs literals.
func TestToolVisibilityExactRustPrompt(t *testing.T) {
	tools := []provider.GenerationTool{{Name: "search_memory", Description: "Search memory"}, {Name: "read_memory_page", Description: "Read memory"}, {Name: "web.browse.open", Description: "Open browser"}}
	for _, transport := range []provider.ToolTransport{provider.ToolTransportNone, provider.ToolTransportNative} {
		t.Run(string(transport), func(t *testing.T) {
			expected, err := os.ReadFile("testdata/rust_prompts/tool_visibility_" + string(transport) + ".txt")
			if err != nil {
				t.Fatal(err)
			}
			got := toolVisibilityMessage(tools, transport, true)
			if got.Role != "developer" || got.Content != string(expected) {
				t.Fatalf("tool visibility differs from Rust:\n%s", got.Content)
			}
		})
	}
}

func TestToolVisibilityPreservesServiceOwnership(t *testing.T) {
	row := "- service\tconnection:docs\tname=\"Docs\""
	tools := []provider.GenerationTool{{Name: "docs.read", Description: "Read", ServiceCatalogRow: row, ServiceConnectionID: "connection:docs"}, {Name: "docs.search", Description: "Search", ServiceCatalogRow: row, ServiceConnectionID: "connection:docs"}}
	message := toolVisibilityMessage(tools, provider.ToolTransportNative, false)
	content := modelContextSectionContent(t, []provider.GenerationMessage{message}, "tools.visibility")
	if strings.Count(content, row) != 1 || !strings.Contains(content, "- capability\tdocs.read\tservice=connection:docs\tRead") || !strings.Contains(content, "- capability\tdocs.search\tservice=connection:docs\tSearch") {
		t.Fatalf("service ownership differs from Rust: %s", content)
	}
}
