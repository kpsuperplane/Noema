package store

import (
	"encoding/json"
	"strings"
	"testing"
	"time"
)

// Rust source: crates/noema-capabilities/src/authentication/tests.rs::challenge_round_trips_only_bounded_non_secret_identity.
func TestRustCapabilities_challenge_round_trips_only_bounded_non_secret_identity(t *testing.T) {
	database := openTestStore(t)
	serverID := createRustCapabilityMCPServer(t, database)
	request := createRustCapabilityAuthRequest(t, database, MCPAuthRequest{
		AuthorityKind: "mcp_server", AuthorityID: serverID, ServerID: serverID,
		CapabilityName: "mcp.docs.read", BindingJSON: `{"authority_revision":"authority:7"}`,
		ArgumentsJSON: `{"ordinary":"value"}`,
	})

	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	var roundTrip MCPAuthRequest
	if err := json.Unmarshal(encoded, &roundTrip); err != nil {
		t.Fatal(err)
	}
	if roundTrip.AuthorityKind != request.AuthorityKind || roundTrip.AuthorityID != serverID ||
		roundTrip.ServerID != serverID || roundTrip.CapabilityName != "mcp.docs.read" ||
		roundTrip.BindingJSON != request.BindingJSON || roundTrip.ArgumentsJSON != request.ArgumentsJSON {
		t.Fatalf("authentication identity changed: %#v", roundTrip)
	}
	if strings.Contains(string(encoded), "secret") {
		t.Fatalf("secret entered authentication request: %s", encoded)
	}

	persisted, err := database.MCPAuthRequest(t.Context(), request.ID, request.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if persisted.AuthorityKind != "mcp_server" || persisted.AuthorityID != serverID ||
		persisted.CapabilityName != "mcp.docs.read" || persisted.BindingJSON != `{"authority_revision":"authority:7"}` ||
		persisted.ArgumentsJSON != `{"ordinary":"value"}` {
		t.Fatalf("persisted authentication identity changed: %#v", persisted)
	}
}

// Rust source: crates/noema-capabilities/src/authentication/tests.rs::challenge_rejects_display_text_controls_and_oversize_values.
func TestRustCapabilities_challenge_rejects_display_text_controls_and_oversize_values(t *testing.T) {
	database := openTestStore(t)
	for _, authorityID := range []string{"", "display label", "line\nbreak", strings.Repeat("a", 513)} {
		_, _, err := database.CreateMCPAuthRequest(t.Context(), MCPAuthRequest{
			OwnerHumanID: "human:local", AuthorityKind: "mcp_server", AuthorityID: authorityID,
			CapabilityName: "mcp.docs.read", BindingJSON: `{}`, ArgumentsJSON: `{}`,
		}, time.Now().UTC())
		if err == nil || err.Error() != "MCP authentication request is invalid" {
			t.Errorf("authority ID %q error = %v", authorityID, err)
		}
	}
}

// Rust source: crates/noema-capabilities/src/policy.rs::destination_rejects_display_text_and_control_characters.
func TestRustCapabilities_destination_rejects_display_text_and_control_characters(t *testing.T) {
	database := openTestStore(t)
	for _, authorityID := range []string{"personal account", "personal\nother"} {
		_, _, err := database.CreateMCPAuthRequest(t.Context(), MCPAuthRequest{
			OwnerHumanID: "human:local", AuthorityKind: "adapter_connection", AuthorityID: authorityID,
			CapabilityName: "adapter.calendar", BindingJSON: `{}`, ArgumentsJSON: `{}`,
		}, time.Now().UTC())
		if err == nil || err.Error() != "MCP authentication request is invalid" {
			t.Errorf("destination identity %q error = %v", authorityID, err)
		}
	}
}

// Rust source: crates/noema-capabilities/src/policy.rs::destination_serializes_only_stable_non_secret_identity.
func TestRustCapabilities_destination_serializes_only_stable_non_secret_identity(t *testing.T) {
	database := openTestStore(t)
	request := createRustCapabilityAuthRequest(t, database, MCPAuthRequest{
		AuthorityKind: "adapter_connection", AuthorityID: strings.Repeat("c", 32),
		CapabilityName: "adapter.calendar",
		BindingJSON:    `{"service_id":"adapter:calendar","connection_id":"connection:personal","account_id":"account:synthetic","revision":"definition:7/credential:2"}`,
		ArgumentsJSON:  `{"ordinary":"value"}`,
	})
	persisted, err := database.MCPAuthRequest(t.Context(), request.ID, request.Revision)
	if err != nil {
		t.Fatal(err)
	}
	var destination map[string]any
	if err := json.Unmarshal([]byte(persisted.BindingJSON), &destination); err != nil {
		t.Fatal(err)
	}
	want := map[string]any{
		"service_id": "adapter:calendar", "connection_id": "connection:personal",
		"account_id": "account:synthetic", "revision": "definition:7/credential:2",
	}
	if len(destination) != len(want) {
		t.Fatalf("destination fields = %#v", destination)
	}
	for key, expected := range want {
		if destination[key] != expected {
			t.Fatalf("destination[%q] = %#v, want %#v", key, destination[key], expected)
		}
	}
	encoded, _ := json.Marshal(destination)
	for _, secret := range []string{"access_token", "refresh_token", "client_secret", "secret"} {
		if strings.Contains(string(encoded), secret) {
			t.Fatalf("secret field %q entered destination serialization: %s", secret, encoded)
		}
	}
	if persisted.AuthorityID != strings.Repeat("c", 32) || persisted.AuthorityKind != "adapter_connection" {
		t.Fatalf("stable connection identity changed: %#v", persisted)
	}
}

func createRustCapabilityMCPServer(t *testing.T, database *Store) string {
	t.Helper()
	serverID := "mcp_server:" + strings.Repeat("d", 32)
	_, err := database.CommitMCPConnection(t.Context(), NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("e", 32), Revision: "mcp_definition_revision:" + strings.Repeat("f", 32),
			DisplayName: "Capabilities", TransportKind: "streamable_http", SafeConfig: json.RawMessage(`{"url":"https://example.test"}`)},
		ServerID: serverID, ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("a", 32), AuthStatus: "none",
	}, time.Now().UTC())
	if err != nil {
		t.Fatal(err)
	}
	return serverID
}

func createRustCapabilityAuthRequest(t *testing.T, database *Store, value MCPAuthRequest) MCPAuthRequest {
	t.Helper()
	now := time.Now().UTC()
	conversation, err := database.EnsurePrimaryConversation(t.Context(), "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(t.Context(), conversation.ID, "Use the configured capability.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(t.Context(), turn, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{ProviderCallID: "call-capability", ProviderName: value.CapabilityName,
			Name: value.CapabilityName, Arguments: json.RawMessage(value.ArgumentsJSON)},
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	value.OwnerHumanID = "human:local"
	value.ConversationID, value.TurnID, value.CallItemID = conversation.ID, turn.ID, items[len(items)-1].ID
	request, _, err := database.CreateMCPAuthRequest(t.Context(), value, now)
	if err != nil {
		t.Fatal(err)
	}
	return request
}
