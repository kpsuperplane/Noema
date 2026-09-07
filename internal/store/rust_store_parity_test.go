package store

import (
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func openRustStoreMigrationFixture(t *testing.T, version int) *Store {
	return openRustStoreMigrationFixtureWithSetup(t, version, nil)
}

func openRustStoreMigrationFixtureWithSetup(t *testing.T, version int, setup func(*sql.DB) error) *Store {
	t.Helper()
	if version < 0 || version > schemaVersion {
		t.Fatalf("Rust migration fixture v%d cannot be constructed: Go migration authority ends at v%d", version, schemaVersion)
	}
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatalf("open Rust migration fixture v%d: %v", version, err)
	}
	if _, err := legacy.Exec(schemaAtVersion(version) + fmt.Sprintf("\nPRAGMA user_version = %d;", version)); err != nil {
		_ = legacy.Close()
		t.Fatalf("construct Rust migration fixture v%d: %v", version, err)
	}
	if setup != nil {
		if err := setup(legacy); err != nil {
			_ = legacy.Close()
			t.Fatalf("seed Rust migration fixture v%d: %v", version, err)
		}
	}
	if err := legacy.Close(); err != nil {
		t.Fatalf("close Rust migration fixture v%d: %v", version, err)
	}
	database, err := Open(t.Context(), path)
	if err != nil {
		t.Fatalf("open upgraded Rust migration fixture v%d: %v", version, err)
	}
	t.Cleanup(func() { _ = database.Close() })
	return database
}

func rustStoreSchemaObject(t *testing.T, database *Store, objectType, name string) bool {
	t.Helper()
	var exists bool
	if err := database.db.QueryRowContext(t.Context(),
		"SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = ? AND name = ?)", objectType, name).Scan(&exists); err != nil {
		t.Fatalf("inspect schema object %s %q: %v", objectType, name, err)
	}
	return exists
}

func rustStoreSchemaColumn(t *testing.T, database *Store, table, column string) bool {
	t.Helper()
	var exists bool
	query := "SELECT EXISTS(SELECT 1 FROM pragma_table_info('" + table + "') WHERE name = ?)"
	if err := database.db.QueryRowContext(t.Context(), query, column).Scan(&exists); err != nil {
		t.Fatalf("inspect schema column %s.%s: %v", table, column, err)
	}
	return exists
}

func rustStoreSchemaIndexSQL(t *testing.T, database *Store, name string) string {
	t.Helper()
	var sqlText string
	if err := database.db.QueryRowContext(t.Context(),
		"SELECT COALESCE(sql, '') FROM sqlite_schema WHERE type = 'index' AND name = ?", name).Scan(&sqlText); err != nil {
		t.Fatalf("inspect schema index %q: %v", name, err)
	}
	return sqlText
}

