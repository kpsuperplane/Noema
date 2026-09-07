package store

import (
	"errors"
	"reflect"
	"testing"
)

// Rust source: crates/noema-conversations/src/records.rs:175::local_chat_constructor_preserves_default_wire_values_and_requested_provider
func TestRustConversations_LocalChatConstructorPreservesDefaultWireValuesAndRequestedProvider(t *testing.T) {
	model := "gpt-test"
	cwd := "/tmp/noema"
	conversation := NewLocalConversation(&model, &cwd)
	if conversation.Title != nil {
		t.Fatalf("title = %q, want nil", *conversation.Title)
	}
	if got, want := conversation.Owner.HumanID, "human:local"; got != want {
		t.Fatalf("owner human id = %q, want %q", got, want)
	}
	if conversation.PrimaryHumanID == nil || *conversation.PrimaryHumanID != "human:local" {
		t.Fatalf("primary human id = %v, want %q", conversation.PrimaryHumanID, "human:local")
	}
	if conversation.PrimaryAgentID == nil || *conversation.PrimaryAgentID != "agent:primary" {
		t.Fatalf("primary agent id = %v, want %q", conversation.PrimaryAgentID, "agent:primary")
	}
	if got, want := conversation.Provider, "codex"; got != want {
		t.Fatalf("provider = %q, want %q", got, want)
	}
	if conversation.Model == nil || *conversation.Model != model {
		t.Fatalf("model = %v, want %q", conversation.Model, model)
	}
	if conversation.CWD == nil || *conversation.CWD != cwd {
		t.Fatalf("cwd = %v, want %q", conversation.CWD, cwd)
	}
	if got, want := conversation.Metadata, map[string]any{}; !reflect.DeepEqual(got, want) {
		t.Fatalf("metadata = %#v, want %#v", got, want)
	}

	local := NewLocalConversationForProvider("local_models", nil, nil)
	if got, want := local.Provider, "local_models"; got != want {
		t.Fatalf("local provider = %q, want %q", got, want)
	}
	if got, want := local.Owner.HumanID, "human:local"; got != want {
		t.Fatalf("local owner human id = %q, want %q", got, want)
	}
}

// Rust source: crates/noema-conversations/src/references.rs:57::references_preserve_wire_values_valid_ids_and_validation_boundaries
func TestRustConversations_ReferencesPreserveWireValuesValidIDsAndValidationBoundaries(t *testing.T) {
	for _, value := range []string{"", "   ", "\t\n"} {
		_, err := NewActorRef(value)
		assertRustConversationError(t, err, ConversationErrorEmptyReferenceID, "", "", "actor")

		_, err = NewConversationOwnerRef(value)
		assertRustConversationError(t, err, ConversationErrorEmptyReferenceID, "", "", "conversation_owner")
	}
}

// Rust source: crates/noema-conversations/src/status.rs:156::every_status_vocabulary_round_trips_every_wire_value_and_rejects_unknowns
func TestRustConversations_EveryStatusVocabularyRoundTripsEveryWireValueAndRejectsUnknowns(t *testing.T) {
	assertRustConversationVocabulary(t, AllAgentStatuses(), AgentStatus.String, ParseAgentStatus, "agent_status")
	assertRustConversationVocabulary(t, AllConversationTurnStatuses(), ConversationTurnStatus.String, ParseConversationTurnStatus, "conversation_turn_status")
	assertRustConversationVocabulary(t, AllConversationItemKinds(), ConversationItemKind.String, ParseConversationItemKind, "conversation_item_kind")
	assertRustConversationVocabulary(t, AllConversationItemStatuses(), ConversationItemStatus.String, ParseConversationItemStatus, "conversation_item_status")
	assertRustConversationVocabulary(t, AllConversationContextSummaryStatuses(), ConversationContextSummaryStatus.String, ParseConversationContextSummaryStatus, "conversation_context_summary_status")
}

func assertRustConversationVocabulary[T comparable](t *testing.T, values []T, asString func(T) string, parse func(string) (T, error), kind string) {
	t.Helper()
	for _, value := range values {
		got, err := parse(asString(value))
		if err != nil {
			t.Fatalf("parse(%q) returned error: %v", asString(value), err)
		}
		if got != value {
			t.Fatalf("parse(%q) = %v, want %v", asString(value), got, value)
		}
	}
	_, err := parse("unknown")
	assertRustConversationError(t, err, ConversationErrorInvalidEnum, kind, "unknown", "")
}

func assertRustConversationError(t *testing.T, err error, code ConversationErrorCode, kind, value, referenceKind string) {
	t.Helper()
	if err == nil {
		t.Fatalf("expected conversation error %q", code)
	}
	var conversationErr *ConversationError
	if !errors.As(err, &conversationErr) {
		t.Fatalf("error = %T %v, want *ConversationError", err, err)
	}
	want := &ConversationError{Code: code, Kind: kind, Value: value, ReferenceKind: referenceKind}
	if !reflect.DeepEqual(conversationErr, want) {
		t.Fatalf("conversation error = %#v, want %#v", conversationErr, want)
	}
}
