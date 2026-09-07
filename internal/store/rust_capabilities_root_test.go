package store

import (
	"encoding/json"
	"strings"
	"testing"
)

// Rust source: crates/noema-capabilities/src/authentication/tests.rs::challenge_round_trips_only_bounded_non_secret_identity.
func TestRustCapabilities_challenge_round_trips_only_bounded_non_secret_identity(t *testing.T) {
	request := MCPAuthRequest{AuthorityKind: "mcp_server", AuthorityID: "mcp:docs", CapabilityName: "mcp.docs.read",
		BindingJSON: `{"authority_revision":"authority:7"}`, ArgumentsJSON: `{"ordinary":"value"}`}
	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	var roundTrip MCPAuthRequest
	if err := json.Unmarshal(encoded, &roundTrip); err != nil {
		t.Fatal(err)
	}
	if roundTrip.AuthorityID != "mcp:docs" || roundTrip.CapabilityName != "mcp.docs.read" ||
		roundTrip.BindingJSON != request.BindingJSON || roundTrip.ArgumentsJSON != request.ArgumentsJSON {
		t.Fatalf("authentication identity changed: %#v", roundTrip)
	}
	if strings.Contains(string(encoded), "secret") {
		t.Fatalf("secret entered authentication request: %s", encoded)
	}
}

// Rust source: crates/noema-capabilities/src/authentication/tests.rs::challenge_rejects_display_text_controls_and_oversize_values.
func TestRustCapabilities_challenge_rejects_display_text_controls_and_oversize_values(t *testing.T) {
	for _, authorityID := range []string{"", "display label", "line\nbreak", strings.Repeat("a", 513)} {
		if validAuthorityIdentity(authorityID) {
			t.Errorf("authority ID %q was accepted", authorityID)
		}
	}
}

// Rust source: crates/noema-capabilities/src/policy.rs::destination_rejects_display_text_and_control_characters.
func TestRustCapabilities_destination_rejects_display_text_and_control_characters(t *testing.T) {
	for _, authorityID := range []string{"personal account", "personal\nother"} {
		if validAuthorityIdentity(authorityID) {
			t.Errorf("destination identity %q was accepted", authorityID)
		}
	}
}

// Rust source: crates/noema-capabilities/src/policy.rs::destination_serializes_only_stable_non_secret_identity.
func TestRustCapabilities_destination_serializes_only_stable_non_secret_identity(t *testing.T) {
	request := MCPAuthRequest{AuthorityKind: "adapter_connection", AuthorityID: "connection:personal", CapabilityName: "adapter.calendar",
		BindingJSON:   `{"service_id":"adapter:calendar","connection_id":"connection:personal","account_id":"account:synthetic","revision":"definition:7/credential:2"}`,
		ArgumentsJSON: `{"ordinary":"value"}`}
	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	for _, secret := range []string{"access_token", "refresh_token", "client_secret", "secret"} {
		if strings.Contains(string(encoded), secret) {
			t.Fatalf("secret field %q entered destination serialization: %s", secret, encoded)
		}
	}
	if !strings.Contains(string(encoded), `"AuthorityID":"connection:personal"`) {
		t.Fatalf("stable connection identity missing: %s", encoded)
	}
}
