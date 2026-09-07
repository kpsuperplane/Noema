package memory

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
	"testing"
)

// Port of crates/noema-memory/src/native_tests.rs:native_memory_initializes_root_and_search_index.
func TestRustNativeMemoryInitializesRootAndSearchIndex(t *testing.T) {
	store, _ := openMemoryTestStore(t)
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		Path: "people.md", Title: "People", Icon: "users",
		Body: "Alice recently completed several hikes and likes tea",
	}}}, State{}); err != nil {
		t.Fatal(err)
	}
	root, err := store.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	if len(root.Children) != 1 {
		t.Fatalf("root children = %#v", root.Children)
	}
	child := root.Children[0]
	if child.Path != "people.md" || child.Icon != "users" {
		t.Fatalf("child metadata = %#v", child)
	}
	if child.Excerpt != "Alice recently completed several hikes and likes tea" {
		t.Fatalf("child excerpt = %q", child.Excerpt)
	}
	content, err := store.root.ReadFile("people.md")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(content), "\nicon: users\n") {
		t.Fatalf("canonical child does not contain icon: %s", content)
	}
	for query := range map[string]bool{"Alice": true, "Alice \"": true, "recent hike completed": true} {
		results, searchErr := store.Search(query, 5)
		if searchErr != nil || len(results) == 0 || results[0].Path != "people.md" {
			t.Fatalf("search %q = %#v, %v", query, results, searchErr)
		}
	}
	if results, searchErr := store.Search("unrelated Alice", 5); searchErr != nil || len(results) != 0 {
		t.Fatalf("strict search = %#v, %v", results, searchErr)
	}
	if results, searchErr := store.SearchRelevant("unrelated conversation context Alice", 5); searchErr != nil || len(results) == 0 || results[0].Path != "people.md" {
		t.Fatalf("relevance search = %#v, %v", results, searchErr)
	}
	terms := make([]string, 0, 141)
	for index := 0; index < 140; index++ {
		terms = append(terms, fmt.Sprintf("term%d", index))
	}
	terms = append(terms, "Alice")
	if results, searchErr := store.SearchRelevant(strings.Join(terms, " "), 5); searchErr != nil || len(results) == 0 || results[0].Path != "people.md" {
		t.Fatalf("long relevance search = %#v, %v", results, searchErr)
	}
	page, err := store.ReadPage("people.md")
	if err != nil || page.Body != "Alice recently completed several hikes and likes tea" {
		t.Fatalf("page = %#v, %v", page, err)
	}
	byID, err := store.ReadPage(page.ID)
	if err != nil || byID.Path != "people.md" {
		t.Fatalf("page by stable ID = %#v, %v", byID, err)
	}
	if !strings.Contains(string(content), "schema: noema.memory.page/v2") || strings.Contains(string(content), "sources:") {
		t.Fatalf("stored page metadata = %s", content)
	}
}

// Port of crates/noema-memory/src/native_tests.rs:native_memory_rejects_unsafe_paths_and_oversized_bodies.
func TestRustNativeMemoryRejectsUnsafePathsAndOversizedBodies(t *testing.T) {
	for _, pagePath := range []string{"../root.md", ".pending/injected.md"} {
		if _, err := normalizePagePath(pagePath); err == nil {
			t.Fatalf("unsafe path %q was accepted", pagePath)
		}
	}
	if err := validatePageChange(PageChange{Path: "injected.md", Title: "Title\nowner: attacker", Icon: "file-text"}, "injected.md"); err == nil {
		t.Fatal("frontmatter injection was accepted")
	}
	if err := validatePageChange(PageChange{Path: "duplicate-title.md", Title: "Duplicate title", Icon: "file-text", Body: "# Duplicate title\n\nLead"}, "duplicate-title.md"); err == nil {
		t.Fatal("duplicate generated title was accepted")
	}
	if err := validatePageChange(PageChange{Path: RootPagePath, Title: "Kevin", Icon: "user", Body: strings.Repeat("Kevin has a durable preference for thoughtful technical systems. ", 24)}, RootPagePath); err == nil {
		t.Fatal("oversized root body was accepted")
	}
	if err := validatePageChange(PageChange{Path: "unsupported-icon.md", Title: "Unsupported icon", Icon: "not-a-lucide-icon"}, "unsupported-icon.md"); err == nil {
		t.Fatal("unsupported icon was accepted")
	}
}

