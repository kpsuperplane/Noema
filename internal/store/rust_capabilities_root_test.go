package store

import (
	"encoding/json"
	"errors"
	"reflect"
	"strings"
	"testing"
)

// Rust source: crates/noema-capabilities/src/authentication/tests.rs::challenge_round_trips_only_bounded_non_secret_identity.
func TestRustCapabilities_challenge_round_trips_only_bounded_non_secret_identity(t *testing.T) {
	challenge, err := NewCapabilityAuthenticationChallenge(
		CapabilityAuthenticationReauthenticate,
		CapabilityAuthenticationMCPServer,
		"mcp:docs",
		"authority:7",
	)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(challenge)
	if err != nil {
		t.Fatal(err)
	}
	var decoded CapabilityAuthenticationChallenge
	if err := json.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, challenge) {
		t.Fatalf("challenge changed after round trip: %#v", decoded)
	}
	var shape map[string]any
	if err := json.Unmarshal(encoded, &shape); err != nil {
		t.Fatal(err)
	}
	if shape["authority_id"] != "mcp:docs" || shape["authority_revision"] != "authority:7" {
		t.Fatalf("challenge identity = %#v", shape)
	}
	if strings.Contains(string(encoded), "secret") {
		t.Fatalf("secret entered authentication challenge: %s", encoded)
	}
}

// Rust source: crates/noema-capabilities/src/authentication/tests.rs::challenge_rejects_display_text_controls_and_oversize_values.
func TestRustCapabilities_challenge_rejects_display_text_controls_and_oversize_values(t *testing.T) {
	for _, invalid := range []string{"", "display label", "line\nbreak", strings.Repeat("a", 513)} {
		if _, err := NewCapabilityAuthenticationChallenge(
			CapabilityAuthenticationReplaceCredential,
			CapabilityAuthenticationAdapterConnection,
			invalid,
			"revision:1",
		); err == nil {
			t.Errorf("authority ID %q was accepted", invalid)
		}
	}
}

// Rust source: crates/noema-capabilities/src/policy.rs::destination_rejects_display_text_and_control_characters.
func TestRustCapabilities_destination_rejects_display_text_and_control_characters(t *testing.T) {
	for _, connectionID := range []string{"personal account", "personal\nother"} {
		_, err := NewCapabilityDestination("gmail", connectionID, nil, "1")
		var typed CapabilityDestinationError
		if err == nil || !errors.As(err, &typed) || typed != (CapabilityDestinationError{Kind: "invalid", Field: "connection_id"}) {
			t.Errorf("connection identity %q error = %v", connectionID, err)
		}
	}
}

// Rust source: crates/noema-capabilities/src/policy.rs::destination_serializes_only_stable_non_secret_identity.
func TestRustCapabilities_destination_serializes_only_stable_non_secret_identity(t *testing.T) {
	accountID := "account:synthetic"
	destination, err := NewCapabilityDestination(
		"adapter:calendar",
		"connection:personal",
		&accountID,
		"definition:7/credential:2",
	)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := json.Marshal(destination)
	if err != nil {
		t.Fatal(err)
	}
	var got map[string]any
	if err := json.Unmarshal(encoded, &got); err != nil {
		t.Fatal(err)
	}
	want := map[string]any{
		"service_id": "adapter:calendar", "connection_id": "connection:personal",
		"account_id": "account:synthetic", "revision": "definition:7/credential:2",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("destination = %#v, want %#v", got, want)
	}
	if strings.Contains(string(encoded), "access_token") || strings.Contains(string(encoded), "refresh_token") || strings.Contains(string(encoded), "client_secret") {
		t.Fatalf("secret entered destination serialization: %s", encoded)
	}
}
