package graphql

import (
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestPendingAdapterNamesDoNotLoadConnectorAuthority(t *testing.T) {
	r := readyAgentTestResolver(t)
	r.Adapters = nil // Display reads must not need the execution service, even with pending requests.
	ctx, now := t.Context(), time.Now()
	empty, err := r.pendingAdapterAuthentications(ctx, nil, nil, 50)
	if err != nil || len(empty) != 0 {
		t.Fatalf("empty requests = %v, %v", empty, err)
	}
	conversation, err := r.Store.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	for _, kind := range []string{"adapter_connection", "adapter_grant"} {
		now = now.Add(time.Millisecond)
		turn, _, err := r.Store.BeginConversationTurn(ctx, conversation.ID, "Read my calendar", nil, now)
		if err != nil {
			t.Fatal(err)
		}
		items, err := r.Store.StartConversationToolRound(ctx, turn, store.ConversationToolRound{Provider: "openrouter", Call: store.ConversationToolCallInput{ProviderCallID: kind, ProviderName: "calendar.read", Name: "calendar.read", Arguments: json.RawMessage(`{}`)}}, now)
		if err != nil {
			t.Fatal(err)
		}
		value := store.MCPAuthRequest{OwnerHumanID: localHumanID, ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: items[len(items)-1].ID, AuthorityKind: kind, AuthorityID: strings.Repeat("a", 32), CapabilityName: "calendar.read", BindingJSON: `{}`, ArgumentsJSON: `{}`}
		if kind == "adapter_grant" {
			value.AuthorityID = strings.Repeat("b", 32)
			value.AdapterConnectionID = strings.Repeat("a", 32)
			value.AdapterSemanticDigest = strings.Repeat("c", 64)
			value.AdapterAuthorityRevision = 1
		}
		if _, _, err := r.Store.CreateMCPAuthRequest(ctx, value, now); err != nil {
			t.Fatal(err)
		}
	}
	for _, name := range []string{"", "Calendar 日本語", "Updated calendar"} {
		definitions := []store.AdapterDefinitionIndex{{Digest: strings.Repeat("c", 64), DefinitionID: "definition:calendar", AdapterID: "calendar", DefinitionRevision: "1", SourceReference: "https://example.com/docs", OperationCount: 1, DisplayName: name}}
		connections := []store.AdapterConnectionIndex{{ID: strings.Repeat("a", 32), Slug: "calendar", Digest: definitions[0].Digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"read"}}}
		if err := r.Store.ReconcileAdapters(ctx, definitions, connections, now); err != nil {
			t.Fatal(err)
		}
		values, err := r.pendingAdapterAuthentications(ctx, &conversation.ID, nil, 50)
		if err != nil || len(values) != 2 {
			t.Fatalf("pending = %v, %v", values, err)
		}
		for i, value := range values {
			want := name
			if want == "" {
				want = strings.Repeat(string(rune('a'+i)), 32)
			}
			if got := value.(*model.AdapterAuthenticationIntervention).ServiceDisplayName; got != want {
				t.Fatalf("name = %q, want %q", got, want)
			}
		}
	}
	other := "task:unrelated"
	if values, err := r.pendingAdapterAuthentications(ctx, nil, &other, 50); err != nil || len(values) != 0 {
		t.Fatalf("task filter = %v, %v", values, err)
	}
	if values, err := r.pendingAdapterAuthentications(ctx, &other, nil, 50); err != nil || len(values) != 0 {
		t.Fatalf("conversation filter = %v, %v", values, err)
	}
	if values, err := r.pendingAdapterAuthentications(ctx, nil, nil, 1); err != nil || len(values) != 1 {
		t.Fatalf("limit = %v, %v", values, err)
	}
}

func TestTaskWithoutAttentionDoesNotReadTaskFiles(t *testing.T) {
	r := readyAgentTestResolver(t)
	task, err := r.captureTask(t.Context(), model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Read task", TaskDocument: "Read task", ClientMutationID: "read-task"})
	if err != nil {
		t.Fatal(err)
	}
	if err := r.home.Close(); err != nil {
		t.Fatal(err)
	}
	values, err := r.pendingHumanInterventions(t.Context(), nil, &task.Task.TaskID, nil, nil)
	if err != nil || len(values) != 0 {
		t.Fatalf("attention without file access = %v, %v", values, err)
	}
}