// Port of crates/noema-memory/src/native_tests.rs:native_memory_recovers_exact_staged_bytes_before_indexing.
func TestRustNativeMemoryRecoversExactStagedBytesBeforeIndexing(t *testing.T) {
	store, root := openMemoryTestStore(t)
	change := PageChange{Path: "people.md", Title: "People", Icon: "users", Body: "Alice"}
	bytes, err := renderNewPage(change, "people.md", "", "")
	if err != nil {
		t.Fatal(err)
	}
	pendingDir := ".pending/recovery"
	if err := ensureRootDirectory(store.root, pendingDir); err != nil {
		t.Fatal(err)
	}
	payload := pendingPublication{
		Pages: []stagedPage{{Path: "people.md", Bytes: bytes}},
		State: State{UpdatedAt: "1"},
	}
	if err := writeRootFile(store.root, pendingDir+"/changes.json", mustJSON(payload)); err != nil {
		t.Fatal(err)
	}
	if err := store.Close(); err != nil {
		t.Fatal(err)
	}
	recovered, err := New(root)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = recovered.Close() })
	got, err := recovered.root.ReadFile("people.md")
	if err != nil || string(got) != string(bytes) {
		t.Fatalf("recovered bytes = %q, %v", got, err)
	}
	results, err := recovered.Search("Alice", 5)
	if err != nil || len(results) == 0 || results[0].Path != "people.md" {
		t.Fatalf("recovered search = %#v, %v", results, err)
	}
	if _, err := root.Stat("memory/human/.pending/recovery"); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("pending recovery remains: %v", err)
	}
}

func mustJSON(value any) []byte {
	data, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return data
}

// Port of crates/noema-memory/src/native_tests.rs:native_memory_enforces_compare_publish_citations_and_rendered_word_limit.
func TestRustNativeMemoryEnforcesComparePublishCitationsAndRenderedWordLimit(t *testing.T) {
	store, _ := openMemoryTestStore(t)
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		Path: "people.md", Title: "People", Icon: "users", Body: "Alice [^1]",
		Citations: []Citation{{Sources: []string{"item-1", "item-2"}}},
	}}}, State{}); err != nil {
		t.Fatal(err)
	}
	current, err := store.ReadPage("people.md")
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		ID: current.ID, ExpectedHash: current.Hash, Path: "people.md", Title: "People", Icon: "users", Body: "Updated [^1]",
		Citations: []Citation{{Sources: []string{"item-1", "item-2"}}},
	}}}, State{}); err != nil {
		t.Fatal(err)
	}
	updated, err := store.ReadPage("people.md")
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		ID: updated.ID, ExpectedHash: current.Hash, Path: "people.md", Title: "People", Icon: "users", Body: "Stale [^1]",
		Citations: []Citation{{Sources: []string{"item-1"}}},
	}}}, State{}); err == nil {
		t.Fatal("stale compare-and-publish was accepted")
	}
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		ID: updated.ID, ExpectedHash: updated.Hash, Path: "people.md", Title: "People", Icon: "users", Body: "Missing citation",
		Citations: []Citation{{Sources: []string{"item-1"}}},
	}}}, State{}); err == nil {
		t.Fatal("missing citation was accepted")
	}
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		Path: "too-long.md", Title: "Title", Icon: "file-text", Body: strings.Repeat("word ", MaxWords),
	}}}, State{}); err == nil {
		t.Fatal("rendered word limit was not enforced")
	}
}

// Port of crates/noema-memory/src/native_tests.rs:native_memory_validates_final_hierarchy_and_preserves_ids_across_moves.
func TestRustNativeMemoryValidatesFinalHierarchyAndPreservesIDsAcrossMoves(t *testing.T) {
	store, _ := openMemoryTestStore(t)
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{Path: "people/alice.md", Title: "Alice", Icon: "user"}}}, State{}); err == nil {
		t.Fatal("orphan child was accepted")
	}
	if err := store.Publish(ChangeSet{Upserts: []PageChange{
		{Path: "people.md", Title: "People", Icon: "users"},
		{Path: "people/alice.md", Title: "Alice", Icon: "user"},
		{Path: "people/alice/preferences.md", Title: "Preferences", Icon: "sparkles"},
	}}, State{}); err != nil {
		t.Fatal(err)
	}
	deep, err := store.ReadPage("people/alice/preferences.md")
	if err != nil {
		t.Fatal(err)
	}
	if len(deep.Ancestors) != 2 || deep.Ancestors[0].Path != "people.md" || deep.Ancestors[1].Path != "people/alice.md" {
		t.Fatalf("deep ancestors = %#v", deep.Ancestors)
	}
	if err := store.Publish(ChangeSet{Deletes: []string{"people.md"}}, State{}); err == nil {
		t.Fatal("parent deletion was accepted")
	}
	alice, err := store.ReadPage("people/alice.md")
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Publish(ChangeSet{
		Upserts: []PageChange{{ID: alice.ID, ExpectedHash: alice.Hash, Path: "people/alicia.md", Title: "Alicia", Icon: "user"}},
		Deletes: []string{"people/alice/preferences.md"},
	}, State{}); err != nil {
		t.Fatal(err)
	}
	if _, err := store.ReadPage("people/alice.md"); !errors.Is(err, ErrPageNotFound) {
		t.Fatalf("old page error = %v", err)
	}
	moved, err := store.ReadPage("people/alicia.md")
	if err != nil || moved.ID != alice.ID {
		t.Fatalf("moved page = %#v, %v", moved, err)
	}
}
