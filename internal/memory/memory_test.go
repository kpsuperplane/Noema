package memory

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"runtime"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/home"
)

func TestMemoryPublishesHierarchyCitationsAndStableMove(t *testing.T) {
	store, _ := openMemoryTestStore(t)
	root, err := store.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	if info, statErr := store.root.Stat(RootPagePath); statErr != nil || runtime.GOOS != "windows" && info.Mode().Perm() != 0o600 {
		t.Fatalf("root page protection = %#v, %v", info, statErr)
	}
	state := State{
		ConversationID: "conversation:test", LastConsolidatedSequence: 4,
		LastConsolidatedItem: "item:four", UpdatedAt: "2026-09-04T12:00:00Z",
	}
	err = store.Publish(ChangeSet{Upserts: []PageChange{
		{ID: root.ID, ExpectedHash: root.Hash, Path: RootPagePath, Title: "Human memory", Icon: "user", Body: "A compact overview."},
		{Path: "people.md", Title: "People", Icon: "users", Body: "Important people."},
		{Path: "people/alice.md", Title: "Alice", Icon: "user", Body: "Alice likes tea.[^1]", Citations: []Citation{{Sources: []string{"item:one"}}}},
	}}, state)
	if err != nil {
		t.Fatal(err)
	}
	alice, err := store.ReadPage("people/alice.md")
	if err != nil {
		t.Fatal(err)
	}
	if alice.Parent != "people.md" || len(alice.Ancestors) != 1 || alice.Ancestors[0].Path != "people.md" {
		t.Fatalf("Alice hierarchy = %#v", alice)
	}
	if len(alice.Citations) != 1 || alice.Citations[0].Sources[0] != "item:one" {
		t.Fatalf("Alice citations = %#v", alice.Citations)
	}
	if Reference(alice).Excerpt != "Alice likes tea." {
		t.Fatalf("Alice excerpt = %q", Reference(alice).Excerpt)
	}
	originalID := alice.ID
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		ID: alice.ID, ExpectedHash: alice.Hash, Path: "people/alicia.md",
		Title: "Alicia", Icon: "user", Body: "Alicia likes tea.",
	}}}, state); err != nil {
		t.Fatal(err)
	}
	moved, err := store.ReadPage(originalID)
	if err != nil || moved.Path != "people/alicia.md" || moved.ID != originalID {
		t.Fatalf("moved page = %#v, %v", moved, err)
	}
	if _, err := store.ReadPage("people/alice.md"); !errors.Is(err, ErrPageNotFound) {
		t.Fatalf("old page error = %v", err)
	}
	checkpoint, err := store.State()
	if err != nil || checkpoint != state {
		t.Fatalf("checkpoint = %#v, %v", checkpoint, err)
	}
}

func TestMemorySearchIsStrictBoundedAndRebuilt(t *testing.T) {
	store, _ := openMemoryTestStore(t)
	changes := []PageChange{{
		Path: "people.md", Title: "People", Icon: "users",
		Body: "Alice recently completed several hikes and likes tea.",
	}}
	for index := range 17 {
		changes = append(changes, PageChange{
			Path: fmt.Sprintf("topic-%02d.md", index), Title: fmt.Sprintf("Topic %02d", index),
			Icon: "file-text", Body: "Shared reference material.",
		})
	}
	if err := store.Publish(ChangeSet{Upserts: changes}, State{}); err != nil {
		t.Fatal(err)
	}
	for query, want := range map[string]string{
		"Alice":                 "people.md",
		"Alice \"":              "people.md",
		"recent hike completed": "people.md",
	} {
		results, err := store.Search(query, 5)
		if err != nil || len(results) != 1 || results[0].Path != want || !strings.Contains(results[0].Snippet, "Alice") {
			t.Fatalf("search %q = %#v, %v", query, results, err)
		}
	}
	for _, query := range []string{"", "\"", "unrelated Alice"} {
		results, err := store.Search(query, 5)
		if err != nil || len(results) != 0 {
			t.Fatalf("empty or strict search %q = %#v, %v", query, results, err)
		}
	}
	bounded, err := store.Search("shared", 100)
	if err != nil || len(bounded) != maxSearchResults || bounded[0].Path != "topic-00.md" {
		t.Fatalf("bounded search = %#v, %v", bounded, err)
	}
	people, err := store.ReadPage("people.md")
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		ID: people.ID, ExpectedHash: people.Hash, Path: people.Path,
		Title: people.Title, Icon: people.Icon, Body: "Bob likes trains.",
	}}}, State{}); err != nil {
		t.Fatal(err)
	}
	oldResults, _ := store.Search("Alice", 5)
	newResults, _ := store.Search("Bob", 5)
	if len(oldResults) != 0 || len(newResults) != 1 || newResults[0].Path != "people.md" {
		t.Fatalf("rebuilt search = old %#v, new %#v", oldResults, newResults)
	}
}

