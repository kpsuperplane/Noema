package runtime

import (
	"github.com/kpsuperplane/noema/internal/provider"
	"os"
	"strings"
	"testing"
)

// Keep the Rust instructions except for the deferred access authority.
func TestToolVisibilityPreservesRustInstructionsWithDeferredAuthority(t *testing.T) {
	tools := []provider.GenerationTool{{Name: "search_memory", Description: "Search memory"}, {Name: "read_memory_page", Description: "Read memory"}, {Name: "web.browse.open", Description: "Open browser"}}
	for _, transport := range []provider.ToolTransport{provider.ToolTransportNone, provider.ToolTransportNative} {
		t.Run(string(transport), func(t *testing.T) {
			expected, err := os.ReadFile("testdata/rust_prompts/tool_visibility_" + string(transport) + ".txt")
			if err != nil {
				t.Fatal(err)
			}
			got := toolVisibilityMessage(tools, transport, true)
			want := modelContextSectionContent(t, []provider.GenerationMessage{{Role: "developer", Content: string(expected)}}, "tools.visibility")
			want = strings.Replace(want, "require a callable tool owned by that exact service.", "require a current tool catalog entry or tools.load directory entry owned by that exact service. Deferred definitions must be loaded before use.", 1)
			for _, tool := range tools {
				want = strings.ReplaceAll(want, "\t"+tool.Description, "")
			}
			if got.Role != "developer" || modelContextSectionContent(t, []provider.GenerationMessage{got}, "tools.visibility") != want {
				t.Fatalf("tool visibility differs from Rust:\n%s", got.Content)
			}
		})
	}
}

func TestToolVisibilityPreservesServiceOwnership(t *testing.T) {
	row := "- service\tconnection:docs\tname=\"Docs\""
	tools := []provider.GenerationTool{{Name: "docs.read", Description: "Read", ServiceCatalogRow: row, ServiceConnectionID: "connection:docs"}, {Name: "docs.search", Deferred: true, Description: "Search", ServiceCatalogRow: row, ServiceConnectionID: "connection:docs"}}
	message := toolVisibilityMessage(tools, provider.ToolTransportNative, false)
	content := modelContextSectionContent(t, []provider.GenerationMessage{message}, "tools.visibility")
	if strings.Count(content, row) != 1 || !strings.Contains(content, `callable_tool_names: ["docs.read","tool_search"]`) || !strings.Contains(content, "- capability\tdocs.read\tservice=connection:docs\n") || !strings.Contains(content, "- deferred_tool\tdocs.search\tservice=connection:docs\n") {
		t.Fatalf("service ownership differs from Rust: %s", content)
	}
}

func TestDeferredAccessInstructionsAppearOnlyWithNativeDeferredTools(t *testing.T) {
	tools := []provider.GenerationTool{{Name: "docs.read", Deferred: true, ServiceConnectionID: "connection:docs", ServiceCatalogRow: "- service\tconnection:docs\tname=\"Docs\""}}
	for _, transport := range []provider.ToolTransport{provider.ToolTransportNative, provider.ToolTransportNone} {
		message := toolVisibilityMessage(tools, transport, false)
		content := modelContextSectionContent(t, []provider.GenerationMessage{message}, "tools.visibility")
		want := transport == provider.ToolTransportNative
		if strings.Contains(content, "A deferred_tool entry is current access evidence for its owning service.") != want || strings.Contains(content, "- provider_native\ttool_search\t") != want {
			t.Fatalf("incorrect deferred access instructions: %s", content)
		}
	}
	rows := taskToolPromptRows(tools, nil, nil)
	if !strings.Contains(rows, "- provider_native\ttool_search\t") || !strings.Contains(rows, "Its absence from callable_tool_names does not mean that the service is disconnected.") {
		t.Fatalf("Task lost the deferred access instructions: %s", rows)
	}
	tools[0].Deferred = false
	if strings.Contains(taskToolPromptRows(tools, nil, nil), "tool_search") {
		t.Fatal("eager Task catalog advertised tool search")
	}
}