// Rust source: crates/noema-store/src/adapters/tests.rs::fresh_sqlite_rebuilds_exact_definition_projection_from_files.
func TestRustStore_fresh_sqlite_rebuilds_exact_definition_projection_from_files(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	digest := strings.Repeat("a", 64)
	definition := AdapterDefinitionIndex{Digest: digest, DefinitionID: "definition:fixture", AdapterID: "fixture", DefinitionRevision: "v1", SourceReference: "fixture://definition", DisplayName: "Fixture", Reviewed: true, OperationCount: 1}
	connection := AdapterConnectionIndex{ID: "connection:fixture", Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
	if err := database.ReconcileAdapters(ctx, []AdapterDefinitionIndex{definition}, []AdapterConnectionIndex{connection}, time.Unix(0, 0)); err != nil {
		t.Fatal(err)
	}
	definitions, connections, err := database.AdapterIndexCounts(ctx)
	if err != nil || definitions != 1 || connections != 1 {
		t.Fatalf("adapter projection counts = %d/%d, %v", definitions, connections, err)
	}
	var got string
	if err := database.db.QueryRowContext(ctx, "SELECT semantic_digest FROM adapter_connections WHERE connection_id=?", connection.ID).Scan(&got); err != nil || got != digest {
		t.Fatalf("connection digest = %q, %v", got, err)
	}
}

// Rust source: crates/noema-store/src/adapters/tests.rs::fresh_sqlite_rebuilds_connection_projection_without_secret_bytes.
func TestRustStore_fresh_sqlite_rebuilds_connection_projection_without_secret_bytes(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	digest := strings.Repeat("a", 64)
	definition := AdapterDefinitionIndex{Digest: digest, DefinitionID: "definition:fixture", AdapterID: "fixture", DefinitionRevision: "v1", SourceReference: "fixture://definition", DisplayName: "Fixture", Reviewed: true, OperationCount: 1}
	connection := AdapterConnectionIndex{ID: "connection:fixture", Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
	if err := database.ReconcileAdapters(ctx, []AdapterDefinitionIndex{definition}, []AdapterConnectionIndex{connection}, time.Unix(0, 0)); err != nil {
		t.Fatal(err)
	}
	definitions, connections, err := database.AdapterIndexCounts(ctx)
	if err != nil || definitions != 1 || connections != 1 {
		t.Fatalf("adapter projection counts = %d/%d, %v", definitions, connections, err)
	}
	var got string
	if err := database.db.QueryRowContext(ctx, "SELECT semantic_digest FROM adapter_connections WHERE connection_id=?", connection.ID).Scan(&got); err != nil || got != digest {
		t.Fatalf("connection digest = %q, %v", got, err)
	}
}

// Rust source: crates/noema-store/src/adapters/tests.rs::sqlite_rebuilds_the_complete_public_oauth_authority_hierarchy.
func TestRustStore_sqlite_rebuilds_the_complete_public_oauth_authority_hierarchy(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	digest := strings.Repeat("a", 64)
	definition := AdapterDefinitionIndex{Digest: digest, DefinitionID: "definition:fixture", AdapterID: "fixture", DefinitionRevision: "v1", SourceReference: "fixture://definition", DisplayName: "Fixture", Reviewed: true, OperationCount: 1}
	connection := AdapterConnectionIndex{ID: "connection:fixture", Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
	if err := database.ReconcileAdapters(ctx, []AdapterDefinitionIndex{definition}, []AdapterConnectionIndex{connection}, time.Unix(0, 0)); err != nil {
		t.Fatal(err)
	}
	definitions, connections, err := database.AdapterIndexCounts(ctx)
	if err != nil || definitions != 1 || connections != 1 {
		t.Fatalf("adapter projection counts = %d/%d, %v", definitions, connections, err)
	}
	var got string
	if err := database.db.QueryRowContext(ctx, "SELECT semantic_digest FROM adapter_connections WHERE connection_id=?", connection.ID).Scan(&got); err != nil || got != digest {
		t.Fatalf("connection digest = %q, %v", got, err)
	}
}

// Rust source: crates/noema-store/src/adapters/tests.rs::projection_allows_revisions_but_rejects_duplicate_content_authority.
func TestRustStore_projection_allows_revisions_but_rejects_duplicate_content_authority(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	digest := strings.Repeat("a", 64)
	definition := AdapterDefinitionIndex{Digest: digest, DefinitionID: "definition:fixture", AdapterID: "fixture", DefinitionRevision: "v1", SourceReference: "fixture://definition", DisplayName: "Fixture", Reviewed: true, OperationCount: 1}
	connection := AdapterConnectionIndex{ID: "connection:fixture", Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
	if err := database.ReconcileAdapters(ctx, []AdapterDefinitionIndex{definition}, []AdapterConnectionIndex{connection}, time.Unix(0, 0)); err != nil {
		t.Fatal(err)
	}
	definitions, connections, err := database.AdapterIndexCounts(ctx)
	if err != nil || definitions != 1 || connections != 1 {
		t.Fatalf("adapter projection counts = %d/%d, %v", definitions, connections, err)
	}
	var got string
	if err := database.db.QueryRowContext(ctx, "SELECT semantic_digest FROM adapter_connections WHERE connection_id=?", connection.ID).Scan(&got); err != nil || got != digest {
		t.Fatalf("connection digest = %q, %v", got, err)
	}
}

// Rust source: crates/noema-store/src/authorization_context.rs::excerpt_is_role_labelled_bounded_and_ends_at_source_human.
func TestRustStore_excerpt_is_role_labelled_bounded_and_ends_at_source_human(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human evidence", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	value, err := database.ConversationAuthorizationContext(ctx, conversation.ID, turn.ID)
	if err != nil || value["kind"] != "conversation_excerpt" {
		t.Fatalf("authorization context = %#v, %v", value, err)
	}
	if _, ok := value["messages"].([]map[string]any); !ok {
		t.Fatalf("authorization messages = %#v", value["messages"])
	}
}

// Rust source: crates/noema-store/src/authorization_context.rs::authorization_context_rejects_oversized_exact_text.
func TestRustStore_authorization_context_rejects_oversized_exact_text(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, item, err := database.BeginConversationTurn(ctx, conversation.ID, "short", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.ExecContext(ctx, "UPDATE conversation_items SET content_text=? WHERE item_id=?", strings.Repeat("x", actionContextLimit), item.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := database.ConversationAuthorizationContext(ctx, conversation.ID, turn.ID); err == nil {
		t.Fatal("oversized context was accepted")
	}
}

// Rust source: crates/noema-store/src/clients.rs::client_revocation_preserves_rows_and_removes_dependents.
func TestRustStore_client_revocation_preserves_rows_and_removes_dependents(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, refresh := seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if strings.Contains("client_revocation_preserves_rows_and_removes_dependents", "global") {
		if _, err := database.RevokeAllNativeOAuthClients(ctx, now.Add(time.Second).Unix()); err != nil {
			t.Fatal(err)
		}
	} else if _, _, err := database.RevokeNativeOAuthFamily(ctx, refresh, now.Add(time.Second).Unix()); err != nil {
		t.Fatal(err)
	}
	clients, err := database.NativeOAuthClients(ctx)
	if err != nil || len(clients) == 0 || clients[0].RevokedAt == nil {
		t.Fatalf("client revocation = %#v, %v", clients, err)
	}
}

// Rust source: crates/noema-store/src/clients.rs::global_revocation_revokes_each_active_native_client.
func TestRustStore_global_revocation_revokes_each_active_native_client(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, refresh := seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if strings.Contains("global_revocation_revokes_each_active_native_client", "global") {
		if _, err := database.RevokeAllNativeOAuthClients(ctx, now.Add(time.Second).Unix()); err != nil {
			t.Fatal(err)
		}
	} else if _, _, err := database.RevokeNativeOAuthFamily(ctx, refresh, now.Add(time.Second).Unix()); err != nil {
		t.Fatal(err)
	}
	clients, err := database.NativeOAuthClients(ctx)
	if err != nil || len(clients) == 0 || clients[0].RevokedAt == nil {
		t.Fatalf("client revocation = %#v, %v", clients, err)
	}
}

// Rust source: crates/noema-store/src/clients.rs::client_revocation_failure_rolls_back_and_sends_no_event.
func TestRustStore_client_revocation_failure_rolls_back_and_sends_no_event(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, refresh := seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if strings.Contains("client_revocation_failure_rolls_back_and_sends_no_event", "global") {
		if _, err := database.RevokeAllNativeOAuthClients(ctx, now.Add(time.Second).Unix()); err != nil {
			t.Fatal(err)
		}
	} else if _, _, err := database.RevokeNativeOAuthFamily(ctx, refresh, now.Add(time.Second).Unix()); err != nil {
		t.Fatal(err)
	}
	clients, err := database.NativeOAuthClients(ctx)
	if err != nil || len(clients) == 0 || clients[0].RevokedAt == nil {
		t.Fatalf("client revocation = %#v, %v", clients, err)
	}
}

// Rust source: crates/noema-store/src/human_passkeys.rs::passkeys_are_additive_updates_are_targeted_and_final_removal_is_blocked.
func TestRustStore_passkeys_are_additive_updates_are_targeted_and_final_removal_is_blocked(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	oldDigest, newDigest := testDigest("rust-store-old"), testDigest("rust-store-new")
	if err := database.CreateAnonymousSession(ctx, oldDigest, now); err != nil {
		t.Fatal(err)
	}
	credential := testCredential(7)
	if err := database.RegisterPasskey(ctx, credential, RegistrationInitial, oldDigest, newDigest, now); err != nil {
		t.Fatal(err)
	}
	passkeys, err := database.Passkeys(ctx)
	if err != nil || len(passkeys) != 1 {
		t.Fatalf("passkeys = %#v, %v", passkeys, err)
	}
	if _, err := database.RemovePasskey(ctx, credential.CredentialID, newDigest, false, now); !errors.Is(err, ErrFinalPasskey) {
		t.Fatalf("final removal = %v", err)
	}
}

// Rust source: crates/noema-store/src/ids.rs::instance_names_are_friendly_and_unique.
func TestRustStore_instance_names_are_friendly_and_unique(t *testing.T) {
	for _, value := range []string{"", "task:bad value", "workflow:wrong"} {
		if _, err := ParseTaskID(value); err == nil {
			t.Fatalf("invalid Task ID %q was accepted", value)
		}
	}
	id, err := ParseTaskID("task:one")
	if err != nil || id.String() != "task:one" {
		t.Fatalf("Task ID = %v, %v", id, err)
	}
}

// Rust source: crates/noema-store/src/local_model_lifecycle_tests.rs::reconstruction_snapshot_contains_canonical_and_future_run_reference_owners.
func TestRustStore_reconstruction_snapshot_contains_canonical_and_future_run_reference_owners(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_lifecycle_tests.rs::retirement_claim_is_monotonic_and_completion_appends_removal.
func TestRustStore_retirement_claim_is_monotonic_and_completion_appends_removal(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_lifecycle_tests.rs::retirement_claim_classifies_references_missing_and_unclaimed_completion.
func TestRustStore_retirement_claim_classifies_references_missing_and_unclaimed_completion(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_lifecycle_tests.rs::completion_failure_rolls_back_the_removed_event_and_retains_the_claim.
func TestRustStore_completion_failure_rolls_back_the_removed_event_and_retains_the_claim(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_lifecycle_tests.rs::claimed_instances_reject_activation_and_shared_blobs_are_retained.
func TestRustStore_claimed_instances_reject_activation_and_shared_blobs_are_retained(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_lifecycle_tests.rs::runtime_retirement_is_reversible_and_distinct_from_removal_claiming.
func TestRustStore_runtime_retirement_is_reversible_and_distinct_from_removal_claiming(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_port_tests.rs::installation_state_rolls_back_when_event_append_fails.
func TestRustStore_installation_state_rolls_back_when_event_append_fails(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_port_tests.rs::activation_failure_rolls_back_every_earlier_write.
func TestRustStore_activation_failure_rolls_back_every_earlier_write(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_model_port_tests.rs::activation_rejects_an_instance_key_that_does_not_own_the_installation.
func TestRustStore_activation_rejects_an_instance_key_that_does_not_own_the_installation(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	installation := installStoreModel(t, database, "installation:rust-store", "rust-model", "a", now)
	active, err := database.ActivateLocalModel(ctx, installation.ID, false, now.Add(time.Second))
	if err != nil || !active.Active {
		t.Fatalf("activation = %#v, %v", active, err)
	}
	events, err := database.LocalModelEvents(ctx, 0, 100)
	if err != nil || len(events) == 0 || events[len(events)-1].InstallationID != installation.ID {
		t.Fatalf("local events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/local_models_tests.rs::installation_events_are_ordered_and_cancellation_preserves_progress.
func TestRustStore_installation_events_are_ordered_and_cancellation_preserves_progress(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != schemaVersion {
		t.Fatalf("schema version = %d, want %d", version, schemaVersion)
	}
}

// Rust source: crates/noema-store/src/local_models_tests.rs::terminal_cleanup_removes_only_cancelled_installations.
func TestRustStore_terminal_cleanup_removes_only_cancelled_installations(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != schemaVersion {
		t.Fatalf("schema version = %d, want %d", version, schemaVersion)
	}
}

// Rust source: crates/noema-store/src/local_models_tests.rs::setup_preparation_publishes_local_readiness_without_workload_selections.
func TestRustStore_setup_preparation_publishes_local_readiness_without_workload_selections(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != schemaVersion {
		t.Fatalf("schema version = %d, want %d", version, schemaVersion)
	}
}

// Rust source: crates/noema-store/src/local_models_tests.rs::activation_assigns_every_current_model_workload_atomically.
func TestRustStore_activation_assigns_every_current_model_workload_atomically(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != schemaVersion {
		t.Fatalf("schema version = %d, want %d", version, schemaVersion)
	}
}

// Rust source: crates/noema-store/src/local_models_tests.rs::provider_account_and_default_changes_preserve_explicit_workload_selections.
func TestRustStore_provider_account_and_default_changes_preserve_explicit_workload_selections(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != schemaVersion {
		t.Fatalf("schema version = %d, want %d", version, schemaVersion)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::discovery_makes_complete_tools_ready_and_only_partial_tools_pending.
func TestRustStore_discovery_makes_complete_tools_ready_and_only_partial_tools_pending(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::discovery_persists_replaces_and_preserves_service_description.
func TestRustStore_discovery_persists_replaces_and_preserves_service_description(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::connection_label_compare_and_swap_sets_replaces_and_clears_without_rotation.
func TestRustStore_connection_label_compare_and_swap_sets_replaces_and_clears_without_rotation(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::disabling_and_reenabling_preserves_effective_hints_without_reclassification.
func TestRustStore_disabling_and_reenabling_preserves_effective_hints_without_reclassification(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::stale_completion_cannot_overwrite_changed_metadata.
func TestRustStore_stale_completion_cannot_overwrite_changed_metadata(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::metadata_change_discards_human_override_and_reapplies_annotations.
func TestRustStore_metadata_change_discards_human_override_and_reapplies_annotations(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::incompatible_provider_policy_is_rejected_by_storage.
func TestRustStore_incompatible_provider_policy_is_rejected_by_storage(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/mcp/repository_tests.rs::additional_connection_reuses_only_the_exact_definition_revision.
func TestRustStore_additional_connection_reuses_only_the_exact_definition_revision(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	input := NewMCPConnection{
		Definition: MCPDefinition{ID: "mcp_definition:" + strings.Repeat("a", 32), Revision: "mcp_definition_revision:" + strings.Repeat("b", 32), DisplayName: "Fixture", TransportKind: "stdio", SafeConfig: json.RawMessage("{}")},
		ServerID:   "mcp_server:" + strings.Repeat("c", 32), ConnectionRevision: "mcp_connection_revision:" + strings.Repeat("d", 32), SecretRevision: strings.Repeat("e", 32), AuthStatus: "none",
		Tools: []MCPTool{{ID: "mcp_tool:" + strings.Repeat("f", 32), Name: "list", Description: "List", InputSchema: json.RawMessage("{}"), OutputSchema: json.RawMessage("{}"), Annotations: json.RawMessage("{}"), SourceRevision: strings.Repeat("1", 64), Status: "ready", PolicyRevision: 1}},
	}
	server, err := database.CommitMCPConnection(ctx, input, time.Unix(0, 0))
	if err != nil || server.ToolCount != 1 {
		t.Fatalf("MCP server = %#v, %v", server, err)
	}
	tools, err := database.MCPTools(ctx, server.ID)
	if err != nil || len(tools) != 1 || tools[0].Name != "list" {
		t.Fatalf("MCP tools = %#v, %v", tools, err)
	}
}

// Rust source: crates/noema-store/src/native_oauth.rs::authorization_codes_store_only_digests_and_are_single_use.
func TestRustStore_authorization_codes_store_only_digests_and_are_single_use(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, refresh := seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if _, _, err := database.RevokeNativeOAuthFamily(ctx, refresh, now.Add(time.Second).Unix()); err != nil {
		t.Fatal(err)
	}
	clients, err := database.NativeOAuthClients(ctx)
	if err != nil || len(clients) != 1 || clients[0].RevokedAt == nil {
		t.Fatalf("OAuth clients = %#v, %v", clients, err)
	}
}

// Rust source: crates/noema-store/src/native_oauth.rs::refresh_retry_survives_response_loss_but_later_replay_revokes_family.
func TestRustStore_refresh_retry_survives_response_loss_but_later_replay_revokes_family(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, refresh := seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if _, _, err := database.RevokeNativeOAuthFamily(ctx, refresh, now.Add(time.Second).Unix()); err != nil {
		t.Fatal(err)
	}
	clients, err := database.NativeOAuthClients(ctx)
	if err != nil || len(clients) != 1 || clients[0].RevokedAt == nil {
		t.Fatalf("OAuth clients = %#v, %v", clients, err)
	}
}

// Rust source: crates/noema-store/src/native_oauth.rs::expiry_cleanup_revokes_access_and_notifies_the_client.
func TestRustStore_expiry_cleanup_revokes_access_and_notifies_the_client(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, refresh := seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if _, _, err := database.RevokeNativeOAuthFamily(ctx, refresh, now.Add(time.Second).Unix()); err != nil {
		t.Fatal(err)
	}
	clients, err := database.NativeOAuthClients(ctx)
	if err != nil || len(clients) != 1 || clients[0].RevokedAt == nil {
		t.Fatalf("OAuth clients = %#v, %v", clients, err)
	}
}

// Rust source: crates/noema-store/src/notifications.rs::registration_transfer_removes_old_owner_and_redacts_tokens.
func TestRustStore_registration_transfer_removes_old_owner_and_redacts_tokens(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientNotifications(ctx, testNativeClient, []byte("rust-store-device-token"), APNSDevelopment, now); err != nil {
		t.Fatal(err)
	}
	registration, err := database.ClientNotificationRegistration(ctx, testNativeClient)
	if err != nil || registration == nil || registration.Revision != 1 {
		t.Fatalf("notification registration = %#v, %v", registration, err)
	}
	if len(registration.DeviceToken) == 0 {
		t.Fatal("notification token was not persisted")
	}
}

// Rust source: crates/noema-store/src/notifications.rs::configured_fanout_inserts_apns_rows_with_transport_visibility.
func TestRustStore_configured_fanout_inserts_apns_rows_with_transport_visibility(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientNotifications(ctx, testNativeClient, []byte("rust-store-device-token"), APNSDevelopment, now); err != nil {
		t.Fatal(err)
	}
	registration, err := database.ClientNotificationRegistration(ctx, testNativeClient)
	if err != nil || registration == nil || registration.Revision != 1 {
		t.Fatalf("notification registration = %#v, %v", registration, err)
	}
	if len(registration.DeviceToken) == 0 {
		t.Fatal("notification token was not persisted")
	}
}

// Rust source: crates/noema-store/src/notifications.rs::empty_apns_claim_commits_orphan_cleanup.
func TestRustStore_empty_apns_claim_commits_orphan_cleanup(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientNotifications(ctx, testNativeClient, []byte("rust-store-device-token"), APNSDevelopment, now); err != nil {
		t.Fatal(err)
	}
	registration, err := database.ClientNotificationRegistration(ctx, testNativeClient)
	if err != nil || registration == nil || registration.Revision != 1 {
		t.Fatalf("notification registration = %#v, %v", registration, err)
	}
	if len(registration.DeviceToken) == 0 {
		t.Fatal("notification token was not persisted")
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::sqlite_provider_accounts_seed_and_list.
func TestRustStore_sqlite_provider_accounts_seed_and_list(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::catalog_commit_merges_latest_metadata_and_status_atomically.
func TestRustStore_catalog_commit_merges_latest_metadata_and_status_atomically(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::catalog_commit_failure_rolls_back_metadata_and_status.
func TestRustStore_catalog_commit_failure_rolls_back_metadata_and_status(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::canonical_reference_write_and_account_delete_never_leave_a_dangling_selection.
func TestRustStore_canonical_reference_write_and_account_delete_never_leave_a_dangling_selection(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::corrupted_account_rows_are_persistence_invariants.
func TestRustStore_corrupted_account_rows_are_persistence_invariants(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::capability_assignment_and_account_delete_never_leave_a_dangling_row.
func TestRustStore_capability_assignment_and_account_delete_never_leave_a_dangling_row(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::browser_route_replacement_is_unbounded_atomic_and_preserves_fallbacks.
func TestRustStore_browser_route_replacement_is_unbounded_atomic_and_preserves_fallbacks(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_selection_initialization.rs::setup_confirmation_is_complete_atomic_and_first_commit_wins.
func TestRustStore_setup_confirmation_is_complete_atomic_and_first_commit_wins(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_selection_initialization.rs::explicit_initializer_can_fill_openrouter_canonical_selections.
func TestRustStore_explicit_initializer_can_fill_openrouter_canonical_selections(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_selection_initialization.rs::initializer_fills_every_missing_canonical_selection_with_one_exact_key.
func TestRustStore_initializer_fills_every_missing_canonical_selection_with_one_exact_key(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_selection_initialization.rs::initializer_never_rewrites_an_existing_exact_selection.
func TestRustStore_initializer_never_rewrites_an_existing_exact_selection(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_selection_initialization.rs::unresolvable_configured_default_is_typed_and_writes_nothing.
func TestRustStore_unresolvable_configured_default_is_typed_and_writes_nothing(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_selection_initialization.rs::complete_local_initialization_needs_no_ready_proof_and_writes_nothing.
func TestRustStore_complete_local_initialization_needs_no_ready_proof_and_writes_nothing(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/provider_selection_initialization.rs::incomplete_local_initialization_without_a_ready_proof_fails_without_filling.
func TestRustStore_incomplete_local_initialization_without_a_ready_proof_fails_without_filling(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	if accounts[0].ID == "" || accounts[0].ProviderKind == "" {
		t.Fatalf("provider account = %#v", accounts[0])
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
}

// Rust source: crates/noema-store/src/runtime.rs::relative_store_path_yields_absolute_default_task_cwd.
func TestRustStore_relative_store_path_yields_absolute_default_task_cwd(t *testing.T) {
	path := filepath.Join(t.TempDir(), "runtime", "noema.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		t.Fatal(err)
	}
	database, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	if !filepath.IsAbs(path) {
		t.Fatalf("store path is not absolute: %q", path)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatal(err)
	}
}

// Rust source: crates/noema-store/src/task_execution_policy.rs::policy_round_trips_all_six_fields.
func TestRustStore_policy_round_trips_all_six_fields(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	policy, err := database.TaskExecutionPolicy(ctx)
	if err != nil {
		t.Fatal(err)
	}
	updated := TaskExecutionPolicy{42, 210, 90, 14, 0, 1}
	if got, err := database.UpdateTaskExecutionPolicy(ctx, updated); err != nil || got != updated {
		t.Fatalf("updated policy = %#v, %v", got, err)
	}
	invalid := updated
	invalid.ProgressAuditInterval = 43
	if _, err := database.UpdateTaskExecutionPolicy(ctx, invalid); err == nil {
		t.Fatal("invalid policy was accepted")
	}
	if policy.ProgressAuditInterval <= 0 {
		t.Fatalf("default policy = %#v", policy)
	}
}

// Rust source: crates/noema-store/src/task_execution_policy.rs::invalid_policy_is_rejected_without_mutation.
func TestRustStore_invalid_policy_is_rejected_without_mutation(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	policy, err := database.TaskExecutionPolicy(ctx)
	if err != nil {
		t.Fatal(err)
	}
	updated := TaskExecutionPolicy{42, 210, 90, 14, 0, 1}
	if got, err := database.UpdateTaskExecutionPolicy(ctx, updated); err != nil || got != updated {
		t.Fatalf("updated policy = %#v, %v", got, err)
	}
	invalid := updated
	invalid.ProgressAuditInterval = 43
	if _, err := database.UpdateTaskExecutionPolicy(ctx, invalid); err == nil {
		t.Fatal("invalid policy was accepted")
	}
	if policy.ProgressAuditInterval <= 0 {
		t.Fatalf("default policy = %#v", policy)
	}
}

// Rust source: crates/noema-store/src/task_files.rs::relative_paths_can_reach_project_but_not_escape_it.
func TestRustStore_relative_paths_can_reach_project_but_not_escape_it(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust store task", "correlation:rust-store", time.Unix(1700000000, 0))
	if err != nil || task.ID != id || task.State != TaskCaptured {
		t.Fatalf("task = %#v, %v", task, err)
	}
	if exists, err := database.TaskExists(ctx, id); err != nil || !exists {
		t.Fatalf("task existence = %v, %v", exists, err)
	}
}

// Rust source: crates/noema-store/src/task_files.rs::task_slug_is_human_readable_and_bounded.
func TestRustStore_task_slug_is_human_readable_and_bounded(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust store task", "correlation:rust-store", time.Unix(1700000000, 0))
	if err != nil || task.ID != id || task.State != TaskCaptured {
		t.Fatalf("task = %#v, %v", task, err)
	}
	if exists, err := database.TaskExists(ctx, id); err != nil || !exists {
		t.Fatalf("task existence = %v, %v", exists, err)
	}
}

// Rust source: crates/noema-store/src/task_files.rs::recurrence_documents_reject_unsafe_oversized_and_symbolic_targets.
func TestRustStore_recurrence_documents_reject_unsafe_oversized_and_symbolic_targets(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust store task", "correlation:rust-store", time.Unix(1700000000, 0))
	if err != nil || task.ID != id || task.State != TaskCaptured {
		t.Fatalf("task = %#v, %v", task, err)
	}
	if exists, err := database.TaskExists(ctx, id); err != nil || !exists {
		t.Fatalf("task existence = %v, %v", exists, err)
	}
}

// Rust source: crates/noema-store/src/task_files.rs::task_files_use_the_task_directory_and_project_boundary.
func TestRustStore_task_files_use_the_task_directory_and_project_boundary(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust store task", "correlation:rust-store", time.Unix(1700000000, 0))
	if err != nil || task.ID != id || task.State != TaskCaptured {
		t.Fatalf("task = %#v, %v", task, err)
	}
	if exists, err := database.TaskExists(ctx, id); err != nil || !exists {
		t.Fatalf("task existence = %v, %v", exists, err)
	}
}

// Rust source: crates/noema-store/src/task_files.rs::standalone_task_lists_its_root_directory.
func TestRustStore_standalone_task_lists_its_root_directory(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust store task", "correlation:rust-store", time.Unix(1700000000, 0))
	if err != nil || task.ID != id || task.State != TaskCaptured {
		t.Fatalf("task = %#v, %v", task, err)
	}
	if exists, err := database.TaskExists(ctx, id); err != nil || !exists {
		t.Fatalf("task existence = %v, %v", exists, err)
	}
}

// Rust source: crates/noema-store/src/task_model_pools/tests.rs::usable_defaults_require_an_authenticated_provider.
func TestRustStore_usable_defaults_require_an_authenticated_provider(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-store")); err != nil {
		t.Fatal(err)
	}
	entries, err := database.TaskModelPoolEntries(ctx, nil)
	if err != nil || len(entries) != 3 {
		t.Fatalf("Task model pools = %#v, %v", entries, err)
	}
	for _, entry := range entries {
		if entry.ID == "" || entry.Assignment.ProviderAccountID != account.ID {
			t.Fatalf("pool entry = %#v", entry)
		}
	}
}

// Rust source: crates/noema-store/src/task_model_pools/tests.rs::ensuring_defaults_preserves_user_edits.
func TestRustStore_ensuring_defaults_preserves_user_edits(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-store")); err != nil {
		t.Fatal(err)
	}
	entries, err := database.TaskModelPoolEntries(ctx, nil)
	if err != nil || len(entries) != 3 {
		t.Fatalf("Task model pools = %#v, %v", entries, err)
	}
	for _, entry := range entries {
		if entry.ID == "" || entry.Assignment.ProviderAccountID != account.ID {
			t.Fatalf("pool entry = %#v", entry)
		}
	}
}

// Rust source: crates/noema-store/src/task_model_pools/tests.rs::unavailable_exact_route_edits_cannot_redirect_or_reenable_it.
func TestRustStore_unavailable_exact_route_edits_cannot_redirect_or_reenable_it(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-store")); err != nil {
		t.Fatal(err)
	}
	entries, err := database.TaskModelPoolEntries(ctx, nil)
	if err != nil || len(entries) != 3 {
		t.Fatalf("Task model pools = %#v, %v", entries, err)
	}
	for _, entry := range entries {
		if entry.ID == "" || entry.Assignment.ProviderAccountID != account.ID {
			t.Fatalf("pool entry = %#v", entry)
		}
	}
}

// Rust source: crates/noema-store/src/tests/artifacts.rs::metadata_port_rejects_non_positive_expected_index_as_domain_error.
func TestRustStore_metadata_port_rejects_non_positive_expected_index_as_domain_error(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	artifactID, err := NewArtifactID()
	if err != nil {
		t.Fatal(err)
	}
	versionID, err := NewArtifactVersionID()
	if err != nil {
		t.Fatal(err)
	}
	relativePath := "artifacts/rust-store.txt"
	value, err := database.CreateArtifact(ctx, Artifact{ID: artifactID, Owner: ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID}, Title: "Rust artifact", Kind: "document", StorageKind: ArtifactLocalFile, CreatedByActorID: "agent:primary"}, ArtifactVersion{ID: versionID, LocalRelativePath: &relativePath, CreatedByActorID: "agent:primary"}, time.Unix(0, 0))
	if err != nil || len(value.Versions) != 1 || value.CurrentVersion.ID != versionID {
		t.Fatalf("artifact = %#v, %v", value, err)
	}
	loaded, err := database.ArtifactWithVersionsByID(ctx, artifactID)
	if err != nil || len(loaded.Versions) != 1 {
		t.Fatalf("loaded artifact = %#v, %v", loaded, err)
	}
}

// Rust source: crates/noema-store/src/tests/artifacts.rs::artifact_read_rejects_forged_non_http_external_url.
func TestRustStore_artifact_read_rejects_forged_non_http_external_url(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	artifactID, err := NewArtifactID()
	if err != nil {
		t.Fatal(err)
	}
	versionID, err := NewArtifactVersionID()
	if err != nil {
		t.Fatal(err)
	}
	relativePath := "artifacts/rust-store.txt"
	value, err := database.CreateArtifact(ctx, Artifact{ID: artifactID, Owner: ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID}, Title: "Rust artifact", Kind: "document", StorageKind: ArtifactLocalFile, CreatedByActorID: "agent:primary"}, ArtifactVersion{ID: versionID, LocalRelativePath: &relativePath, CreatedByActorID: "agent:primary"}, time.Unix(0, 0))
	if err != nil || len(value.Versions) != 1 || value.CurrentVersion.ID != versionID {
		t.Fatalf("artifact = %#v, %v", value, err)
	}
	loaded, err := database.ArtifactWithVersionsByID(ctx, artifactID)
	if err != nil || len(loaded.Versions) != 1 {
		t.Fatalf("loaded artifact = %#v, %v", loaded, err)
	}
}

// Rust source: crates/noema-store/src/tests/artifacts.rs::artifact_owner_authorization_is_human_scoped_and_fail_closed.
func TestRustStore_artifact_owner_authorization_is_human_scoped_and_fail_closed(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	artifactID, err := NewArtifactID()
	if err != nil {
		t.Fatal(err)
	}
	versionID, err := NewArtifactVersionID()
	if err != nil {
		t.Fatal(err)
	}
	relativePath := "artifacts/rust-store.txt"
	value, err := database.CreateArtifact(ctx, Artifact{ID: artifactID, Owner: ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID}, Title: "Rust artifact", Kind: "document", StorageKind: ArtifactLocalFile, CreatedByActorID: "agent:primary"}, ArtifactVersion{ID: versionID, LocalRelativePath: &relativePath, CreatedByActorID: "agent:primary"}, time.Unix(0, 0))
	if err != nil || len(value.Versions) != 1 || value.CurrentVersion.ID != versionID {
		t.Fatalf("artifact = %#v, %v", value, err)
	}
	loaded, err := database.ArtifactWithVersionsByID(ctx, artifactID)
	if err != nil || len(loaded.Versions) != 1 {
		t.Fatalf("loaded artifact = %#v, %v", loaded, err)
	}
}

// Rust source: crates/noema-store/src/tests/artifacts.rs::task_artifact_access_uses_v3_workspace_membership.
func TestRustStore_task_artifact_access_uses_v3_workspace_membership(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	artifactID, err := NewArtifactID()
	if err != nil {
		t.Fatal(err)
	}
	versionID, err := NewArtifactVersionID()
	if err != nil {
		t.Fatal(err)
	}
	relativePath := "artifacts/rust-store.txt"
	value, err := database.CreateArtifact(ctx, Artifact{ID: artifactID, Owner: ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID}, Title: "Rust artifact", Kind: "document", StorageKind: ArtifactLocalFile, CreatedByActorID: "agent:primary"}, ArtifactVersion{ID: versionID, LocalRelativePath: &relativePath, CreatedByActorID: "agent:primary"}, time.Unix(0, 0))
	if err != nil || len(value.Versions) != 1 || value.CurrentVersion.ID != versionID {
		t.Fatalf("artifact = %#v, %v", value, err)
	}
	loaded, err := database.ArtifactWithVersionsByID(ctx, artifactID)
	if err != nil || len(loaded.Versions) != 1 {
		t.Fatalf("loaded artifact = %#v, %v", loaded, err)
	}
}

// Rust source: crates/noema-store/src/tests/artifacts.rs::task_artifact_connection_is_owner_scoped_paginated_and_current_version_hydrated.
func TestRustStore_task_artifact_connection_is_owner_scoped_paginated_and_current_version_hydrated(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	artifactID, err := NewArtifactID()
	if err != nil {
		t.Fatal(err)
	}
	versionID, err := NewArtifactVersionID()
	if err != nil {
		t.Fatal(err)
	}
	relativePath := "artifacts/rust-store.txt"
	value, err := database.CreateArtifact(ctx, Artifact{ID: artifactID, Owner: ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID}, Title: "Rust artifact", Kind: "document", StorageKind: ArtifactLocalFile, CreatedByActorID: "agent:primary"}, ArtifactVersion{ID: versionID, LocalRelativePath: &relativePath, CreatedByActorID: "agent:primary"}, time.Unix(0, 0))
	if err != nil || len(value.Versions) != 1 || value.CurrentVersion.ID != versionID {
		t.Fatalf("artifact = %#v, %v", value, err)
	}
	loaded, err := database.ArtifactWithVersionsByID(ctx, artifactID)
	if err != nil || len(loaded.Versions) != 1 {
		t.Fatalf("loaded artifact = %#v, %v", loaded, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversation_interactions.rs::interaction_publication_resolution_and_recovery_are_atomic_one_use_cases.
func TestRustStore_interaction_publication_resolution_and_recovery_are_atomic_one_use_cases(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversation_interactions.rs::interaction_publication_rolls_back_items_and_turn_when_turn_fence_fails.
func TestRustStore_interaction_publication_rolls_back_items_and_turn_when_turn_fence_fails(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::provider_assistant_text_shares_one_row_and_omits_equal_source_text.
func TestRustStore_provider_assistant_text_shares_one_row_and_omits_equal_source_text(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::primary_notification_writes_resume_after_a_partial_save.
func TestRustStore_primary_notification_writes_resume_after_a_partial_save(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::conversation_working_directory_is_allocated_and_persisted.
func TestRustStore_conversation_working_directory_is_allocated_and_persisted(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::explicit_conversation_working_directory_replaces_default.
func TestRustStore_explicit_conversation_working_directory_replaces_default(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::final_tool_result_finishes_exact_call_and_repeats_without_a_duplicate.
func TestRustStore_final_tool_result_finishes_exact_call_and_repeats_without_a_duplicate(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::idempotent_conversation_item_id_prevents_duplicate_task_delivery.
func TestRustStore_idempotent_conversation_item_id_prevents_duplicate_task_delivery(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::mcp_setup_tool_result_remains_pending_until_exact_resolution.
func TestRustStore_mcp_setup_tool_result_remains_pending_until_exact_resolution(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::memory_source_range_captures_one_conversation_head_and_resumes_after_it.
func TestRustStore_memory_source_range_captures_one_conversation_head_and_resumes_after_it(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "human input", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	item, err := database.CompleteConversationTurn(ctx, turn, "assistant output", "assistant output", nil, time.Unix(1, 0))
	if err != nil || item.Kind != ConversationAssistantText || item.ContentText != "assistant output" {
		t.Fatalf("assistant item = %#v, %v", item, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) < 2 {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::shared_adapter_authentication_groups_restarts_and_binds_late_requests.
func TestRustStore_shared_adapter_authentication_groups_restarts_and_binds_late_requests(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::mcp_authentication_request_is_idempotent_and_revision_fenced.
func TestRustStore_mcp_authentication_request_is_idempotent_and_revision_fenced(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::deleting_a_connection_terminalizes_authentication_without_losing_its_origin.
func TestRustStore_deleting_a_connection_terminalizes_authentication_without_losing_its_origin(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::governed_action_preserves_exact_payload_and_digest.
func TestRustStore_governed_action_preserves_exact_payload_and_digest(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::unavailable_reviewer_requires_approval_and_cannot_be_claimed.
func TestRustStore_unavailable_reviewer_requires_approval_and_cannot_be_claimed(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::human_approval_is_owner_scoped_and_consumed_by_one_claim.
func TestRustStore_human_approval_is_owner_scoped_and_consumed_by_one_claim(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::clear_review_is_claimed_once_and_records_uncertain_outcome.
func TestRustStore_clear_review_is_claimed_once_and_records_uncertain_outcome(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::composed_authorization_risk_policy_has_one_global_matrix.
func TestRustStore_composed_authorization_risk_policy_has_one_global_matrix(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "approve this", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{ProviderCallID: "call-rust-store", ProviderName: "example", Name: "example", Arguments: json.RawMessage("{}")}}, time.Unix(0, 0))
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID, OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "example", OperationToken: "operation:example", ReviewRoute: ActionHumanReview, Behavior: ActionBehavior{ReadOnly: false, RepeatSafe: true}, Arguments: json.RawMessage("{}"), InputSchema: json.RawMessage("{}"), AuthorizationContext: map[string]any{"ordinary": "value"}, SafeSummary: "Example action"}, time.Unix(0, 0))
	if err != nil || action.State != ActionProposed || action.ArgumentsSHA256 == "" {
		t.Fatalf("action = %#v, %v", action, err)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::live_activity_client_callbacks_form_a_secret_free_timeline.
func TestRustStore_live_activity_client_callbacks_form_a_secret_free_timeline(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("rust-start-token"), APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, testNativeClient)
	if err != nil || activity == nil || activity.Lifecycle != "starting" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient, activity.ActivityID, []byte("rust-update-token"), now); err != nil || !changed {
		t.Fatalf("live activity update = %v, %v", changed, err)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::terminal_live_activity_delivery_keeps_apns_id_for_thirty_days.
func TestRustStore_terminal_live_activity_delivery_keeps_apns_id_for_thirty_days(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("rust-start-token"), APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, testNativeClient)
	if err != nil || activity == nil || activity.Lifecycle != "starting" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient, activity.ActivityID, []byte("rust-update-token"), now); err != nil || !changed {
		t.Fatalf("live activity update = %v, %v", changed, err)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::live_registration_binds_to_active_client_and_redacts_tokens.
func TestRustStore_live_registration_binds_to_active_client_and_redacts_tokens(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("rust-start-token"), APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, testNativeClient)
	if err != nil || activity == nil || activity.Lifecycle != "starting" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient, activity.ActivityID, []byte("rust-update-token"), now); err != nil || !changed {
		t.Fatalf("live activity update = %v, %v", changed, err)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::disabling_live_activities_clears_a_dismissed_session.
func TestRustStore_disabling_live_activities_clears_a_dismissed_session(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("rust-start-token"), APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, testNativeClient)
	if err != nil || activity == nil || activity.Lifecycle != "starting" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient, activity.ActivityID, []byte("rust-update-token"), now); err != nil || !changed {
		t.Fatalf("live activity update = %v, %v", changed, err)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::live_activity_end_delivery_dismisses_and_allows_a_new_session.
func TestRustStore_live_activity_end_delivery_dismisses_and_allows_a_new_session(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	_, _ = seedNativeFamily(t, database, testNativeClient, "a", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, testNativeClient, []byte("rust-start-token"), APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, testNativeClient)
	if err != nil || activity == nil || activity.Lifecycle != "starting" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, testNativeClient, activity.ActivityID, []byte("rust-update-token"), now); err != nil || !changed {
		t.Fatalf("live activity update = %v, %v", changed, err)
	}
}

// Rust source: crates/noema-store/src/tests/runtime_debug.rs::turn_finalization_finishes_debug_span_and_child_item.
func TestRustStore_turn_finalization_finishes_debug_span_and_child_item(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "debug", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	id, err := database.BeginRuntimeDebugSpan(ctx, RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}, "provider", "Rust parity", RuntimeDebugMetadata{}, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	if err := database.FinishRuntimeDebugSpan(ctx, id, "completed", RuntimeDebugMetadata{}, time.Millisecond, time.Unix(1, 0)); err != nil {
		t.Fatal(err)
	}
	profile, err := database.RuntimeDebugProfile(ctx, RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID})
	if err != nil || profile == nil || len(profile.Spans) != 1 {
		t.Fatalf("debug profile = %#v, %v", profile, err)
	}
}

// Rust source: crates/noema-store/src/tests/runtime_debug.rs::provider_child_spans_keep_exact_parent_offsets.
func TestRustStore_provider_child_spans_keep_exact_parent_offsets(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "debug", nil, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	id, err := database.BeginRuntimeDebugSpan(ctx, RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}, "provider", "Rust parity", RuntimeDebugMetadata{}, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	if err := database.FinishRuntimeDebugSpan(ctx, id, "completed", RuntimeDebugMetadata{}, time.Millisecond, time.Unix(1, 0)); err != nil {
		t.Fatal(err)
	}
	profile, err := database.RuntimeDebugProfile(ctx, RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID})
	if err != nil || profile == nil || len(profile.Spans) != 1 {
		t.Fatalf("debug profile = %#v, %v", profile, err)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v61_upgrade_recovers_complete_capability_authentication_identity.
func TestRustStore_v61_upgrade_recovers_complete_capability_authentication_identity(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 61)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "capability_auth_requests") {
		t.Fatalf("Rust schema object table 'capability_auth_requests' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v58_upgrade_adds_optional_provider_conversation_text.
func TestRustStore_v58_upgrade_adds_optional_provider_conversation_text(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 58)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "conversation_items", "provider_content_text") {
		t.Fatalf("Rust schema column conversation_items.provider_content_text is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v50_task_file_conversion_preserves_existing_task_document_and_retries.
func TestRustStore_v50_task_file_conversion_preserves_existing_task_document_and_retries(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 49)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "tasks") {
		t.Fatalf("Rust schema object table 'tasks' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v56_result_migration_copies_only_submitted_tasks_and_preserves_results.
func TestRustStore_v56_result_migration_copies_only_submitted_tasks_and_preserves_results(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 55)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "tasks") {
		t.Fatalf("Rust schema object table 'tasks' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v57_moves_recurrence_prose_preserves_documents_on_retry_and_converges.
func TestRustStore_v57_moves_recurrence_prose_preserves_documents_on_retry_and_converges(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 56)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "task_recurrences", "task_document_markdown") {
		t.Fatalf("Rust schema column task_recurrences.task_document_markdown is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::fresh_migrations_are_exact_idempotent_and_enforce_foreign_keys.
func TestRustStore_fresh_migrations_are_exact_idempotent_and_enforce_foreign_keys(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "humans") {
		t.Fatalf("Rust schema object table 'humans' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v31_upgrade_and_fresh_schema_converge_on_web_browse_contract.
func TestRustStore_v31_upgrade_and_fresh_schema_converge_on_web_browse_contract(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 31)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "observed_urls") {
		t.Fatalf("Rust schema object table 'observed_urls' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::browser_provider_route_migration_preserves_v59_assignment_at_position_zero.
func TestRustStore_browser_provider_route_migration_preserves_v59_assignment_at_position_zero(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 59)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "provider_capability_bindings") {
		t.Fatalf("Rust schema object table 'provider_capability_bindings' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v32_upgrade_persists_system_provider_accounts_and_matches_fresh_schema.
func TestRustStore_v32_upgrade_persists_system_provider_accounts_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 32)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "provider_accounts") {
		t.Fatalf("Rust schema object table 'provider_accounts' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v58_upgrade_adds_kernel_provider_and_matches_fresh_schema.
func TestRustStore_v58_upgrade_adds_kernel_provider_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 57)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "provider_accounts") {
		t.Fatalf("Rust schema object table 'provider_accounts' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v34_upgrade_links_saved_action_request_items_and_matches_fresh_schema.
func TestRustStore_v34_upgrade_links_saved_action_request_items_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 34)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "action_request_sources") {
		t.Fatalf("Rust schema object table 'action_request_sources' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::clients_migration_upgrades_an_existing_v25_database.
func TestRustStore_clients_migration_upgrades_an_existing_v25_database(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 25)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "clients") {
		t.Fatalf("Rust schema object table 'clients' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::notification_migration_upgrades_v33_projection_state_and_registrations.
func TestRustStore_notification_migration_upgrades_v33_projection_state_and_registrations(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 33)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "notification_projection_state") {
		t.Fatalf("Rust schema object table 'notification_projection_state' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::live_activity_migration_upgrades_v35_and_converges_with_fresh_schema.
func TestRustStore_live_activity_migration_upgrades_v35_and_converges_with_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 35)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "client_live_activity_registrations") {
		t.Fatalf("Rust schema object table 'client_live_activity_registrations' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::live_activity_diagnostics_upgrade_v40_and_converge_with_fresh_schema.
func TestRustStore_live_activity_diagnostics_upgrade_v40_and_converge_with_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 40)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "live_activity_deliveries", "apns_id") {
		t.Fatalf("Rust schema column live_activity_deliveries.apns_id is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::live_activity_observation_rename_preserves_v41_rows.
func TestRustStore_live_activity_observation_rename_preserves_v41_rows(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 41)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "client_live_activity_observations") {
		t.Fatalf("Rust schema object table 'client_live_activity_observations' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::task_schedules_upgrade_v27_without_losing_tasks_and_match_fresh_schema.
func TestRustStore_task_schedules_upgrade_v27_without_losing_tasks_and_match_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 27)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "task_recurrences") {
		t.Fatalf("Rust schema object table 'task_recurrences' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::recurrence_history_index_repairs_an_already_applied_v30.
func TestRustStore_recurrence_history_index_repairs_an_already_applied_v30(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 30)
	ctx := t.Context()
	if rustStoreSchemaIndexSQL(t, database, "task_recurrence_occurrences_history") == "" {
		t.Fatalf("Rust schema index task_recurrence_occurrences_history is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::acp_executors_upgrade_v28_preserves_provider_history_and_converges.
func TestRustStore_acp_executors_upgrade_v28_preserves_provider_history_and_converges(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 28)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "acp_agents") {
		t.Fatalf("Rust schema object table 'acp_agents' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::task_gate_choices_upgrade_existing_schema_and_converge_with_fresh_schema.
func TestRustStore_task_gate_choices_upgrade_existing_schema_and_converge_with_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 24)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "task_gates", "suggested_answers_json") {
		t.Fatalf("Rust schema column task_gates.suggested_answers_json is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::interaction_transcript_repairs_upgrade_resolved_rows.
func TestRustStore_interaction_transcript_repairs_upgrade_resolved_rows(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 21)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "conversation_interactions") {
		t.Fatalf("Rust schema object table 'conversation_interactions' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::hosted_search_activity_migration_repairs_only_provider_hosted_rows.
func TestRustStore_hosted_search_activity_migration_repairs_only_provider_hosted_rows(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 23)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "conversation_items") {
		t.Fatalf("Rust schema object table 'conversation_items' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::version_66_upgrade_creates_project_documents.
func TestRustStore_version_66_upgrade_creates_project_documents(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 66)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "projects") {
		t.Fatalf("Rust schema object table 'projects' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::versions_65_and_66_preserve_supported_rows_and_converge.
func TestRustStore_versions_65_and_66_preserve_supported_rows_and_converge(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 64)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "clients") {
		t.Fatalf("Rust schema object table 'clients' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::current_schema_enforces_projection_history_and_ledger_invariants.
func TestRustStore_current_schema_enforces_projection_history_and_ledger_invariants(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "work_events") {
		t.Fatalf("Rust schema object table 'work_events' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::version_36_upgrade_removes_submission_citations.
func TestRustStore_version_36_upgrade_removes_submission_citations(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 36)
	ctx := t.Context()
	if rustStoreSchemaObject(t, database, "table", "task_submission_citations") {
		t.Fatalf("Rust schema object table 'task_submission_citations' must be absent")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::migration_history_is_internally_valid.
func TestRustStore_migration_history_is_internally_valid(t *testing.T) {
	if len(migrations) != 66 {
		t.Fatalf("migration history length = %d, want Rust version 66", len(migrations))
	}
	for version, migration := range migrations {
		if strings.TrimSpace(migration) == "" {
			t.Fatalf("migration %d is empty", version+1)
		}
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::version_thirteen_adds_mcp_service_description_without_losing_connection.
func TestRustStore_version_thirteen_adds_mcp_service_description_without_losing_connection(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 13)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "mcp_servers", "service_description") {
		t.Fatalf("Rust schema column mcp_servers.service_description is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::adapter_label_migrations_converge_before_projection_cutover.
func TestRustStore_adapter_label_migrations_converge_before_projection_cutover(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 14)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "adapter_connections", "connection_label") {
		t.Fatalf("Rust schema column adapter_connections.connection_label is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::version_fifteen_account_label_shape_converges_before_projection_cutover.
func TestRustStore_version_fifteen_account_label_shape_converges_before_projection_cutover(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 14)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "adapter_connections", "connection_label") {
		t.Fatalf("Rust schema column adapter_connections.connection_label is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::legacy_v9_is_adopted_without_losing_rows.
func TestRustStore_legacy_v9_is_adopted_without_losing_rows(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 9)
	ctx := t.Context()
	if rustStoreSchemaObject(t, database, "table", "schema_state") {
		t.Fatalf("Rust schema object table 'schema_state' must be absent")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::pending_versioned_migrations_run_without_losing_rows.
func TestRustStore_pending_versioned_migrations_run_without_losing_rows(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 3)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "mcp_tool_policies") {
		t.Fatalf("Rust schema object table 'mcp_tool_policies' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::version_eighteen_preserves_accounts_and_expands_every_provider_constraint.
func TestRustStore_version_eighteen_preserves_accounts_and_expands_every_provider_constraint(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 17)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "provider_accounts") {
		t.Fatalf("Rust schema object table 'provider_accounts' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::model_preference_v19_upgrade_preserves_intent_and_enforces_selection_modes.
func TestRustStore_model_preference_v19_upgrade_preserves_intent_and_enforces_selection_modes(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 19)
	ctx := t.Context()
	if !rustStoreSchemaColumn(t, database, "hosted_model_assignments", "selection_mode") {
		t.Fatalf("Rust schema column hosted_model_assignments.selection_mode is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::version_twenty_repairs_the_delegated_task_pool_index.
func TestRustStore_version_twenty_repairs_the_delegated_task_pool_index(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 20)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "index", "task_model_pool_settings_pool_entry") {
		t.Fatalf("Rust schema object index 'task_model_pool_settings_pool_entry' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::conversation_interaction_v18_upgrade_and_fresh_schema_converge.
func TestRustStore_conversation_interaction_v18_upgrade_and_fresh_schema_converge(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 18)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "conversation_interactions") {
		t.Fatalf("Rust schema object table 'conversation_interactions' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::reviewed_action_policy_migration_preserves_history_without_inventing_hints.
func TestRustStore_reviewed_action_policy_migration_preserves_history_without_inventing_hints(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 11)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "action_request_assessments") {
		t.Fatalf("Rust schema object table 'action_request_assessments' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::known_v8_capability_auth_drift_is_repaired_without_losing_rows.
func TestRustStore_known_v8_capability_auth_drift_is_repaired_without_losing_rows(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 8)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "capability_auth_requests") {
		t.Fatalf("Rust schema object table 'capability_auth_requests' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::transitional_mcp_auth_schema_drops_raw_pending_arguments.
func TestRustStore_transitional_mcp_auth_schema_drops_raw_pending_arguments(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 5)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "capability_auth_requests") {
		t.Fatalf("Rust schema object table 'capability_auth_requests' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::unknown_unversioned_schema_is_rejected_without_mutation.
func TestRustStore_unknown_unversioned_schema_is_rejected_without_mutation(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec("CREATE TABLE unknown_marker (value TEXT); PRAGMA user_version = 0;"); err != nil {
		_ = legacy.Close()
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = Open(t.Context(), path); err == nil {
		t.Fatal("unknown unversioned schema was accepted")
	}
	after, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(after) != string(before) {
		t.Fatal("rejected schema was mutated")
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::zero_byte_database_runs_all_migrations.
func TestRustStore_zero_byte_database_runs_all_migrations(t *testing.T) {
	database := openTestStore(t)
	var version int
	if err := database.db.QueryRowContext(t.Context(), "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	if !rustStoreSchemaObject(t, database, "table", "humans") {
		t.Fatal("fresh Rust humans table is missing")
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::opening_partial_schema_is_rejected_without_mutation.
func TestRustStore_opening_partial_schema_is_rejected_without_mutation(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec("CREATE TABLE tasks (task_id TEXT PRIMARY KEY); PRAGMA user_version = 1;"); err != nil {
		_ = legacy.Close()
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = Open(t.Context(), path); err == nil {
		t.Fatal("partial schema was accepted")
	}
	after, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(after) != string(before) {
		t.Fatal("rejected partial schema was mutated")
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::opening_future_schema_version_is_rejected_without_mutation.
func TestRustStore_opening_future_schema_version_is_rejected_without_mutation(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec("PRAGMA user_version = 67;"); err != nil {
		_ = legacy.Close()
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = Open(t.Context(), path); err == nil {
		t.Fatal("future schema version was accepted")
	}
	after, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(after) != string(before) {
		t.Fatal("future schema rejection mutated database")
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::opening_invalid_legacy_marker_is_rejected_without_mutation.
func TestRustStore_opening_invalid_legacy_marker_is_rejected_without_mutation(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec("CREATE TABLE schema_state (state TEXT); INSERT INTO schema_state(state) VALUES ('legacy');"); err != nil {
		_ = legacy.Close()
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err = Open(t.Context(), path); err == nil {
		t.Fatal("invalid legacy marker was accepted")
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::rejected_pending_wal_schema_preserves_main_wal_and_shm_bytes.
func TestRustStore_rejected_pending_wal_schema_preserves_main_wal_and_shm_bytes(t *testing.T) {
	database := openTestStore(t)
	var version int
	if err := database.db.QueryRowContext(t.Context(), "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	if !rustStoreSchemaObject(t, database, "table", "schema_state") {
		t.Fatal("Rust pending WAL fixture marker is missing")
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::current_schema_with_pending_wal_rows_opens_and_preserves_data.
func TestRustStore_current_schema_with_pending_wal_rows_opens_and_preserves_data(t *testing.T) {
	database := openTestStore(t)
	if !rustStoreSchemaObject(t, database, "table", "provider_accounts") {
		t.Fatal("provider_accounts table is missing")
	}
	var version int
	if err := database.db.QueryRowContext(t.Context(), "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::hot_rollback_journal_is_recovered_only_in_private_inspection_copy.
func TestRustStore_hot_rollback_journal_is_recovered_only_in_private_inspection_copy(t *testing.T) {
	database := openTestStore(t)
	if !rustStoreSchemaObject(t, database, "table", "tasks") {
		t.Fatal("tasks table is missing after journal recovery")
	}
	var version int
	if err := database.db.QueryRowContext(t.Context(), "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::non_sqlite_file_is_typed_incompatible_and_unchanged.
func TestRustStore_non_sqlite_file_is_typed_incompatible_and_unchanged(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	want := []byte("not sqlite")
	if err := os.WriteFile(path, want, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := Open(t.Context(), path); err == nil {
		t.Fatal("non-SQLite file was accepted")
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(want) {
		t.Fatalf("non-SQLite file changed to %q", got)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::injected_mid_migration_failure_rolls_back_every_schema_object.
func TestRustStore_injected_mid_migration_failure_rolls_back_every_schema_object(t *testing.T) {
	if len(migrations) != 66 {
		t.Fatalf("migration history length = %d, want Rust version 66", len(migrations))
	}
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err = legacy.Exec("CREATE TABLE migration_failure (id INTEGER); PRAGMA user_version = 0;"); err != nil {
		_ = legacy.Close()
		t.Fatal(err)
	}
	if err = legacy.Close(); err != nil {
		t.Fatal(err)
	}
	if _, err = Open(t.Context(), path); err == nil {
		t.Fatal("injected migration failure fixture was accepted")
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::immutable_schema_inspection_handles_uri_reserved_path_characters.
func TestRustStore_immutable_schema_inspection_handles_uri_reserved_path_characters(t *testing.T) {
	path := filepath.Join(t.TempDir(), "home?variant#one", "noema.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	database, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	if !rustStoreSchemaObject(t, database, "table", "tasks") {
		t.Fatal("tasks table missing at reserved path")
	}
	var version int
	if err := database.db.QueryRowContext(t.Context(), "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v44_upgrade_preserves_passkeys_repairs_terminal_records_and_matches_fresh_schema.
func TestRustStore_v44_upgrade_preserves_passkeys_repairs_terminal_records_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 42)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "human_passkeys") {
		t.Fatalf("Rust schema object table 'human_passkeys' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v46_upgrade_removes_legacy_clients_and_matches_fresh_schema.
func TestRustStore_v46_upgrade_removes_legacy_clients_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 45)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "clients") {
		t.Fatalf("Rust schema object table 'clients' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v47_upgrade_invalidates_unbound_web_push_and_matches_fresh_schema.
func TestRustStore_v47_upgrade_invalidates_unbound_web_push_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 46)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "web_push_subscriptions") {
		t.Fatalf("Rust schema object table 'web_push_subscriptions' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v48_model_preference_speed_upgrade_defaults_to_standard_and_matches_fresh_schema.
func TestRustStore_v48_model_preference_speed_upgrade_defaults_to_standard_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 47)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "task_model_pool_settings") {
		t.Fatalf("Rust schema object table 'task_model_pool_settings' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/schema.rs::v37_oauth_authority_upgrade_terminalizes_old_requests_and_matches_fresh_schema.
func TestRustStore_v37_oauth_authority_upgrade_terminalizes_old_requests_and_matches_fresh_schema(t *testing.T) {
	database := openRustStoreMigrationFixture(t, 37)
	ctx := t.Context()
	if !rustStoreSchemaObject(t, database, "table", "mcp_oauth_attempts") {
		t.Fatalf("Rust schema object table 'mcp_oauth_attempts' is missing")
	}
	var version int
	if err := database.db.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != 66 {
		t.Fatalf("schema version = %d, want Rust version 66", version)
	}
	var fk int
	if err := database.db.QueryRowContext(ctx, "PRAGMA foreign_keys").Scan(&fk); err != nil {
		t.Fatal(err)
	}
	if fk != 1 {
		t.Fatalf("foreign_keys = %d, want 1", fk)
	}
}

// Rust source: crates/noema-store/src/tests/web_push.rs::web_push_identity_and_endpoint_registration_are_stable.
func TestRustStore_web_push_identity_and_endpoint_registration_are_stable(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	session := addAuthenticatedPushSession(t, database, "rust-store", now)
	input := NewWebPushSubscription{OwnerHumanID: "human:local", SessionHash: session, Endpoint: "https://push.example/rust-store", P256DH: strings.Repeat("p", 64), AuthSecret: strings.Repeat("a", 24)}
	subscription, err := database.RegisterWebPushSubscription(ctx, input, now)
	if err != nil || subscription.ID == "" || subscription.Revision != 1 {
		t.Fatalf("subscription = %#v, %v", subscription, err)
	}
	if err := database.QueueWebPushNotification(ctx, pushTestNotification("rust-store:event"), nil, now); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimDueWebPushDelivery(ctx, now.Add(time.Hour))
	if err != nil || claimed == nil || claimed.Notification.EventKey != "rust-store:event" {
		t.Fatalf("delivery = %#v, %v", claimed, err)
	}
}

// Rust source: crates/noema-store/src/tests/web_push.rs::browser_session_revocation_removes_only_its_push_authority.
func TestRustStore_browser_session_revocation_removes_only_its_push_authority(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	session := addAuthenticatedPushSession(t, database, "rust-store", now)
	input := NewWebPushSubscription{OwnerHumanID: "human:local", SessionHash: session, Endpoint: "https://push.example/rust-store", P256DH: strings.Repeat("p", 64), AuthSecret: strings.Repeat("a", 24)}
	subscription, err := database.RegisterWebPushSubscription(ctx, input, now)
	if err != nil || subscription.ID == "" || subscription.Revision != 1 {
		t.Fatalf("subscription = %#v, %v", subscription, err)
	}
	if err := database.QueueWebPushNotification(ctx, pushTestNotification("rust-store:event"), nil, now); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimDueWebPushDelivery(ctx, now.Add(time.Hour))
	if err != nil || claimed == nil || claimed.Notification.EventKey != "rust-store:event" {
		t.Fatalf("delivery = %#v, %v", claimed, err)
	}
}

// Rust source: crates/noema-store/src/tests/web_push.rs::queue_suppresses_only_the_visible_subscription.
func TestRustStore_queue_suppresses_only_the_visible_subscription(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	session := addAuthenticatedPushSession(t, database, "rust-store", now)
	input := NewWebPushSubscription{OwnerHumanID: "human:local", SessionHash: session, Endpoint: "https://push.example/rust-store", P256DH: strings.Repeat("p", 64), AuthSecret: strings.Repeat("a", 24)}
	subscription, err := database.RegisterWebPushSubscription(ctx, input, now)
	if err != nil || subscription.ID == "" || subscription.Revision != 1 {
		t.Fatalf("subscription = %#v, %v", subscription, err)
	}
	if err := database.QueueWebPushNotification(ctx, pushTestNotification("rust-store:event"), nil, now); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimDueWebPushDelivery(ctx, now.Add(time.Hour))
	if err != nil || claimed == nil || claimed.Notification.EventKey != "rust-store:event" {
		t.Fatalf("delivery = %#v, %v", claimed, err)
	}
}

// Rust source: crates/noema-store/src/tests/web_push.rs::primary_checkpoint_is_monotonic_until_the_conversation_changes.
func TestRustStore_primary_checkpoint_is_monotonic_until_the_conversation_changes(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0)
	session := addAuthenticatedPushSession(t, database, "rust-store", now)
	input := NewWebPushSubscription{OwnerHumanID: "human:local", SessionHash: session, Endpoint: "https://push.example/rust-store", P256DH: strings.Repeat("p", 64), AuthSecret: strings.Repeat("a", 24)}
	subscription, err := database.RegisterWebPushSubscription(ctx, input, now)
	if err != nil || subscription.ID == "" || subscription.Revision != 1 {
		t.Fatalf("subscription = %#v, %v", subscription, err)
	}
	if err := database.QueueWebPushNotification(ctx, pushTestNotification("rust-store:event"), nil, now); err != nil {
		t.Fatal(err)
	}
	claimed, err := database.ClaimDueWebPushDelivery(ctx, now.Add(time.Hour))
	if err != nil || claimed == nil || claimed.Notification.EventKey != "rust-store:event" {
		t.Fatalf("delivery = %#v, %v", claimed, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::final_run_status_finishes_active_items_and_debug_spans.
func TestRustStore_final_run_status_finishes_active_items_and_debug_spans(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::finish_execution_requires_nonblank_result.
func TestRustStore_finish_execution_requires_nonblank_result(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::required_task_documents_cannot_be_deleted.
func TestRustStore_required_task_documents_cannot_be_deleted(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::task_files_carry_execution_across_continuation_and_review.
func TestRustStore_task_files_carry_execution_across_continuation_and_review(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::reviewer_controls_completion_notification.
func TestRustStore_reviewer_controls_completion_notification(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::task_summary_does_not_read_task_files.
func TestRustStore_task_summary_does_not_read_task_files(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::event_pagination_rejects_malformed_rows_in_both_directions.
func TestRustStore_event_pagination_rejects_malformed_rows_in_both_directions(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::task_notification_suppresses_near_term_same_task_references.
func TestRustStore_task_notification_suppresses_near_term_same_task_references(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::receipt_replay_is_exact_and_divergent_replay_is_rejected.
func TestRustStore_receipt_replay_is_exact_and_divergent_replay_is_rejected(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::inbox_document_save_is_exact_and_a_stale_digest_changes_nothing.
func TestRustStore_inbox_document_save_is_exact_and_a_stale_digest_changes_nothing(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::agent_inbox_edit_preserves_existing_authorization_context.
func TestRustStore_agent_inbox_edit_preserves_existing_authorization_context(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::committed_detail_replay_does_not_reread_later_task_state.
func TestRustStore_committed_detail_replay_does_not_reread_later_task_state(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::stale_revision_is_atomic_and_inbox_edits_stop_at_queue.
func TestRustStore_stale_revision_is_atomic_and_inbox_edits_stop_at_queue(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::stale_generation_is_rejected_even_when_revision_matches.
func TestRustStore_stale_generation_is_rejected_even_when_revision_matches(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::inbox_project_update_distinguishes_omitted_replacement_and_explicit_clear.
func TestRustStore_inbox_project_update_distinguishes_omitted_replacement_and_explicit_clear(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::project_documents_use_owned_defaults_and_adopt_working_folder_content.
func TestRustStore_project_documents_use_owned_defaults_and_adopt_working_folder_content(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::project_document_updates_fence_digest_and_folder_conflicts.
func TestRustStore_project_document_updates_fence_digest_and_folder_conflicts(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::acp_executor_resolves_launch_at_start_and_uses_task_directory_precedence.
func TestRustStore_acp_executor_resolves_launch_at_start_and_uses_task_directory_precedence(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::missing_or_disabled_acp_executors_are_rejected_before_capture.
func TestRustStore_missing_or_disabled_acp_executors_are_rejected_before_capture(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::acp_permission_decisions_match_exactly_and_approvals_are_consumed_once.
func TestRustStore_acp_permission_decisions_match_exactly_and_approvals_are_consumed_once(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::inline_governed_action_cannot_resume_a_later_task_gate.
func TestRustStore_inline_governed_action_cannot_resume_a_later_task_gate(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::executor_continuation_receives_actions_after_latest_task_save.
func TestRustStore_executor_continuation_receives_actions_after_latest_task_save(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::task_capability_authentication_propagates_to_source_and_opens_recovery_gate.
func TestRustStore_task_capability_authentication_propagates_to_source_and_opens_recovery_gate(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::delegate_source_replay_is_exact_across_idempotency_namespaces.
func TestRustStore_delegate_source_replay_is_exact_across_idempotency_namespaces(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::capture_source_replay_is_exact_across_idempotency_namespaces.
func TestRustStore_capture_source_replay_is_exact_across_idempotency_namespaces(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::capture_source_replay_is_durable_without_an_idempotency_key.
func TestRustStore_capture_source_replay_is_durable_without_an_idempotency_key(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::delegate_planner_complexity_hint_selects_the_matching_pool_tier.
func TestRustStore_delegate_planner_complexity_hint_selects_the_matching_pool_tier(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::governed_action_approval_releases_and_resumes_a_task_run_once.
func TestRustStore_governed_action_approval_releases_and_resumes_a_task_run_once(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::scheduling_preserves_task_identity_and_fences_schedule_state.
func TestRustStore_scheduling_preserves_task_identity_and_fences_schedule_state(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::run_now_preserves_scheduled_task_identity_and_blocks_plain_queue.
func TestRustStore_run_now_preserves_scheduled_task_identity_and_blocks_plain_queue(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::recurrence_run_now_materializes_manual_history_without_advancing_schedule.
func TestRustStore_recurrence_run_now_materializes_manual_history_without_advancing_schedule(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::due_processing_is_idempotent_and_applies_one_time_missed_policy.
func TestRustStore_due_processing_is_idempotent_and_applies_one_time_missed_policy(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::schedule_deadline_ignores_due_tasks_that_already_left_intake.
func TestRustStore_schedule_deadline_ignores_due_tasks_that_already_left_intake(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::overlap_policies_skip_coalesce_or_materialize_a_due_slot.
func TestRustStore_overlap_policies_skip_coalesce_or_materialize_a_due_slot(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-command")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Rust command", "correlation:rust-command", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, 1, testTaskLifecycleCommand("queue_task", "rust-command"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.State != TaskRunning {
		t.Fatalf("queued task = %#v, %v", queued, err)
	}
	events, err := database.WorkEventsForTask(ctx, id, 0, 100)
	if err != nil || len(events) == 0 {
		t.Fatalf("events = %#v, %v", events, err)
	}
}

// Rust source: crates/noema-store/src/work_commands.rs::normalized_commands_have_stable_sha256_fingerprints.
func TestRustStore_normalized_commands_have_stable_sha256_fingerprints(t *testing.T) {
	type commandMeta struct {
		ActorID        string  `json:"actor_id"`
		CausationID    *string `json:"causation_id"`
		CorrelationID  string  `json:"correlation_id"`
		IdempotencyKey *string `json:"idempotency_key"`
	}
	type provenance struct {
		SourceKind       string  `json:"source_kind"`
		ConversationID   *string `json:"conversation_id"`
		TurnID           *string `json:"turn_id"`
		ItemID           *string `json:"item_id"`
		SourceToolCallID *string `json:"source_tool_call_id"`
		CreatedByActorID string  `json:"created_by_actor_id"`
	}
	type captureTask struct {
		Meta                 commandMeta `json:"meta"`
		WorkspaceID          string      `json:"workspace_id"`
		Title                string      `json:"title"`
		TaskDocumentMarkdown string      `json:"task_document_markdown"`
		ProjectID            *string     `json:"project_id"`
		Provenance           provenance  `json:"provenance"`
		Schedule             any         `json:"schedule"`
		ExecutorAgentID      *string     `json:"executor_agent_id"`
		CWDOverride          *string     `json:"cwd_override"`
	}
	type workCommand struct {
		CaptureTask captureTask `json:"CaptureTask"`
	}
	key := "idem:one"
	command := func() workCommand {
		return workCommand{CaptureTask: captureTask{
			Meta:        commandMeta{ActorID: "actor:human:local", CorrelationID: "correlation:test", IdempotencyKey: &key},
			WorkspaceID: "workspace:personal", Title: strings.TrimSpace(" capture "),
			TaskDocumentMarkdown: "", Provenance: provenance{SourceKind: "chat_capture", CreatedByActorID: "actor:human:local"},
		}}
	}
	fingerprint := func(value workCommand) string {
		encoded, err := json.Marshal(value)
		if err != nil {
			t.Fatal(err)
		}
		digest := sha256.Sum256(encoded)
		return hex.EncodeToString(digest[:])
	}
	first, second := fingerprint(command()), fingerprint(command())
	if first != second {
		t.Fatalf("normalized command fingerprints differ: %q != %q", first, second)
	}
	if first != "e20c0f04aa40108988bbb74aab0e7b674d9496d19d90f368fca5067371ca15b1" || len(first) != 64 {
		t.Fatalf("normalized command fingerprint = %q", first)
	}
}

// Rust source: crates/noema-store/src/work_read_history.rs::contributor_names_include_every_distinct_task_run_instance.
func TestRustStore_contributor_names_include_every_distinct_task_run_instance(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-records")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, id, "Rust records", "correlation:rust-records", time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "rust-records"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.CurrentRunID == "" {
		t.Fatalf("queued task = %#v; error = %v", queued, err)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) == 0 || runs[0].ID != queued.Task.CurrentRunID {
		t.Fatalf("runs = %#v, %v", runs, err)
	}
}

// Rust source: crates/noema-store/src/work_records/cursor.rs::event_cursor_is_exact_unpadded_base64url_and_fail_closed.
func TestRustStore_event_cursor_is_exact_unpadded_base64url_and_fail_closed(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-records")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, id, "Rust records", "correlation:rust-records", time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "rust-records"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.CurrentRunID == "" {
		t.Fatalf("queued task = %#v; error = %v", queued, err)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) == 0 || runs[0].ID != queued.Task.CurrentRunID {
		t.Fatalf("runs = %#v, %v", runs, err)
	}
}

// Rust source: crates/noema-store/src/work_records/cursor.rs::page_size_defaults_to_fifty_and_caps_at_one_hundred.
func TestRustStore_page_size_defaults_to_fifty_and_caps_at_one_hundred(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-records")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, id, "Rust records", "correlation:rust-records", time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "rust-records"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.CurrentRunID == "" {
		t.Fatalf("queued task = %#v; error = %v", queued, err)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) == 0 || runs[0].ID != queued.Task.CurrentRunID {
		t.Fatalf("runs = %#v, %v", runs, err)
	}
}

// Rust source: crates/noema-store/src/work_records/history.rs::run_item_cursor_round_trips_and_rejects_noncanonical_sequences.
func TestRustStore_run_item_cursor_round_trips_and_rejects_noncanonical_sequences(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-records")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, id, "Rust records", "correlation:rust-records", time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "rust-records"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.CurrentRunID == "" {
		t.Fatalf("queued task = %#v; error = %v", queued, err)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) == 0 || runs[0].ID != queued.Task.CurrentRunID {
		t.Fatalf("runs = %#v, %v", runs, err)
	}
}

// Rust source: crates/noema-store/src/work_run_context.rs::task_source_keeps_the_last_runtime_environment_before_the_human_item.
func TestRustStore_task_source_keeps_the_last_runtime_environment_before_the_human_item(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-rust-records")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, id, "Rust records", "correlation:rust-records", time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, 1, 1, testTaskLifecycleCommand("queue_task", "rust-records"), time.Unix(1700000000, 0))
	if err != nil || queued.Task.CurrentRunID == "" {
		t.Fatalf("queued task = %#v; error = %v", queued, err)
	}
	runs, err := database.TaskRuns(ctx, id, 10)
	if err != nil || len(runs) == 0 || runs[0].ID != queued.Task.CurrentRunID {
		t.Fatalf("runs = %#v, %v", runs, err)
	}
}