func TestMemoryRejectsUnsafeStaleAndOversizedChanges(t *testing.T) {
	store, _ := openMemoryTestStore(t)
	root, err := store.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	boundedBody := strings.TrimSpace(strings.Repeat("界 ", MaxWords-1))
	if unicodeWordCount("# Bounded\n\n"+boundedBody) != MaxWords {
		t.Fatal("Unicode boundary fixture is invalid")
	}
	if err := store.Publish(ChangeSet{Upserts: []PageChange{{
		Path: "bounded.md", Title: "Bounded", Icon: "file-text", Body: boundedBody,
	}}}, State{}); err != nil {
		t.Fatalf("publish exact word boundary: %v", err)
	}
	cases := []ChangeSet{
		{Upserts: []PageChange{{Path: "../outside.md", Title: "Outside", Icon: "file-text"}}},
		{Upserts: []PageChange{{Path: "missing/child.md", Title: "Child", Icon: "file-text"}}},
		{Upserts: []PageChange{{ID: root.ID, ExpectedHash: "stale", Path: RootPagePath, Title: root.Title, Icon: root.Icon}}},
		{Upserts: []PageChange{{Path: "large.md", Title: "Large", Icon: "file-text", Body: strings.Repeat("界 ", MaxWords)}}},
	}
	for index, changes := range cases {
		if err := store.Publish(changes, State{}); !errors.Is(err, ErrInvalidPage) {
			t.Fatalf("invalid change %d error = %v", index, err)
		}
	}
	after, err := store.ReadRoot()
	if err != nil || after.Hash != root.Hash {
		t.Fatalf("root changed after rejection = %#v, %v", after, err)
	}
}

func TestMemoryRecoversStagedPublication(t *testing.T) {
	store, root := openMemoryTestStore(t)
	current, err := store.ReadRoot()
	if err != nil {
		t.Fatal(err)
	}
	updatedAt := "2026-09-04T13:00:00Z"
	rendered, err := renderNewPage(PageChange{
		ID: current.ID, ExpectedHash: current.Hash, Path: RootPagePath,
		Title: "Recovered memory", Icon: "brain", Body: "Recovered exactly.",
	}, RootPagePath, current.ID, updatedAt, updatedAt)
	if err != nil {
		t.Fatal(err)
	}
	pending := pendingPublication{
		Pages: []stagedPage{{Path: RootPagePath, Bytes: rendered}},
		State: State{LastConsolidatedSequence: 7, UpdatedAt: updatedAt},
	}
	payload, err := json.Marshal(pending)
	if err != nil {
		t.Fatal(err)
	}
	if err := ensureRootDirectory(store.root, ".pending/recover"); err != nil {
		t.Fatal(err)
	}
	if err := writeRootFile(store.root, ".pending/recover/changes.json", payload); err != nil {
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
	page, err := recovered.ReadRoot()
	if err != nil || page.Title != "Recovered memory" || page.Hash != sha256Bytes(rendered) {
		t.Fatalf("recovered page = %#v, %v", page, err)
	}
	if _, err := root.Stat("memory/human/.pending/recover"); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("pending stage remains: %v", err)
	}
}

func openMemoryTestStore(t *testing.T) (*Store, *os.Root) {
	t.Helper()
	paths, err := home.FromRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root, err := paths.Open()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	store, err := New(root)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = store.Close() })
	return store, root
}
