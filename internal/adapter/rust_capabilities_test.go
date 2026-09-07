package adapter

import (
	"testing"
)

// Rust source: crates/noema-capabilities/src/credential_sanitization.rs::exact_standard_fields_are_removed_without_damaging_lookalikes.
func TestRustCapabilities_exact_standard_fields_are_removed_without_damaging_lookalikes(t *testing.T) {
	sanitized, ok := sanitizeSensitiveOutputWithQueryNames(map[string]any{
		"Authorization":                 "Bearer private",
		"access_token":                  "private",
		"nested":                        map[string]any{"PASSWORD": "private"},
		"oauth_authorization_supported": true,
		"authorization_url":             "https://example.com/authorize",
		"token_count":                   float64(12),
		"cookie_policy":                 "strict",
		"secret_rotation_status":        "current",
		"client_id":                     "ordinary-client",
		"opaque_id":                     "p8xQ2zL7wN4vR9mK",
	}, nil, nil, nil).(map[string]any)
	if !ok {
		t.Fatal("sanitized payload is not an object")
	}
	if sanitized["Authorization"] != "[REDACTED]" {
		t.Errorf("Authorization = %#v", sanitized["Authorization"])
	}
	if sanitized["access_token"] != "[REDACTED]" {
		t.Errorf("access_token = %#v", sanitized["access_token"])
	}
	nested, ok := sanitized["nested"].(map[string]any)
	if !ok || nested["PASSWORD"] != "[REDACTED]" {
		t.Errorf("nested password = %#v", sanitized["nested"])
	}
	for key, want := range map[string]any{
		"oauth_authorization_supported": true,
		"authorization_url":             "https://example.com/authorize",
		"token_count":                   12,
		"cookie_policy":                 "strict",
		"secret_rotation_status":        "current",
		"client_id":                     "ordinary-client",
		"opaque_id":                     "p8xQ2zL7wN4vR9mK",
	} {
		if key == "token_count" {
			if value, ok := sanitized[key].(float64); !ok || value != 12 {
				t.Errorf("%s = %#v, want 12", key, sanitized[key])
			}
			continue
		}
		if sanitized[key] != want {
			t.Errorf("%s = %#v, want %#v", key, sanitized[key], want)
		}
	}
}

// Rust source: crates/noema-capabilities/src/credential_sanitization.rs::urls_lose_only_standard_credentials.
func TestRustCapabilities_urls_lose_only_standard_credentials(t *testing.T) {
	check := func(raw, want string, removed bool) {
		t.Helper()
		value := sanitizeSensitiveOutputWithQueryNames(map[string]any{"url": raw}, nil, nil, nil).(map[string]any)
		clean, _ := value["url"].(string)
		if clean != want {
			t.Errorf("clean URL = %q, want %q", clean, want)
		}
		if (clean != raw) != removed {
			t.Errorf("URL changed = %t, want %t", clean != raw, removed)
		}
	}
	check("https://user:password@example.com/path?view=full&access_token=private#section", "https://example.com/path?view=full#section", true)
	check("https://example.com/callback#access_token=private&state=ordinary", "https://example.com/callback#state=ordinary", true)
	check("https://example.com/path?token_count=5&code=sample#section", "https://example.com/path?token_count=5&code=sample#section", false)
}

// Rust source: crates/noema-capabilities/src/credential_sanitization.rs::connection_declared_names_extend_exact_field_and_url_cleanup.
func TestRustCapabilities_connection_declared_names_extend_exact_field_and_url_cleanup(t *testing.T) {
	sanitized, ok := sanitizeSensitiveOutputWithQueryNames(map[string]any{
		"X-Custom-Credential":       "private",
		"x_custom_credential_count": 2,
		"url":                       "https://example.com/path?view=full&x-custom-credential=ordinary&authz=private#section",
	}, map[string]bool{"authz": true, "x-custom-credential": true}, map[string]bool{"authz": true}, nil).(map[string]any)
	if !ok {
		t.Fatal("sanitized payload is not an object")
	}
	if sanitized["X-Custom-Credential"] != "[REDACTED]" {
		t.Errorf("custom credential = %#v", sanitized["X-Custom-Credential"])
	}
	if sanitized["x_custom_credential_count"] != 2 {
		t.Errorf("custom credential lookalike = %#v", sanitized["x_custom_credential_count"])
	}
	if sanitized["url"] != "https://example.com/path?view=full&x-custom-credential=ordinary#section" {
		t.Errorf("custom credential URL = %#v", sanitized["url"])
	}
}
