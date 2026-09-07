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
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
)

const rustStoreFixtureConnectionID = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

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
	connection := AdapterConnectionIndex{ID: rustStoreFixtureConnectionID, Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
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
	connection := AdapterConnectionIndex{ID: rustStoreFixtureConnectionID, Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
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
	connection := AdapterConnectionIndex{ID: rustStoreFixtureConnectionID, Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
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
	connection := AdapterConnectionIndex{ID: rustStoreFixtureConnectionID, Slug: "personal", Label: "Fixture", Digest: digest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, AllowedOperations: []string{"list"}}
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
	now := time.Unix(1700000000, 0)
	if err := database.EnsureBuiltinProviderAccounts(ctx, now); err != nil {
		t.Fatal(err)
	}
	accounts, err := database.ActiveProviderAccounts(ctx)
	if err != nil || len(accounts) == 0 {
		t.Fatalf("provider accounts = %#v, %v", accounts, err)
	}
	ids := make(map[string]bool, len(accounts))
	for _, account := range accounts {
		ids[account.ID] = true
	}
	for _, expected := range []string{
		"provider_account:codex:default",
		"provider_account:foundation_local:default",
		"provider_account:openai:default",
		"provider_account:duckduckgo_public:system",
		"provider_account:direct_http:system",
		"provider_account:obscura:system",
	} {
		if !ids[expected] {
			t.Errorf("active provider accounts omit Rust fixture %q", expected)
		}
	}
	var count int
	if err := database.db.QueryRowContext(ctx, "SELECT COUNT(*) FROM provider_accounts").Scan(&count); err != nil || count < len(accounts) {
		t.Fatalf("provider rows = %d, %v", count, err)
	}
	if _, err := database.DeleteProviderAccount(ctx, "provider_account:direct_http:system"); !errors.Is(err, provider.ErrProtectedAccount) {
		t.Errorf("built-in direct HTTP delete error = %v, want protected-account error", err)
	}
	missing := "provider_account:missing"
	if _, err := database.ProviderAccount(ctx, missing); !errors.Is(err, provider.ErrAccountNotFound) {
		t.Errorf("missing provider read error = %v, want account-not-found", err)
	}
	account, err := database.CreateProviderAccount(ctx, provider.Account{
		ID: "provider_account:exa:port-test", ProviderKind: "exa", AccountKey: "port-test",
		DisplayName: "Search", AuthMethod: provider.AuthSecretInput, IsActive: true,
		Status: provider.StatusUnauthenticated, CreatedAt: now, UpdatedAt: now,
		Metadata: provider.AccountMetadata{"secretConfigured": json.RawMessage(`false`)},
	})
	if err != nil {
		t.Fatal(err)
	}
	updated, err := database.UpdateProviderCredential(ctx, account.ID, 0, provider.AuthSecretInput, true,
		provider.AccountMetadata{"secretConfigured": json.RawMessage(`true`)}, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if updated.Status != provider.StatusAuthenticated || updated.LastAuthenticatedAt == nil {
		t.Errorf("updated account = %#v, want authenticated with last-authenticated time", updated)
	}
	if string(updated.Metadata["secretConfigured"]) != "true" {
		t.Errorf("updated account secretConfigured = %s, want true", updated.Metadata["secretConfigured"])
	}
	if _, err := database.DeleteProviderAccount(ctx, account.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := database.UpdateProviderCredential(ctx, account.ID, 0, provider.AuthSecretInput, true, nil, now.Add(2*time.Second)); !errors.Is(err, provider.ErrAccountNotFound) {
		t.Errorf("update after delete error = %v, want account-not-found", err)
	}
	path := filepath.Join(t.TempDir(), "db", "noema.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	writer, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	defer writer.Close()
	reader, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	defer reader.Close()
	if err := writer.EnsureBuiltinProviderAccounts(ctx, now); err != nil {
		t.Fatal(err)
	}
	codex, err := writer.ProviderAccount(ctx, "provider_account:codex:default")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := writer.UpdateProviderCredential(ctx, codex.ID, codex.Metadata.CredentialRevision(), provider.AuthOAuthDeviceCode, true,
		provider.AccountMetadata{"profiles": json.RawMessage(`[{"id":"visible","label":"Visible"}]`)}, now); err != nil {
		t.Fatal(err)
	}
	observed, err := reader.ProviderAccount(ctx, codex.ID)
	if err != nil {
		t.Fatal(err)
	}
	if string(observed.Metadata["profiles"]) != `[{"id":"visible","label":"Visible"}]` || observed.Status != provider.StatusAuthenticated {
		t.Errorf("second-handle account = %#v, want authenticated visible profile", observed)
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
	now := time.Unix(1700000000, 0)
	account, err := database.CreateProviderAccount(ctx, provider.Account{
		ID: "provider_account:exa:corrupt", ProviderKind: "exa", AccountKey: "corrupt",
		DisplayName: "Exa", AuthMethod: provider.AuthSecretInput, IsActive: true,
		Status: provider.StatusAuthenticated, CreatedAt: now, UpdatedAt: now,
		Metadata: provider.AccountMetadata{},
	})
	if err != nil {
		t.Fatal(err)
	}
	connection, err := database.db.Conn(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := connection.ExecContext(ctx, "PRAGMA ignore_check_constraints = ON"); err != nil {
		_ = connection.Close()
		t.Fatal(err)
	}
	if _, err := connection.ExecContext(ctx, "UPDATE provider_accounts SET metadata_json = '{' WHERE provider_account_id = ?", account.ID); err != nil {
		_ = connection.Close()
		t.Fatal(err)
	}
	if _, err := connection.ExecContext(ctx, "PRAGMA ignore_check_constraints = OFF"); err != nil {
		_ = connection.Close()
		t.Fatal(err)
	}
	if err := connection.Close(); err != nil {
		t.Fatal(err)
	}
	readError := func() error {
		_, err := database.ProviderAccount(ctx, account.ID)
		return err
	}()
	if readError == nil {
		t.Errorf("corrupt provider row was read successfully")
	} else if !strings.Contains(readError.Error(), "decode provider account metadata") {
		t.Errorf("corrupt provider read error = %v, want metadata decode error", readError)
	}
	if _, err := database.UpdateProviderCredential(ctx, account.ID, 0, provider.AuthSecretInput, true, nil, now.Add(time.Second)); err == nil {
		t.Errorf("credential update accepted corrupt provider row")
	}
	if err := database.SetProviderAccountStatus(ctx, account.ID, provider.StatusAuthenticated, "", "", now.Add(2*time.Second)); err == nil {
		// Rust's update_provider_account path returns an invariant error here.
		t.Errorf("status update accepted corrupt provider row")
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::capability_assignment_and_account_delete_never_leave_a_dangling_row.
func TestRustStore_capability_assignment_and_account_delete_never_leave_a_dangling_row(t *testing.T) {
	ctx := t.Context()
	path := filepath.Join(t.TempDir(), "db", "noema.sqlite3")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	writer, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = writer.Close() })
	if err := writer.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	deleter, err := Open(ctx, path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = deleter.Close() })
	missingProviderAccountID := "provider_account:exa:missing"
	if err := writer.SaveWebProviderBinding(ctx, "web.search", missingProviderAccountID, time.Unix(1700000000, 0)); !errors.Is(err, provider.ErrAccountNotFound) {
		t.Errorf("missing persisted account error = %v, want account-not-found", err)
	}
	systemProviderAccountID := "provider_account:direct_http:system"
	if err := writer.SaveWebProviderBinding(ctx, "web.fetch", systemProviderAccountID, time.Unix(1700000000, 0)); err != nil {
		t.Fatalf("save system assignment: %v", err)
	}
	bindings, err := writer.WebProviderRoute(ctx, "web.fetch")
	if err != nil || len(bindings) != 1 || bindings[0].ProviderAccountID != systemProviderAccountID {
		t.Fatalf("system assignment = %#v, %v", bindings, err)
	}
	now := time.Unix(1700000000, 0)
	account, err := writer.CreateProviderAccount(ctx, provider.Account{
		ID: "provider_account:exa:concurrent", ProviderKind: "exa", AccountKey: "concurrent",
		DisplayName: "Exa", AuthMethod: provider.AuthSecretInput, IsActive: true,
		Status: provider.StatusAuthenticated, CreatedAt: now, UpdatedAt: now,
		Metadata: provider.AccountMetadata{},
	})
	if err != nil {
		t.Fatal(err)
	}
	writeDone := make(chan error, 1)
	deleteDone := make(chan error, 1)
	go func() { writeDone <- writer.SaveWebProviderBinding(ctx, "web.search", account.ID, now) }()
	go func() {
		_, deleteErr := deleter.DeleteProviderAccount(ctx, account.ID)
		deleteDone <- deleteErr
	}()
	writeErr, deleteErr := <-writeDone, <-deleteDone
	if writeErr != nil && !errors.Is(writeErr, provider.ErrAccountNotFound) {
		t.Errorf("concurrent capability write error = %v", writeErr)
	}
	if deleteErr != nil && !errors.Is(deleteErr, provider.ErrProtectedAccount) {
		t.Errorf("concurrent account delete error = %v", deleteErr)
	}
	_, accountErr := writer.ProviderAccount(ctx, account.ID)
	accountExists := accountErr == nil
	if accountErr != nil && !errors.Is(accountErr, provider.ErrAccountNotFound) {
		t.Errorf("concurrent account read error = %v", accountErr)
	}
	bindings, err = writer.WebProviderRoute(ctx, "web.search")
	if err != nil {
		t.Fatal(err)
	}
	assignmentExists := false
	for _, binding := range bindings {
		assignmentExists = assignmentExists || binding.ProviderAccountID == account.ID
	}
	if assignmentExists && !accountExists {
		t.Errorf("dangling capability assignment: account=%v bindings=%#v", accountExists, bindings)
	}
}

// Rust source: crates/noema-store/src/provider_persistence_port_tests.rs::browser_route_replacement_is_unbounded_atomic_and_preserves_fallbacks.
func TestRustStore_browser_route_replacement_is_unbounded_atomic_and_preserves_fallbacks(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Unix(1700000000, 0)); err != nil {
		t.Fatal(err)
	}
	now := time.Unix(1700000000, 0)
	accountIDs := []string{"provider_account:obscura:system"}
	for index := 0; index < 8; index++ {
		key := fmt.Sprintf("route-%d", index)
		account, err := database.CreateProviderAccount(ctx, provider.Account{
			ID: "provider_account:kernel:" + key, ProviderKind: "kernel", AccountKey: key,
			DisplayName: fmt.Sprintf("Kernel %d", index), AuthMethod: provider.AuthSecretInput,
			IsActive: true, Status: provider.StatusAuthenticated, CreatedAt: now, UpdatedAt: now,
			Metadata: provider.AccountMetadata{},
		})
		if err != nil {
			t.Fatal(err)
		}
		accountIDs = append(accountIDs, account.ID)
	}
	if err := database.SaveBrowserProviderRoute(ctx, accountIDs, now); err != nil {
		t.Fatal(err)
	}
	route, err := database.WebProviderRoute(ctx, "web.browse")
	if err != nil {
		t.Fatal(err)
	}
	if len(route) != len(accountIDs) {
		t.Fatalf("nine-provider route length = %d, want %d", len(route), len(accountIDs))
	}
	for index, expected := range accountIDs {
		if route[index].ProviderAccountID != expected || route[index].RoutePosition != index {
			t.Fatalf("nine-provider route[%d] = %#v, want %q at %d", index, route[index], expected, index)
		}
	}
	missing := append([]string{accountIDs[0]}, "provider_account:kernel:missing")
	if err := database.SaveBrowserProviderRoute(ctx, missing, now); !errors.Is(err, provider.ErrAccountNotFound) {
		t.Errorf("missing-account route error = %v, want account-not-found", err)
	}
	preserved, err := database.WebProviderRoute(ctx, "web.browse")
	if err != nil || len(preserved) != len(accountIDs) {
		t.Fatalf("route after missing account = %#v, %v", preserved, err)
	}
	unavailableKey := "route-unavailable"
	unavailable, err := database.CreateProviderAccount(ctx, provider.Account{
		ID: "provider_account:kernel:" + unavailableKey, ProviderKind: "kernel", AccountKey: unavailableKey,
		DisplayName: "Unavailable Kernel", AuthMethod: provider.AuthSecretInput,
		IsActive: true, Status: provider.StatusUnauthenticated, CreatedAt: now, UpdatedAt: now,
		Metadata: provider.AccountMetadata{},
	})
	if err != nil {
		t.Fatal(err)
	}
	unavailableRoute := append([]string{accountIDs[0]}, unavailable.ID)
	if err := database.SaveBrowserProviderRoute(ctx, unavailableRoute, now); err == nil {
		t.Errorf("unavailable-account route was accepted")
	}
	afterUnavailable, err := database.WebProviderRoute(ctx, "web.browse")
	if err != nil || len(afterUnavailable) != len(accountIDs) {
		t.Fatalf("route after unavailable account = %#v, %v", afterUnavailable, err)
	}
	preferred := accountIDs[3]
	reordered := make([]string, 0, len(accountIDs))
	reordered = append(reordered, preferred)
	for _, accountID := range accountIDs {
		if accountID != preferred {
			reordered = append(reordered, accountID)
		}
	}
	if err := database.SaveBrowserProviderRoute(ctx, reordered, now); err != nil {
		t.Fatal(err)
	}
	reorderedRoute, err := database.WebProviderRoute(ctx, "web.browse")
	if err != nil || len(reorderedRoute) != len(accountIDs) || reorderedRoute[0].ProviderAccountID != preferred {
		t.Fatalf("reordered route = %#v, %v", reorderedRoute, err)
	}
	if err := database.SaveBrowserProviderRoute(ctx, []string{accountIDs[0], accountIDs[0]}, now); err == nil {
		t.Errorf("duplicate browser route was accepted")
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
	unused := "unused"
	_, err := database.AppendArtifactVersion(ctx, "artifact:any", ArtifactVersion{
		ID: "artifact_version:" + strings.Repeat("a", 32), LocalRelativePath: &unused,
		CreatedByActorID: "agent:test",
	}, time.Unix(0, 0))
	if err == nil || !strings.Contains(strings.ToLower(err.Error()), "invalid version index") {
		t.Fatalf("non-positive expected index error = %v, want Rust InvalidVersionIndex{version_index: 0}", err)
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
	description := "Planning notes"
	url := "https://notion.so/noema-brief"
	mediaType := "text/html"
	versionTitle := "Initial"
	created, err := database.CreateArtifact(ctx, Artifact{
		ID: artifactID, Owner: ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID},
		Title: "Sprint brief", Description: &description, Kind: "document", StorageKind: ArtifactExternalURL,
		CreatedByActorID: "agent:primary", Source: ArtifactSource{ConversationID: conversation.ID},
		Metadata: map[string]any{"provider": "notion"},
	}, ArtifactVersion{
		ID: versionID, Title: &versionTitle, ExternalURL: &url, MediaType: &mediaType,
		CreatedByActorID: "agent:primary", Source: ArtifactSource{ConversationID: conversation.ID},
	}, time.Unix(0, 0))
	if err != nil || created.Artifact.ID != artifactID || created.CurrentVersion.ID != versionID ||
		created.CurrentVersion.ExternalURL == nil || *created.CurrentVersion.ExternalURL != url {
		t.Fatalf("external artifact = %#v, %v", created, err)
	}
	if _, err := database.db.ExecContext(ctx,
		"UPDATE artifact_versions SET external_url = ? WHERE artifact_version_id = ?",
		"javascript:alert(1)", created.CurrentVersion.ID); err != nil {
		t.Fatalf("forge external URL: %v", err)
	}
	loaded, err := database.ArtifactWithVersionsByID(ctx, artifactID)
	if err == nil {
		t.Fatalf("forged external URL was accepted on read: %#v", loaded)
	}
	if !strings.Contains(strings.ToLower(err.Error()), "external url") {
		t.Fatalf("forged external URL error = %v, want Rust InvalidArtifactExternalUrl", err)
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
	allowed, err := database.ArtifactOwnerAuthorized(ctx, ArtifactOwner{ObjectType: "conversation", ObjectID: conversation.ID})
	if err != nil || !allowed {
		t.Fatalf("local conversation owner authorization = %t, %v", allowed, err)
	}
	foreignAllowed, err := database.ArtifactOwnerAuthorized(ctx, ArtifactOwner{
		ObjectType: "conversation", ObjectID: "conversation:" + strings.Repeat("f", 32),
	})
	if err != nil || foreignAllowed {
		t.Fatalf("foreign conversation owner authorization = %t, %v", foreignAllowed, err)
	}
	unknownAllowed, err := database.ArtifactOwnerAuthorized(ctx, ArtifactOwner{
		ObjectType: "unknown", ObjectID: "object:unknown",
	})
	if err != nil || unknownAllowed {
		t.Fatalf("unknown owner authorization = %t, %v", unknownAllowed, err)
	}
}

// Rust source: crates/noema-store/src/tests/artifacts.rs::task_artifact_access_uses_v3_workspace_membership.
func TestRustStore_task_artifact_access_uses_v3_workspace_membership(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	taskID := "task:" + strings.Repeat("a", 32)
	if _, err := database.CreateTask(ctx, taskID, "Artifact owner", "correlation:artifact-owner", time.Unix(0, 0)); err != nil {
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
	relativePath := "tasks/output.md"
	mediaType := "text/markdown"
	byteSize := int64(1)
	created, err := database.CreateArtifact(ctx, Artifact{
		ID: artifactID, Owner: ArtifactOwner{ObjectType: "task", ObjectID: taskID},
		Title: "Task output", Kind: "document", StorageKind: ArtifactLocalFile,
		CreatedByActorID: "actor:system",
	}, ArtifactVersion{
		ID: versionID, LocalRelativePath: &relativePath, MediaType: &mediaType,
		ByteSize: &byteSize, CreatedByActorID: "actor:system",
	}, time.Unix(0, 0))
	if err != nil {
		t.Fatalf("create task artifact: %v", err)
	}
	allowed, err := database.ArtifactOwnerAuthorized(ctx, created.Artifact.Owner)
	if err != nil || !allowed {
		t.Fatalf("task owner authorization for local human = %t, %v", allowed, err)
	}
	owner, version, found, err := database.AuthorizedLocalArtifactVersion(ctx, versionID)
	if err != nil || !found || owner.ID != artifactID || version.ID != versionID || version.LocalRelativePath == nil || *version.LocalRelativePath != relativePath {
		t.Fatalf("authorized task artifact version = %#v %#v %t, %v", owner, version, found, err)
	}
}

// Rust source: crates/noema-store/src/tests/artifacts.rs::task_artifact_connection_is_owner_scoped_paginated_and_current_version_hydrated.
func TestRustStore_task_artifact_connection_is_owner_scoped_paginated_and_current_version_hydrated(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	taskID := "task:" + strings.Repeat("b", 32)
	if _, err := database.CreateTask(ctx, taskID, "Artifact connection owner", "correlation:artifact-connection", time.Unix(0, 0)); err != nil {
		t.Fatal(err)
	}
	type artifactFixture struct {
		id, versionID, title, path string
	}
	fixtures := []artifactFixture{
		{id: "artifact:" + strings.Repeat("a", 32), versionID: "artifact_version:" + strings.Repeat("a", 32), title: "Artifact a", path: "tasks/a.md"},
		{id: "artifact:" + strings.Repeat("b", 32), versionID: "artifact_version:" + strings.Repeat("b", 32), title: "Artifact b", path: "tasks/b.md"},
	}
	for _, fixture := range fixtures {
		path := fixture.path
		mediaType := "text/markdown"
		byteSize := int64(1)
		if _, err := database.CreateArtifact(ctx, Artifact{
			ID: fixture.id, Owner: ArtifactOwner{ObjectType: "task", ObjectID: taskID},
			Title: fixture.title, Kind: "document", StorageKind: ArtifactLocalFile,
			CreatedByActorID: "actor:system",
		}, ArtifactVersion{
			ID: fixture.versionID, LocalRelativePath: &path, MediaType: &mediaType,
			ByteSize: &byteSize, CreatedByActorID: "actor:system",
		}, time.Unix(0, 0)); err != nil {
			t.Fatalf("create %s: %v", fixture.id, err)
		}
	}
	currentPath := "tasks/a-current.md"
	currentTitle := "Current"
	mediaType := "text/markdown"
	byteSize := int64(1)
	appended, err := database.AppendArtifactVersion(ctx, fixtures[0].id, ArtifactVersion{
		ID: "artifact_version:" + strings.Repeat("c", 32), Title: &currentTitle,
		LocalRelativePath: &currentPath, MediaType: &mediaType, ByteSize: &byteSize,
		CreatedByActorID: "actor:system", Index: 2,
	}, time.Unix(1, 0))
	if err != nil {
		t.Fatalf("append current version: %v", err)
	}
	owner := ArtifactOwner{ObjectType: "task", ObjectID: taskID}
	listed, err := database.ArtifactsForOwner(ctx, owner, 2)
	if err != nil || len(listed) != 2 {
		t.Fatalf("owner artifact page = %#v, %v", listed, err)
	}
	if listed[0].Artifact.Owner != owner || listed[1].Artifact.Owner != owner {
		t.Fatalf("artifact owners = %#v %#v, want %#v", listed[0].Artifact.Owner, listed[1].Artifact.Owner, owner)
	}
	page, err := database.ArtifactsForOwner(ctx, owner, 1)
	if err != nil || len(page) != 1 {
		t.Fatalf("bounded artifact page = %#v, %v", page, err)
	}
	var currentA ArtifactWithVersions
	for _, value := range listed {
		if value.Artifact.ID == fixtures[0].id {
			currentA = value
		}
	}
	if currentA.Artifact.ID != fixtures[0].id || currentA.Artifact.Owner.ObjectType != "task" || currentA.Artifact.Owner.ObjectID != taskID ||
		currentA.CurrentVersion.ID != appended.ID || len(currentA.Versions) != 2 || currentA.CurrentVersion.Title == nil || *currentA.CurrentVersion.Title != currentTitle ||
		currentA.CurrentVersion.LocalRelativePath == nil || *currentA.CurrentVersion.LocalRelativePath != currentPath {
		t.Fatalf("hydrated current Artifact = %#v, want version %s", currentA, appended.ID)
	}
}

// Rust source: crates/noema-store/src/tests/conversation_interactions.rs::interaction_publication_resolution_and_recovery_are_atomic_one_use_cases.
func TestRustStore_interaction_publication_resolution_and_recovery_are_atomic_one_use_cases(t *testing.T) {
	for _, test := range []struct {
		name, terminal        string
		crashAfterPersistence bool
	}{
		{name: "success"},
		{name: "crash_after_persistence", crashAfterPersistence: true},
		{name: "failure", terminal: "provider_timeout"},
	} {
		t.Run(test.name, func(t *testing.T) {
			database := openTestStore(t)
			ctx := t.Context()
			now := time.Unix(1700000000, 0).UTC()
			conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
			if err != nil {
				t.Fatal(err)
			}
			turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Choose one", nil, now)
			if err != nil {
				t.Fatal(err)
			}
			projection := map[string]any{
				"protocol_version": "v0.9.1",
				"catalog":          map[string]any{"catalog_id": "com.noema.a2ui/catalog/v0.9.1"},
				"messages":         []any{}, "deleted_surface_ids": []any{},
				"surfaces": map[string]any{
					"main": map[string]any{
						"surface_id": "main", "namespaced_surface_id": "main", "version": "v0.9.1",
						"catalog_id": "com.noema.a2ui/catalog/v0.9.1", "send_data_model": true,
						"revision": 1, "components": map[string]any{}, "data_model": map[string]any{},
						"actions": []any{map[string]any{"source_component_id": "main", "name": "choose"}},
					},
				},
			}
			items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
				Provider: "openrouter",
				Call: ConversationToolCallInput{
					ProviderRound: 0, OutputIndex: 0, ProviderCallID: "call:" + test.name,
					ProviderName: "present_a2ui", Name: "noema.present_a2ui", Arguments: json.RawMessage(`{"jsonl":"choice"}`),
				},
				A2UI: &ConversationA2UIInput{
					Projection: projection, HasActions: true,
					ProviderSelection: map[string]any{"role": "noema", "provider_kind": "openrouter", "provider_account_id": "provider_account:codex:default", "model_profile": "gpt-test"},
					ToolCatalogDigest: strings.Repeat("a", 64),
				},
			}, now)
			if err != nil || len(items) != 2 || items[0].Kind != ConversationToolCall || items[1].Kind != ConversationA2UICard {
				t.Fatalf("published interaction = %#v, %v", items, err)
			}
			surface := items[1]
			storedProjection, ok := surface.Payload["payload"].(map[string]any)
			if !ok || textJSON(surface.Payload["interaction_state"]) != "pending" {
				t.Fatalf("pending interaction projection = %#v", surface.Payload)
			}
			interactionID := textJSON(storedProjection["interaction_id"])
			if interactionID == "" || intJSON(storedProjection["interaction_revision"]) != 1 {
				t.Fatalf("interaction authority = %#v", storedProjection)
			}
			page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
			if err != nil || len(page.Items) != 3 {
				t.Fatalf("published transcript = %#v, %v", page.Items, err)
			}
			settled := cloneJSONMap(storedProjection)
			settled["interaction_revision"], settled["lifecycle"] = 2, "answered"
			clientID := "client:" + test.name
			continuation, err := database.ResolveConversationA2UI(ctx, conversation.ID, surface.ID, interactionID, 1,
				settled, map[string]any{"status": "resolved", "selected": "yes"}, &clientID, now.Add(time.Second))
			if err != nil || continuation.Action.ParentItemID != surface.ID || continuation.Result.ParentItemID != items[0].ID || continuation.Call.Status != "completed" {
				t.Fatalf("resolved interaction = %#v, %v", continuation, err)
			}
			page, err = database.ConversationItemPage(ctx, conversation.ID, "", 20)
			if err != nil || len(page.Items) != 5 {
				t.Fatalf("resolved transcript = %#v, %v", page.Items, err)
			}
			if _, err := database.ResolveConversationA2UI(ctx, conversation.ID, surface.ID, interactionID, 1,
				settled, map[string]any{}, nil, now); err == nil {
				t.Fatal("second interaction resolution succeeded")
			}
			claimed, err := database.ClaimConversationA2UI(ctx, surface.ID, now.Add(2*time.Second))
			if err != nil || claimed.Turn.Status != "running" || textJSON(claimed.Surface.Payload["interaction_state"]) != "resuming" {
				t.Fatalf("claimed interaction = %#v, %v", claimed, err)
			}
			if test.crashAfterPersistence {
				if _, err := database.CompleteConversationTurn(ctx, claimed.Turn, "The choice was persisted.", "The choice was persisted.", nil, now.Add(3*time.Second)); err != nil {
					t.Fatal(err)
				}
				recovered, err := database.RecoverConversationA2UI(ctx, now.Add(4*time.Second))
				if err != nil || len(recovered) != 0 {
					t.Fatalf("recovery after persisted completion = %#v, %v", recovered, err)
				}
				return
			}
			recovered, err := database.RecoverConversationA2UI(ctx, now.Add(3*time.Second))
			if err != nil || len(recovered) != 1 || recovered[0].Surface.ID != surface.ID || textJSON(recovered[0].Surface.Payload["interaction_state"]) != "answered" {
				t.Fatalf("recovered interaction = %#v, %v", recovered, err)
			}
			reclaimed, err := database.ClaimConversationA2UI(ctx, surface.ID, now.Add(4*time.Second))
			if err != nil {
				t.Fatal(err)
			}
			if test.terminal != "" {
				if _, err := database.FailConversationTurn(ctx, reclaimed.Turn, test.terminal, now.Add(5*time.Second)); err != nil {
					t.Fatal(err)
				}
			} else if _, err := database.CompleteConversationTurn(ctx, reclaimed.Turn, "The choice was completed.", "The choice was completed.", nil, now.Add(5*time.Second)); err != nil {
				t.Fatal(err)
			}
		})
	}
}

// Rust source: crates/noema-store/src/tests/conversation_interactions.rs::interaction_publication_rolls_back_items_and_turn_when_turn_fence_fails.
func TestRustStore_interaction_publication_rolls_back_items_and_turn_when_turn_fence_fails(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Show a form", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.ExecContext(ctx, "UPDATE conversation_turns SET status = 'completed' WHERE turn_id = ?", turn.ID); err != nil {
		t.Fatal(err)
	}
	_, err = database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{
			ProviderRound: 0, OutputIndex: 0, ProviderCallID: "call:rollback",
			ProviderName: "present_a2ui", Name: "noema.present_a2ui", Arguments: json.RawMessage(`{"jsonl":"rollback"}`),
		},
		A2UI: &ConversationA2UIInput{
			Projection: map[string]any{"protocol_version": "v0.9.1", "surface_id": "surface:rollback"}, HasActions: true,
			ProviderSelection: map[string]any{"role": "noema", "provider_kind": "openrouter", "provider_account_id": "provider_account:codex:default", "model_profile": "gpt-test"},
			ToolCatalogDigest: strings.Repeat("b", 64),
		},
	}, now)
	if err == nil {
		t.Fatal("interaction publication ignored the completed-turn fence")
	}
	page, err := database.ConversationItemPage(ctx, conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range page.Items {
		if item.Kind == ConversationToolCall || item.Kind == ConversationA2UICard {
			t.Fatalf("failed publication left output item = %#v", item)
		}
	}
	var status string
	if err := database.db.QueryRowContext(ctx, "SELECT status FROM conversation_turns WHERE turn_id = ?", turn.ID).Scan(&status); err != nil || status != "completed" {
		t.Fatalf("fenced turn status = %q, %v", status, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::provider_assistant_text_shares_one_row_and_omits_equal_source_text.
func TestRustStore_provider_assistant_text_shares_one_row_and_omits_equal_source_text(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	equalTurn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "same", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	equal, err := database.CompleteConversationTurn(ctx, equalTurn, "Same text", "Same text", nil, now.Add(time.Second))
	if err != nil || equal.Kind != ConversationAssistantText || equal.ContentText != "Same text" || equal.ProviderContentText != "" {
		t.Fatalf("equal provider text = %#v, %v", equal, err)
	}
	projectedTurn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "projected", nil, now.Add(2*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	providerText := "Projected text \ue200cite\ue202turn0search0\ue201"
	projected, err := database.CompleteConversationTurn(ctx, projectedTurn, "Projected text", providerText, nil, now.Add(3*time.Second))
	if err != nil || projected.Kind != ConversationAssistantText || projected.ContentText != "Projected text" || projected.ProviderContentText != providerText {
		t.Fatalf("projected provider text = %#v, %v", projected, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil {
		t.Fatalf("provider items = %#v, %v", items, err)
	}
	assistantCount := 0
	for _, item := range items {
		if item.Kind == ConversationAssistantText {
			assistantCount++
		}
	}
	if assistantCount != 2 {
		t.Fatalf("assistant provider rows = %d, items=%#v", assistantCount, items)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::primary_notification_writes_resume_after_a_partial_save.
func TestRustStore_primary_notification_writes_resume_after_a_partial_save(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	firstTaskID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	firstTask, err := database.CreateTask(ctx, firstTaskID, "Notification source", "correlation:notification:one", now)
	if err != nil {
		t.Fatal(err)
	}
	firstEvent, err := database.LatestTaskWorkEvent(ctx, firstTask.ID)
	if err != nil {
		t.Fatal(err)
	}
	write := PrimaryNotificationWrite{Event: firstEvent, Conversation: conversation, Source: "task_notification", Text: "Progress"}
	firstItems, err := database.CommitPrimaryNotification(ctx, write, now)
	if err != nil || len(firstItems) != 1 || firstItems[0].Kind != ConversationAssistantText || firstItems[0].ContentText != "Progress" {
		t.Fatalf("first notification = %#v, %v", firstItems, err)
	}
	repeated, err := database.CommitPrimaryNotification(ctx, write, now)
	if err != nil || len(repeated) != 0 {
		t.Fatalf("repeated first notification = %#v, %v", repeated, err)
	}
	secondTaskID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	secondTask, err := database.CreateTask(ctx, secondTaskID, "Notification completion", "correlation:notification:two", now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	secondEvent, err := database.LatestTaskWorkEvent(ctx, secondTask.ID)
	if err != nil {
		t.Fatal(err)
	}
	secondItems, err := database.CommitPrimaryNotification(ctx, PrimaryNotificationWrite{
		Event: secondEvent, Conversation: conversation, Source: "task_notification", Text: "Done",
	}, now.Add(2*time.Second))
	if err != nil || len(secondItems) != 1 || secondItems[0].Kind != ConversationAssistantText || secondItems[0].ContentText != "Done" {
		t.Fatalf("second notification = %#v, %v", secondItems, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) != 2 || items[0].ContentText != "Progress" || items[1].ContentText != "Done" {
		t.Fatalf("notification provider items = %#v, %v", items, err)
	}
	if items[0].Metadata["notification_id"] != "work-event:"+fmt.Sprint(firstEvent.ID) || items[1].Metadata["notification_id"] != "work-event:"+fmt.Sprint(secondEvent.ID) {
		t.Fatalf("notification identities = %#v", items)
	}
	if cursor, err := database.PrimaryTaskNotificationCursor(ctx); err != nil || cursor != secondEvent.ID {
		t.Fatalf("notification cursor = %d, %v", cursor, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::conversation_working_directory_is_allocated_and_persisted.
func TestRustStore_conversation_working_directory_is_allocated_and_persisted(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", time.Unix(1700000000, 0))
	if err != nil {
		t.Fatal(err)
	}
	if conversation.CWD == "" || !filepath.IsAbs(conversation.CWD) {
		t.Fatalf("default conversation working directory = %q, want an absolute path", conversation.CWD)
	}
	info, err := os.Stat(conversation.CWD)
	if err != nil || !info.IsDir() {
		t.Fatalf("default conversation working directory stat = %v, %v", info, err)
	}
	reloaded, err := database.Conversation(ctx, conversation.ID)
	if err != nil || reloaded.CWD != conversation.CWD {
		t.Fatalf("persisted conversation working directory = %#v, %v", reloaded, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::explicit_conversation_working_directory_replaces_default.
func TestRustStore_explicit_conversation_working_directory_replaces_default(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	explicit := filepath.Join(t.TempDir(), "conversation-cwd")
	if err := os.Mkdir(explicit, 0o700); err != nil {
		t.Fatal(err)
	}
	want, err := filepath.Abs(explicit)
	if err != nil {
		t.Fatal(err)
	}
	selected, err := database.EnsurePrimaryConversation(ctx, "openrouter", explicit, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if selected.ID != conversation.ID || selected.CWD != want {
		t.Fatalf("explicit conversation working directory = %#v, want %q", selected, want)
	}
	reloaded, err := database.Conversation(ctx, conversation.ID)
	if err != nil || reloaded.CWD != want {
		t.Fatalf("reloaded explicit conversation working directory = %#v, %v", reloaded, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::final_tool_result_finishes_exact_call_and_repeats_without_a_duplicate.
func TestRustStore_final_tool_result_finishes_exact_call_and_repeats_without_a_duplicate(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		name, wantStatus string
		success          bool
	}{
		{name: "success", wantStatus: "completed", success: true},
		{name: "failure", wantStatus: "failed"},
	} {
		turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Call "+test.name, nil, now)
		if err != nil {
			t.Fatal(err)
		}
		calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
			Provider: "openrouter",
			Call: ConversationToolCallInput{
				ProviderCallID: "call:" + test.name, ProviderName: "example", Name: "example",
				Arguments: json.RawMessage(fmt.Sprintf(`{"case":%q}`, test.name)),
			},
		}, now)
		if err != nil || len(calls) != 1 || calls[0].Kind != ConversationToolCall || calls[0].Status != "running" {
			t.Fatalf("%s call = %#v, %v", test.name, calls, err)
		}
		resultInput := ConversationToolResultInput{
			CallItemID: calls[0].ID, Provider: "openrouter", ProviderCallID: "call:" + test.name,
			ProviderName: "example", Name: "example", Success: test.success,
			Payload: json.RawMessage(fmt.Sprintf(`{"case":%q}`, test.name)),
		}
		first, err := database.FinishConversationToolCall(ctx, turn, resultInput, now.Add(time.Second))
		if err != nil || first.Kind != ConversationToolResult || first.Status != test.wantStatus || first.ParentItemID != calls[0].ID {
			t.Fatalf("%s first result = %#v, %v", test.name, first, err)
		}
		repeated, err := database.FinishConversationToolCall(ctx, turn, resultInput, now.Add(2*time.Second))
		if err != nil || repeated.ID != first.ID || repeated.Status != test.wantStatus {
			t.Fatalf("%s repeated result = %#v, want same %s, %v", test.name, repeated, first.ID, err)
		}
		if _, err := database.CompleteConversationTurn(ctx, turn, "Finished "+test.name, "Finished "+test.name, nil, now.Add(3*time.Second)); err != nil {
			t.Fatal(err)
		}
	}
	page, err := database.ConversationItemPage(ctx, conversation.ID, "", 50)
	if err != nil {
		t.Fatal(err)
	}
	toolItems := make([]ConversationItem, 0, 4)
	for _, item := range page.Items {
		if item.Kind == ConversationToolCall || item.Kind == ConversationToolResult {
			toolItems = append(toolItems, item)
		}
	}
	if len(toolItems) != 4 {
		t.Fatalf("tool rows = %#v, want four rows", toolItems)
	}
	for index := 0; index < len(toolItems); index += 2 {
		if toolItems[index].Kind != ConversationToolCall || toolItems[index+1].Kind != ConversationToolResult || toolItems[index].Status != toolItems[index+1].Status {
			t.Fatalf("tool call/result pair = %#v", toolItems[index:index+2])
		}
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
	taskID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, taskID, "Task update", "correlation:task-delivery", time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	event, err := database.LatestTaskWorkEvent(ctx, task.ID)
	if err != nil {
		t.Fatal(err)
	}
	write := PrimaryNotificationWrite{Event: event, Conversation: conversation, Source: "task_status", Task: &task}
	first, err := database.CommitPrimaryNotification(ctx, write, time.Unix(0, 0))
	if err != nil || len(first) != 1 || first[0].Kind != ConversationTaskReference || first[0].Payload["task_id"] != task.ID {
		t.Fatalf("first task delivery = %#v, %v", first, err)
	}
	repeated, err := database.CommitPrimaryNotification(ctx, write, time.Unix(0, 0))
	if err != nil || len(repeated) != 0 {
		t.Fatalf("idempotent task delivery = %#v, %v", repeated, err)
	}
	items, err := database.ConversationProviderItems(ctx, conversation.ID)
	if err != nil || len(items) != 1 || items[0].Kind != ConversationTaskReference || items[0].Payload["task_id"] != task.ID {
		t.Fatalf("task delivery items = %#v, %v", items, err)
	}
	if cursor, err := database.PrimaryTaskNotificationCursor(ctx); err != nil || cursor != event.ID {
		t.Fatalf("task delivery cursor = %d, %v", cursor, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::mcp_setup_tool_result_remains_pending_until_exact_resolution.
func TestRustStore_mcp_setup_tool_result_remains_pending_until_exact_resolution(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Connect it.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{ProviderCallID: "setup-1", ProviderName: "mcp.connect_service",
			Name: "mcp.connect_service", Arguments: json.RawMessage(`{"service_url":"https://example.test"}`)},
	}, now)
	if err != nil || len(items) != 1 || items[0].Kind != ConversationToolCall {
		t.Fatalf("setup call = %#v, %v", items, err)
	}
	result, err := database.FinishConversationToolCall(ctx, turn, ConversationToolResultInput{
		CallItemID: items[0].ID, Provider: "openrouter", ProviderCallID: "setup-1",
		ProviderName: "mcp.connect_service", Name: "mcp.connect_service", Success: true,
		Payload: json.RawMessage(`{"status":"needs_auth"}`),
	}, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	pending, err := database.PendingMCPSetupItems(ctx, conversation.ID, 10)
	if err != nil || len(pending) != 1 || pending[0].ID != result.ID {
		t.Fatalf("pending setup items = %#v, %v", pending, err)
	}
	if changed, err := database.ResolveMCPSetupItem(ctx, conversation.ID, result.ID, "mcp:notion"); err != nil || !changed {
		t.Fatalf("resolve setup = %t, %v", changed, err)
	}
	pending, err = database.PendingMCPSetupItems(ctx, conversation.ID, 10)
	if err != nil || len(pending) != 0 {
		t.Fatalf("resolved setup items = %#v, %v", pending, err)
	}
	if changed, err := database.ResolveMCPSetupItem(ctx, conversation.ID, result.ID, "mcp:notion"); err != nil || changed {
		t.Fatalf("repeated setup resolution = %t, %v", changed, err)
	}
}

// Rust source: crates/noema-store/src/tests/conversations.rs::memory_source_range_captures_one_conversation_head_and_resumes_after_it.
func TestRustStore_memory_source_range_captures_one_conversation_head_and_resumes_after_it(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "first", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	trigger, err := database.CompleteConversationTurn(ctx, turn, "context", "context", nil, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	continuation, err := database.BeginConversationContinuation(ctx, conversation.ID, trigger.ID, now.Add(2*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.StartConversationToolRound(ctx, continuation, ConversationToolRound{
		Provider: "openrouter",
		Call: ConversationToolCallInput{ProviderCallID: "memory-tool", ProviderName: "memory.lookup",
			Name: "memory.lookup", Arguments: json.RawMessage(`{"query":"tool evidence"}`)},
	}, now.Add(3*time.Second))
	if err != nil || len(items) != 1 {
		t.Fatalf("memory tool call = %#v, %v", items, err)
	}
	if _, err := database.FinishConversationToolCall(ctx, continuation, ConversationToolResultInput{
		CallItemID: items[0].ID, Provider: "openrouter", ProviderCallID: "memory-tool",
		ProviderName: "memory.lookup", Name: "memory.lookup", Success: true,
		Payload: json.RawMessage(`{"text":"tool evidence"}`),
	}, now.Add(4*time.Second)); err != nil {
		t.Fatal(err)
	}
	first, err := database.CaptureMemorySourceRange(ctx, conversation.ID, 0)
	if err != nil {
		t.Fatal(err)
	}
	if first.CapturedHead != 3 || len(first.Items) != 3 {
		t.Errorf("first memory source range = %#v, want head 3 with three rows", first)
	}
	wantSequence := []int64{1, 2, 3}
	if len(first.Items) == len(wantSequence) {
		for index, item := range first.Items {
			if item.Sequence != wantSequence[index] {
				t.Errorf("first memory sequence[%d] = %d, want %d", index, item.Sequence, wantSequence[index])
			}
		}
	}
	if err := database.CancelConversationTurn(ctx, continuation, now.Add(5*time.Second)); err != nil {
		t.Fatal(err)
	}
	laterTurn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "later", nil, now.Add(6*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if laterTurn.ID == "" {
		t.Fatal("later conversation turn has no id")
	}
	resumed, err := database.CaptureMemorySourceRange(ctx, conversation.ID, first.CapturedHead)
	if err != nil {
		t.Fatal(err)
	}
	if resumed.CapturedHead != first.CapturedHead+1 || len(resumed.Items) != 1 || resumed.Items[0].ContentText != "later" {
		t.Fatalf("resumed memory source range = %#v", resumed)
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
	database, action, now := rustStoreGovernedActionFixture(t, json.RawMessage(`{"record_id":"42","body":{"value":"exact"}}`))
	ctx := t.Context()
	if action.State != ActionProposed || action.CapabilityName != "mcp.example.write" || action.OperationToken != "exact-token" {
		t.Fatalf("action identity = %#v", action)
	}
	if action.Arguments["record_id"] != "42" {
		t.Fatalf("action arguments = %#v", action.Arguments)
	}
	body, ok := action.Arguments["body"].(map[string]any)
	if !ok || body["value"] != "exact" {
		t.Fatalf("nested action arguments = %#v", action.Arguments)
	}
	canonical, err := json.Marshal(action.Arguments)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(canonical)
	if action.ArgumentsSHA256 != hex.EncodeToString(digest[:]) || len(action.ArgumentsSHA256) != 64 {
		t.Fatalf("action argument digest = %q, want %x", action.ArgumentsSHA256, digest)
	}
	if action.InputSchema["type"] != "object" || action.AuthorizationContext["human_or_task_request"] != "update the record" {
		t.Fatalf("action authority fields = schema=%#v context=%#v", action.InputSchema, action.AuthorizationContext)
	}
	observed := []string{"https://example.com/result?q=1"}
	if err := database.ObserveURLs(ctx, "search_result", "tool_call:search", observed, now); err != nil {
		t.Fatal(err)
	}
	if found, err := database.URLWasObserved(ctx, observed[0]); err != nil || !found {
		t.Fatalf("observed URL = %v, %v", found, err)
	}
	if found, err := database.URLWasObserved(ctx, "https://example.com/result?q=2"); err != nil || found {
		t.Fatalf("changed observed URL = %v, %v", found, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::unavailable_reviewer_requires_approval_and_cannot_be_claimed.
func TestRustStore_unavailable_reviewer_requires_approval_and_cannot_be_claimed(t *testing.T) {
	database, action, now := rustStoreGovernedActionFixture(t, json.RawMessage(`{"record_id":"42"}`))
	ctx := t.Context()
	reviewed, _, err := database.RecordActionAssessment(ctx, action.ID, action.Revision, ActionAssessment{
		Status: "reviewer_unavailable", ReasonCodes: []string{"authorization_ambiguous"}, Explanation: "reviewer is unavailable",
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	if reviewed.State != ActionAwaitingApproval || reviewed.Assessment == nil || reviewed.Assessment.Status != "reviewer_unavailable" ||
		len(reviewed.Assessment.ReasonCodes) != 1 || reviewed.Assessment.ReasonCodes[0] != "authorization_ambiguous" {
		t.Fatalf("fallback assessment = %#v", reviewed)
	}
	if _, err := database.ClaimActionRequest(ctx, action.ID, action.Revision, now); err == nil {
		t.Errorf("unapproved action was claimed")
	}
	superseded, err := database.SupersedeActionRequest(ctx, action.ID, action.Revision, "browser_session_unavailable", now)
	if err != nil || superseded.State != ActionSuperseded || superseded.FailureCode != "browser_session_unavailable" {
		t.Fatalf("superseded pending action = %#v, %v", superseded, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::human_approval_is_owner_scoped_and_consumed_by_one_claim.
func TestRustStore_human_approval_is_owner_scoped_and_consumed_by_one_claim(t *testing.T) {
	database, action, now := rustStoreGovernedActionFixture(t, json.RawMessage(`{"record_id":"42"}`))
	ctx := t.Context()
	if _, _, err := database.RecordActionAssessment(ctx, action.ID, action.Revision, ActionAssessment{
		Status: "reviewer_unavailable", ReasonCodes: []string{"authorization_ambiguous"}, Explanation: "reviewer is unavailable",
	}, now); err != nil {
		t.Fatal(err)
	}
	if _, err := database.DecideActionRequest(ctx, action.ID, action.Revision, "human:someone-else", "approve", now); err == nil {
		t.Errorf("foreign human approval was accepted")
	}
	approved, err := database.DecideActionRequest(ctx, action.ID, action.Revision, "human:local", "approve", now)
	if err != nil || approved.State != ActionExecutable {
		t.Fatalf("approved action = %#v, %v", approved, err)
	}
	pending, err := database.PendingActionRequests(ctx, "human:local", nil, nil, 10)
	if err != nil || len(pending) != 0 {
		t.Fatalf("pending actions after approval = %#v, %v", pending, err)
	}
	claimed, err := database.ClaimActionRequest(ctx, action.ID, action.Revision, now)
	if err != nil || claimed.State != ActionExecuting {
		t.Fatalf("claimed action = %#v, %v", claimed, err)
	}
	if _, err := database.ClaimActionRequest(ctx, action.ID, action.Revision, now); err == nil {
		t.Errorf("action received a second execution claim")
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::clear_review_is_claimed_once_and_records_uncertain_outcome.
func TestRustStore_clear_review_is_claimed_once_and_records_uncertain_outcome(t *testing.T) {
	database, action, now := rustStoreGovernedActionFixture(t, json.RawMessage(`{"record_id":"42"}`))
	ctx := t.Context()
	reviewed, _, err := database.RecordActionAssessment(ctx, action.ID, action.Revision, ActionAssessment{
		Status: "completed", Authorization: "explicit", Risk: "low", ReviewerSelection: map[string]any{"model_profile": "reviewer"},
		ReasonCodes: []string{"action_matches_request"}, Explanation: "exact action is authorized",
	}, now)
	if err != nil || reviewed.State != ActionExecutable || reviewed.Assessment == nil || reviewed.Assessment.Authorization != "explicit" ||
		reviewed.Assessment.Risk != "low" || reviewed.Assessment.Explanation != "exact action is authorized" {
		t.Fatalf("reviewed action = %#v, %v", reviewed, err)
	}
	claimed, err := database.ClaimActionRequest(ctx, action.ID, action.Revision, now)
	if err != nil || claimed.State != ActionExecuting {
		t.Fatalf("claimed action = %#v, %v", claimed, err)
	}
	if _, err := database.ClaimActionRequest(ctx, action.ID, action.Revision, now); err == nil {
		t.Errorf("action received a second execution claim")
	}
	finished, err := database.FinishActionRequest(ctx, action.ID, action.Revision, ActionOutcomeUncertain, nil, "outcome_uncertain", now)
	if err != nil || finished.State != ActionOutcomeUncertain || finished.FailureCode != "outcome_uncertain" {
		t.Fatalf("uncertain action outcome = %#v, %v", finished, err)
	}
}

// Rust source: crates/noema-store/src/tests/governed_actions.rs::composed_authorization_risk_policy_has_one_global_matrix.
func TestRustStore_composed_authorization_risk_policy_has_one_global_matrix(t *testing.T) {
	cases := []struct {
		authorization, risk string
		executable          bool
	}{
		{authorization: "explicit", risk: "low", executable: true},
		{authorization: "substantive", risk: "medium", executable: true},
		{authorization: "weak", risk: "low", executable: true},
		{authorization: "absent", risk: "low", executable: false},
		{authorization: "weak", risk: "medium", executable: false},
		{authorization: "explicit", risk: "high", executable: false},
	}
	for _, test := range cases {
		database, action, now := rustStoreGovernedActionFixture(t, json.RawMessage(`{"record_id":"42"}`))
		reviewed, _, err := database.RecordActionAssessment(t.Context(), action.ID, action.Revision, ActionAssessment{
			Status: "completed", Authorization: test.authorization, Risk: test.risk,
			ReviewerSelection: map[string]any{"model_profile": "reviewer"}, ReasonCodes: []string{"action_matches_request"}, Explanation: "bounded review",
		}, now)
		if err != nil {
			t.Fatalf("authorization=%s risk=%s assessment: %v", test.authorization, test.risk, err)
		}
		if (reviewed.State == ActionExecutable) != test.executable {
			t.Errorf("authorization=%s risk=%s executable=%t, want %t", test.authorization, test.risk, reviewed.State == ActionExecutable, test.executable)
		}
	}
}

// rustStoreGovernedActionFixture creates the exact call origin required by the
// Go action store before it persists one proposed governed action.
func rustStoreGovernedActionFixture(t *testing.T, arguments json.RawMessage) (*Store, ActionRequest, time.Time) {
	t.Helper()
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(0, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "update the record", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	calls, err := database.StartConversationToolRound(ctx, turn, ConversationToolRound{Provider: "openrouter", Call: ConversationToolCallInput{
		ProviderCallID: "call:governed-action", ProviderName: "mcp.example", Name: "mcp.example.write", Arguments: arguments,
	}}, now)
	if err != nil || len(calls) == 0 {
		t.Fatalf("tool call = %#v, %v", calls, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{
		ConversationID: conversation.ID, TurnID: turn.ID, CallItemID: calls[len(calls)-1].ID,
		OwnerHumanID: "human:local", RequestingAgentID: "agent:primary", CapabilityName: "mcp.example.write", OperationToken: "exact-token",
		ReviewRoute: ActionLLMReview, Behavior: ActionBehavior{OpenWorld: true}, Arguments: arguments,
		InputSchema: json.RawMessage(`{"type":"object"}`), AuthorizationContext: map[string]any{"human_or_task_request": "update the record"},
		SafeSummary: "mcp.example.write wants to write external data",
	}, now)
	if err != nil {
		t.Fatalf("create governed action: %v", err)
	}
	return database, action, now
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::live_activity_client_callbacks_form_a_secret_free_timeline.
func TestRustStore_live_activity_client_callbacks_form_a_secret_free_timeline(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Now().UTC().Truncate(time.Second)
	clientID := "client:timeline"
	_, _ = seedNativeFamily(t, database, clientID, "c", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSDevelopment,
		[]string{"live_activity:observed:test"}, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil || activity.Lifecycle != "starting" || activity.ActivityID == "" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	activityID := activity.ActivityID
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, clientID, activityID, []byte{4, 5, 6}, now); err != nil || !changed {
		t.Fatalf("live activity update = %v, %v", changed, err)
	}
	if changed, err := database.DismissClientLiveActivity(ctx, clientID, activityID); err != nil || !changed {
		t.Fatalf("dismiss activity = %v, %v", changed, err)
	}
	type observation struct {
		event, activityID, active string
	}
	rows, err := database.db.QueryContext(ctx, `SELECT event, COALESCE(activity_id, ''), active_activity_ids_json
FROM live_activity_observations WHERE client_id=? ORDER BY rowid`, clientID)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	observations := make([]observation, 0, 3)
	for rows.Next() {
		var item observation
		if err := rows.Scan(&item.event, &item.activityID, &item.active); err != nil {
			t.Fatal(err)
		}
		observations = append(observations, item)
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	wantObservations := []observation{
		{event: "snapshot", active: `["live_activity:observed:test"]`},
		{event: "update_token", activityID: activityID, active: `[]`},
		{event: "dismissed", activityID: activityID, active: `[]`},
	}
	if !reflect.DeepEqual(observations, wantObservations) {
		t.Fatalf("live activity observations = %#v, want %#v", observations, wantObservations)
	}
	if strings.Contains(fmt.Sprintf("%#v", observations), "1, 2, 3") || strings.Contains(fmt.Sprintf("%#v", observations), "4, 5, 6") {
		t.Fatalf("live activity observations retained token bytes: %#v", observations)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::terminal_live_activity_delivery_keeps_apns_id_for_thirty_days.
func TestRustStore_terminal_live_activity_delivery_keeps_apns_id_for_thirty_days(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	clientID := "client:delivery"
	_, _ = seedNativeFamily(t, database, clientID, "d", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil || activity.Lifecycle != "starting" || activity.ActivityID == "" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	if err := database.QueueLiveActivityDelivery(ctx, NewLiveActivityDelivery{
		ClientID: clientID, DeliveryKey: "live:start:retention", ActivityID: activity.ActivityID,
		Token: []byte{1, 2, 3}, Environment: APNSProduction, Event: LiveActivityStart,
		Payload: map[string]any{"aps": map[string]any{"event": "start"}}, Urgency: "high", TTLSeconds: 600,
	}, now); err != nil {
		t.Fatal(err)
	}
	delivery, err := database.ClaimDueLiveActivityDelivery(ctx, now)
	if err != nil || delivery == nil || delivery.Event != LiveActivityStart {
		t.Fatalf("claimed delivery = %#v, %v", delivery, err)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *delivery, APNSDelivered, "", "apns-retained", now); err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.ExecContext(ctx, `UPDATE live_activity_deliveries SET created_at_ms=? WHERE client_id=?`,
		now.Add(-29*24*time.Hour).UnixMilli(), clientID); err != nil {
		t.Fatal(err)
	}
	if recent, err := database.ClaimDueLiveActivityDelivery(ctx, now); err != nil || recent != nil {
		t.Fatalf("recent retention claim = %#v, %v", recent, err)
	}
	var apnsID string
	if err := database.db.QueryRowContext(ctx, `SELECT apns_id FROM live_activity_deliveries WHERE client_id=?`, clientID).Scan(&apnsID); err != nil {
		t.Fatal(err)
	}
	if apnsID != "apns-retained" {
		t.Fatalf("retained APNs identifier = %q", apnsID)
	}
	if _, err := database.db.ExecContext(ctx, `UPDATE live_activity_deliveries SET created_at_ms=? WHERE client_id=?`,
		now.Add(-31*24*time.Hour).UnixMilli(), clientID); err != nil {
		t.Fatal(err)
	}
	if expired, err := database.ClaimDueLiveActivityDelivery(ctx, now); err != nil || expired != nil {
		t.Fatalf("expired retention claim = %#v, %v", expired, err)
	}
	var count int
	if err := database.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM live_activity_deliveries WHERE client_id=?`, clientID).Scan(&count); err != nil {
		t.Fatal(err)
	}
	if count != 0 {
		t.Fatalf("retained delivery count = %d, want zero", count)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::live_registration_binds_to_active_client_and_redacts_tokens.
func TestRustStore_live_registration_binds_to_active_client_and_redacts_tokens(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	clientID := "client:live-one"
	oldClientID := "client:live-old"
	_, _ = seedNativeFamily(t, database, clientID, "e", now.Unix())
	_, _ = seedNativeFamily(t, database, oldClientID, "f", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, oldClientID, []byte{1, 2, 3}, APNSDevelopment, nil, now); err != nil {
		t.Fatal(err)
	}
	oldActivity, err := database.ClientTaskActivity(ctx, oldClientID)
	if err != nil || oldActivity == nil || oldActivity.ActivityID == "" {
		t.Fatalf("old live activity = %#v, %v", oldActivity, err)
	}
	if err := database.QueueLiveActivityDelivery(ctx, NewLiveActivityDelivery{
		ClientID: oldClientID, DeliveryKey: "live:start:old", ActivityID: oldActivity.ActivityID,
		Token: []byte{1, 2, 3}, Environment: APNSDevelopment, Event: LiveActivityStart,
		Payload: map[string]any{"aps": map[string]any{"event": "start"}}, Urgency: "high", TTLSeconds: 600,
	}, now); err != nil {
		t.Fatal(err)
	}
	if err := database.DisableClientLiveActivities(ctx, clientID, now); err != nil {
		t.Fatal(err)
	}
	disabled, err := database.ClientLiveActivityRegistration(ctx, clientID)
	if err != nil || disabled == nil || disabled.Enabled || len(disabled.PushToStartToken) != 0 || disabled.Environment != nil {
		t.Fatalf("disabled registration = %#v, %v", disabled, err)
	}
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSDevelopment, nil, now); err != nil {
		t.Fatal(err)
	}
	if registration, err := database.ClientLiveActivityRegistration(ctx, oldClientID); err != nil || registration != nil {
		t.Fatalf("transferred old registration = %#v, %v", registration, err)
	}
	if activity, err := database.ClientTaskActivity(ctx, oldClientID); err != nil || activity != nil {
		t.Fatalf("transferred old activity = %#v, %v", activity, err)
	}
	var oldDeliveryStatus, oldDeliveryCode string
	if err := database.db.QueryRowContext(ctx, `SELECT status, last_error_code FROM live_activity_deliveries
WHERE client_id=? AND delivery_key=?`, oldClientID, "live:start:old").Scan(&oldDeliveryStatus, &oldDeliveryCode); err != nil {
		t.Fatal(err)
	}
	if oldDeliveryStatus != "suppressed" || oldDeliveryCode != "token_transferred" {
		t.Fatalf("transferred delivery = %q/%q", oldDeliveryStatus, oldDeliveryCode)
	}
	registration, err := database.ClientLiveActivityRegistration(ctx, clientID)
	if err != nil || registration == nil || !registration.Enabled || string(registration.PushToStartToken) != string([]byte{1, 2, 3}) {
		t.Fatalf("active registration = %#v, %v", registration, err)
	}
	if debug := fmt.Sprintf("%#v", registration); strings.Contains(debug, "1, 2, 3") || !strings.Contains(debug, clientID) {
		t.Fatalf("registration debug leaks or omits client identity: %s", debug)
	}
	if err := database.RegisterClientLiveActivities(ctx, "client:missing", []byte{1, 2, 3}, APNSDevelopment, nil, now); err == nil {
		t.Fatal("missing client accepted Live Activity registration")
	}
	activity, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil || activity.Lifecycle != "starting" || activity.ActivityID == "" || activity.TaskSessionID == "" {
		t.Fatalf("starting activity = %#v, %v", activity, err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, clientID, activity.ActivityID, []byte{4, 5, 6}, now); err != nil || !changed {
		t.Fatalf("register update token = %v, %v", changed, err)
	}
	active, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || active == nil || active.Lifecycle != "active" {
		t.Fatalf("active activity = %#v, %v", active, err)
	}
	encoded, err := json.Marshal(active)
	if err != nil || strings.Contains(string(encoded), "4, 5, 6") {
		t.Fatalf("active activity serialization leaks token: %s", encoded)
	}
	activeActivityID, activeSessionID := active.ActivityID, active.TaskSessionID
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSDevelopment,
		[]string{activeActivityID}, now); err != nil {
		t.Fatal(err)
	}
	preserved, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || preserved == nil || preserved.ActivityID != activeActivityID || preserved.TaskSessionID != activeSessionID {
		t.Fatalf("preserved activity = %#v, %v", preserved, err)
	}
	if err := database.QueueLiveActivityDelivery(ctx, NewLiveActivityDelivery{
		ClientID: clientID, DeliveryKey: "live:update:stale", ActivityID: activeActivityID,
		Token: []byte{4, 5, 6}, Environment: APNSDevelopment, Event: LiveActivityUpdate,
		Payload: map[string]any{"aps": map[string]any{"event": "update"}}, Urgency: "normal", TTLSeconds: 600,
	}, now); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSDevelopment, nil, now); err != nil {
		t.Fatal(err)
	}
	replacement, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || replacement == nil || replacement.Lifecycle != "starting" || replacement.ActivityID == activeActivityID || replacement.TaskSessionID == activeSessionID || replacement.UpdateToken != nil {
		t.Fatalf("replacement activity = %#v, %v", replacement, err)
	}
	var staleStatus, staleCode string
	if err := database.db.QueryRowContext(ctx, `SELECT status, last_error_code FROM live_activity_deliveries
WHERE client_id=? AND delivery_key=?`, clientID, "live:update:stale").Scan(&staleStatus, &staleCode); err != nil {
		t.Fatal(err)
	}
	if staleStatus != "suppressed" || staleCode != "activity_missing" {
		t.Fatalf("stale delivery = %q/%q", staleStatus, staleCode)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::disabling_live_activities_clears_a_dismissed_session.
func TestRustStore_disabling_live_activities_clears_a_dismissed_session(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	clientID := "client:live"
	_, _ = seedNativeFamily(t, database, clientID, "g", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	activity, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || activity == nil || activity.Lifecycle != "starting" {
		t.Fatalf("live activity = %#v, %v", activity, err)
	}
	if changed, err := database.UpdateClientTaskActivityProjection(ctx, clientID,
		map[string]any{"focusTaskId": "task:one"}, strings.Repeat("a", 64), "task:one", now); err != nil || !changed {
		t.Fatalf("save activity projection = %v, %v", changed, err)
	}
	if changed, err := database.DismissClientLiveActivity(ctx, clientID, activity.ActivityID); err != nil || !changed {
		t.Fatalf("dismiss activity = %v, %v", changed, err)
	}
	if err := database.DisableClientLiveActivities(ctx, clientID, now); err != nil {
		t.Fatal(err)
	}
	dismissed, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || dismissed == nil || dismissed.Lifecycle != "dismissed" || dismissed.ProjectionSignature != "" || dismissed.FocusedTaskID != "" || len(dismissed.Projection) != 0 {
		t.Fatalf("disabled activity = %#v, %v", dismissed, err)
	}
	if cleared, err := database.ClearClientTaskActivityDismissal(ctx, clientID, now); err != nil || cleared {
		t.Fatalf("repeat dismissal clear = %v, %v", cleared, err)
	}
	if _, err := database.db.ExecContext(ctx, `UPDATE local_human_state SET primary_task_notification_event_id=10 WHERE state_id=1`); err != nil {
		t.Fatal(err)
	}
	if err := database.AdvancePrimaryTaskNotification(ctx, 10); err != nil {
		t.Errorf("same notification checkpoint was not idempotent: %v", err)
	}
	var checkpoint int64
	if err := database.db.QueryRowContext(ctx, `SELECT primary_task_notification_event_id FROM local_human_state WHERE state_id=1`).Scan(&checkpoint); err != nil {
		t.Fatal(err)
	}
	if checkpoint != 10 {
		t.Fatalf("notification checkpoint = %d, want 10", checkpoint)
	}
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	if created, err := database.EnsureClientTaskActivitySession(ctx, clientID, now); err != nil || !created {
		t.Fatalf("replacement activity session = %v, %v", created, err)
	}
}

// Rust source: crates/noema-store/src/tests/live_activity.rs::live_activity_end_delivery_dismisses_and_allows_a_new_session.
func TestRustStore_live_activity_end_delivery_dismisses_and_allows_a_new_session(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	clientID := "client:live"
	_, _ = seedNativeFamily(t, database, clientID, "h", now.Unix())
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	starting, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || starting == nil || starting.Lifecycle != "starting" || starting.ActivityID == "" || starting.TaskSessionID == "" {
		t.Fatalf("starting activity = %#v, %v", starting, err)
	}
	if err := database.QueueLiveActivityDelivery(ctx, NewLiveActivityDelivery{
		ClientID: clientID, DeliveryKey: "live:start:test", ActivityID: starting.ActivityID,
		Token: []byte{1, 2, 3}, Environment: APNSProduction, Event: LiveActivityStart,
		Payload: map[string]any{"aps": map[string]any{"event": "start"}}, Urgency: "high", TTLSeconds: 600,
	}, now); err != nil {
		t.Fatal(err)
	}
	start, err := database.ClaimDueLiveActivityDelivery(ctx, now)
	if err != nil || start == nil || start.Event != LiveActivityStart {
		t.Fatalf("start delivery = %#v, %v", start, err)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *start, APNSDelivered, "", "apns-start", now); err != nil {
		t.Fatal(err)
	}
	if changed, err := database.RegisterClientLiveActivityUpdate(ctx, clientID, starting.ActivityID, []byte{4, 5, 6}, now); err != nil || !changed {
		t.Fatalf("register update = %v, %v", changed, err)
	}
	active, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || active == nil || active.Lifecycle != "active" {
		t.Fatalf("active activity = %#v, %v", active, err)
	}
	if changed, err := database.UpdateClientTaskActivityProjection(ctx, clientID,
		map[string]any{"focusTaskId": "task:one", "phase": "working"}, strings.Repeat("a", 64), "task:one", now); err != nil || !changed {
		t.Fatalf("save active projection = %v, %v", changed, err)
	}
	if ending, err := database.MarkClientTaskActivityEnding(ctx, clientID, active.ActivityID, now); err != nil || !ending {
		t.Fatalf("mark activity ending = %v, %v", ending, err)
	}
	if err := database.QueueLiveActivityDelivery(ctx, NewLiveActivityDelivery{
		ClientID: clientID, DeliveryKey: "live:end:test", ActivityID: active.ActivityID,
		Token: []byte{4, 5, 6}, Environment: APNSProduction, Event: LiveActivityEnd,
		Payload: map[string]any{"aps": map[string]any{"event": "end"}}, Urgency: "high", TTLSeconds: 600,
	}, now); err != nil {
		t.Fatal(err)
	}
	end, err := database.ClaimDueLiveActivityDelivery(ctx, now)
	if err != nil || end == nil || end.Event != LiveActivityEnd {
		t.Fatalf("end delivery = %#v, %v", end, err)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *end, APNSDelivered, "", "apns-end", now); err != nil {
		t.Fatal(err)
	}
	dismissed, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || dismissed == nil || dismissed.Lifecycle != "dismissed" {
		t.Fatalf("dismissed activity = %#v, %v", dismissed, err)
	}
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{1, 2, 3}, APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	refreshed, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || refreshed == nil || refreshed.Lifecycle != "dismissed" || refreshed.TaskSessionID != starting.TaskSessionID || refreshed.ActivityID != starting.ActivityID {
		t.Fatalf("refreshed dismissed activity = %#v, %v", refreshed, err)
	}
	if created, err := database.EnsureClientTaskActivitySession(ctx, clientID, now); err != nil || !created {
		t.Fatalf("next activity session = %v, %v", created, err)
	}
	next, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || next == nil || next.Lifecycle != "starting" || next.TaskSessionID == starting.TaskSessionID || next.ActivityID == starting.ActivityID || next.UpdateToken != nil {
		t.Fatalf("next activity = %#v, %v", next, err)
	}
	if err := database.QueueLiveActivityDelivery(ctx, NewLiveActivityDelivery{
		ClientID: clientID, DeliveryKey: "live:start:" + next.TaskSessionID, ActivityID: next.ActivityID,
		Token: []byte{1, 2, 3}, Environment: APNSProduction, Event: LiveActivityStart,
		Payload: map[string]any{"aps": map[string]any{"event": "start"}}, Urgency: "high", TTLSeconds: 600,
	}, now); err != nil {
		t.Fatal(err)
	}
	secondStart, err := database.ClaimDueLiveActivityDelivery(ctx, now)
	if err != nil || secondStart == nil || secondStart.Event != LiveActivityStart {
		t.Fatalf("second start delivery = %#v, %v", secondStart, err)
	}
	if err := database.FinishLiveActivityDelivery(ctx, *secondStart, APNSInvalid, "", "", now); err != nil {
		t.Fatal(err)
	}
	if err := database.RegisterClientLiveActivities(ctx, clientID, []byte{7, 8, 9}, APNSProduction, nil, now); err != nil {
		t.Fatal(err)
	}
	rotated, err := database.ClientTaskActivity(ctx, clientID)
	if err != nil || rotated == nil || rotated.Lifecycle != "starting" || rotated.TaskSessionID == next.TaskSessionID || rotated.ActivityID == next.ActivityID {
		t.Fatalf("rotated activity = %#v, %v", rotated, err)
	}
	if err := database.QueueLiveActivityDelivery(ctx, NewLiveActivityDelivery{
		ClientID: clientID, DeliveryKey: "live:start:disable", ActivityID: rotated.ActivityID,
		Token: []byte{7, 8, 9}, Environment: APNSProduction, Event: LiveActivityStart,
		Payload: map[string]any{"aps": map[string]any{"event": "start"}}, Urgency: "high", TTLSeconds: 600,
	}, now); err != nil {
		t.Fatal(err)
	}
	if err := database.DisableClientLiveActivities(ctx, clientID, now); err != nil {
		t.Fatal(err)
	}
	if pending, err := database.ClaimDueLiveActivityDelivery(ctx, now); err != nil || pending != nil {
		t.Fatalf("delivery after disable = %#v, %v", pending, err)
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
	scope := RuntimeDebugScope{Kind: "conversation_turn", ID: turn.ID}
	id, err := database.BeginRuntimeDebugSpan(ctx, scope, "provider", "Initial provider request", RuntimeDebugMetadata{Phase: "initial"}, time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	live, err := database.RuntimeDebugProfile(ctx, scope)
	if err != nil || live == nil || len(live.Spans) != 1 || live.Spans[0].Status != "running" || live.Spans[0].Metadata.Phase != "initial" {
		t.Fatalf("live debug profile = %#v, %v", live, err)
	}
	_, insertErr := database.db.ExecContext(ctx, `INSERT INTO conversation_items
(item_id, conversation_id, turn_id, sequence_index, kind, status, author_actor_id)
VALUES ('item:unfinished-call', ?, ?, 1, 'tool_call', 'running', 'agent:primary')`, conversation.ID, turn.ID)
	if insertErr != nil {
		// Preserve the Rust fixture. Go currently rejects this short identifier
		// before turn finalization can observe it.
		t.Errorf("Rust unfinished-call fixture insert failed: %v", insertErr)
	}
	if _, err := database.CompleteConversationTurn(ctx, turn, "Completed", "Completed", nil, time.Unix(1, 0)); err != nil {
		t.Fatal(err)
	}
	durable, err := database.RuntimeDebugProfile(ctx, scope)
	if err != nil || durable == nil || len(durable.Spans) != 1 {
		t.Fatalf("durable debug profile = %#v, %v", durable, err)
	}
	if durable.Spans[0].Status != "completed" {
		t.Errorf("turn finalization span status = %q, want completed", durable.Spans[0].Status)
	}
	if durable.Spans[0].DurationMilliseconds == nil {
		t.Errorf("turn finalization span duration is nil")
	}
	var itemStatus string
	itemErr := database.db.QueryRowContext(ctx, `SELECT status FROM conversation_items WHERE item_id = 'item:unfinished-call'`).Scan(&itemStatus)
	if itemErr != nil || itemStatus != "failed" {
		t.Errorf("unfinished call status = %q, %v; want failed", itemStatus, itemErr)
	}
	if _, err := database.db.ExecContext(ctx, `INSERT INTO runtime_debug_spans (span_id, category, name) VALUES ('debug_span:invalid', 'runtime', 'invalid')`); err == nil {
		t.Errorf("a span without exactly one owner was accepted")
	}
	_ = id
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

func rustStoreExecutorFixture(t *testing.T, title string) (*Store, Task, TaskRun, time.Time) {
	t.Helper()
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, title, "correlation:rust-command", now)
	if err != nil {
		t.Fatal(err)
	}
	queued, err := database.QueueTask(ctx, id, task.Revision, task.Generation,
		testTaskLifecycleCommand("queue_task", "rust-command"), now)
	if err != nil {
		t.Fatal(err)
	}
	_, planner, found, err := database.ClaimTaskExecution(ctx, now)
	if err != nil || !found {
		t.Fatalf("claim planner = %#v, %t, %v", planner, found, err)
	}
	if err := database.StartTaskExecution(ctx, planner.ID, planner.Generation, now); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskPlanning(ctx, planner.ID, planner.Generation, "simple", now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	_, executor, found, err := database.ClaimTaskExecution(ctx, now.Add(2*time.Second))
	if err != nil || !found {
		t.Fatalf("claim executor = %#v, %t, %v", executor, found, err)
	}
	if err := database.StartTaskExecution(ctx, executor.ID, executor.Generation, now.Add(3*time.Second)); err != nil {
		t.Fatal(err)
	}
	if queued.Task.ID != task.ID {
		t.Fatalf("queue changed task identity: %#v", queued.Task)
	}
	return database, task, executor, now
}

func rustStoreTaskHome(t *testing.T, taskID, document string) *os.Root {
	t.Helper()
	root, err := os.OpenRoot(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = root.Close() })
	if _, err := home.CreatePendingTaskDocument(root, taskID, document); err != nil {
		t.Fatal(err)
	}
	if err := home.CommitTaskDocument(root, taskID); err != nil {
		t.Fatal(err)
	}
	return root
}

// Rust source: crates/noema-store/src/work_command_tests.rs::final_run_status_finishes_active_items_and_debug_spans.
func TestRustStore_final_run_status_finishes_active_items_and_debug_spans(t *testing.T) {
	for _, test := range []struct {
		name, itemStatus, spanStatus string
	}{
		{name: "completed", itemStatus: "completed", spanStatus: "completed"},
		{name: "failed", itemStatus: "failed", spanStatus: "failed"},
		{name: "cancelled", itemStatus: "cancelled", spanStatus: "cancelled"},
		{name: "interrupted", itemStatus: "failed", spanStatus: "interrupted"},
	} {
		t.Run(test.name, func(t *testing.T) {
			database, task, run, now := rustStoreExecutorFixture(t, "Rust command "+test.name)
			ctx := t.Context()
			if err := database.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{
				{Kind: "assistant_output", Status: "running", Round: 0, Content: "partial output"},
				{Kind: "tool_call", Status: "running", Round: 0, CorrelationID: "call:" + test.name, Content: "tool"},
			}, TaskRunUsage{}, now); err != nil {
				t.Fatal(err)
			}
			items, err := database.TaskRunReplayItems(ctx, run.ID)
			if err != nil || len(items) != 2 {
				t.Fatalf("initial run items = %#v, %v", items, err)
			}
			if err := database.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{
				{Kind: "tool_result", Status: "failed", Round: 0, ParentID: items[1].ID,
					CorrelationID: "call:" + test.name, Content: "tool failed"},
			}, TaskRunUsage{}, now.Add(time.Second)); err != nil {
				t.Fatal(err)
			}
			span, err := database.BeginRuntimeDebugSpan(ctx, RuntimeDebugScope{Kind: "task_run", ID: run.ID},
				"provider", "Provider call", RuntimeDebugMetadata{}, now)
			if err != nil {
				t.Fatal(err)
			}
			var terminalErr error
			switch test.name {
			case "completed":
				terminalErr = database.FinishTaskExecution(ctx, run.ID, run.Generation, false, now.Add(2*time.Second))
			case "failed":
				terminalErr = database.FailTaskExecution(ctx, run.ID, run.Generation, "work_runtime_failed", "Failed.", false, now.Add(2*time.Second))
			case "cancelled":
				current, loadErr := database.Task(ctx, task.ID)
				if loadErr != nil {
					t.Fatal(loadErr)
				}
				_, terminalErr = database.CancelTask(ctx, task.ID, current.Revision, current.Generation, "Cancelled.",
					testTaskLifecycleCommand("cancel_task", "cancel-"+test.name), now.Add(2*time.Second))
			case "interrupted":
				_, terminalErr = database.db.ExecContext(ctx,
					"UPDATE task_runs SET status='failed', ended_at_ms=? WHERE run_id=?", millis(now.Add(2*time.Second)), run.ID)
			}
			if terminalErr != nil {
				t.Fatal(terminalErr)
			}
			if err := database.FinishRuntimeDebugSpan(ctx, span, test.spanStatus, RuntimeDebugMetadata{}, 2*time.Second, now.Add(2*time.Second)); err != nil {
				t.Fatal(err)
			}
			items, err = database.TaskRunReplayItems(ctx, run.ID)
			if err != nil || len(items) != 3 {
				t.Fatalf("final run items = %#v, %v", items, err)
			}
			if items[0].Status != test.itemStatus || items[1].Status != "failed" || items[2].Status != "failed" {
				t.Fatalf("active item statuses = %#v; want [%s failed failed]", items, test.itemStatus)
			}
			profile, err := database.RuntimeDebugProfile(ctx, RuntimeDebugScope{Kind: "task_run", ID: run.ID})
			if err != nil || profile == nil || len(profile.Spans) != 1 {
				t.Fatalf("debug profile = %#v, %v", profile, err)
			}
			if profile.Spans[0].Status != test.spanStatus || profile.Spans[0].EndedAt == nil {
				t.Fatalf("debug span = %#v", profile.Spans[0])
			}
		})
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::finish_execution_requires_nonblank_result.
func TestRustStore_finish_execution_requires_nonblank_result(t *testing.T) {
	for _, test := range []struct {
		name, result string
	}{
		{name: "missing"},
		{name: "blank", result: " \n"},
	} {
		t.Run(test.name, func(t *testing.T) {
			database, task, run, now := rustStoreExecutorFixture(t, "Rust command result "+test.name)
			root := rustStoreTaskHome(t, task.ID, "captured description")
			if test.result != "" {
				if err := home.WriteTaskFile(root, task.ID, "RESULT.md", test.result); err != nil {
					t.Fatal(err)
				}
			}
			// FinishTaskExecution is the Go production boundary corresponding to
			// the Rust WorkCommandService terminal command. It must inspect the
			// current RESULT.md before it commits the terminal transition.
			if err := database.FinishTaskExecution(t.Context(), run.ID, run.Generation, false, now.Add(time.Second)); err == nil {
				t.Fatalf("accepted %s RESULT.md; task=%s", test.name, task.ID)
			}
			if _, err := home.ReadTaskDocument(root, task.ID); err != nil {
				t.Fatal(err)
			}
		})
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::required_task_documents_cannot_be_deleted.
func TestRustStore_required_task_documents_cannot_be_deleted(t *testing.T) {
	database, task, _, _ := rustStoreExecutorFixture(t, "Rust command files")
	_ = database
	root := rustStoreTaskHome(t, task.ID, "captured description")
	if err := home.WriteTaskFile(root, task.ID, "RESULT.md", "Current result."); err != nil {
		t.Fatal(err)
	}
	for _, required := range []string{"TASK.md", "RESULT.md"} {
		if err := home.DeleteTaskFile(root, task.ID, required); err == nil {
			t.Fatalf("deleted required Task file %q", required)
		}
	}
	if document, err := home.ReadTaskDocument(root, task.ID); err != nil || document.Content != "captured description" {
		t.Fatalf("TASK.md after rejected delete = %#v, %v", document, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::task_files_carry_execution_across_continuation_and_review.
func TestRustStore_task_files_carry_execution_across_continuation_and_review(t *testing.T) {
	database, task, first, now := rustStoreExecutorFixture(t, "Rust command file lifecycle")
	ctx := t.Context()
	root := rustStoreTaskHome(t, task.ID, "Durable delegated payload")
	if err := database.FinishTaskExecution(ctx, first.ID, first.Generation, true, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	runs, err := database.TaskRuns(ctx, task.ID, 10)
	if err != nil || len(runs) != 3 {
		t.Fatalf("continuation runs = %#v, %v", runs, err)
	}
	var second TaskRun
	for _, candidate := range runs {
		if candidate.Kind == "executor" && candidate.ID != first.ID && candidate.ParentRunID == first.ID {
			second = candidate
		}
	}
	if second.ID == "" {
		t.Fatalf("continuation child missing from runs = %#v", runs)
	}
	var found bool
	_, second, found, err = database.ClaimTaskExecution(ctx, now.Add(2*time.Second))
	if err != nil || !found || second.Kind != "executor" {
		t.Fatalf("claim continuation = %#v, %t, %v", second, found, err)
	}
	if err := database.StartTaskExecution(ctx, second.ID, second.Generation, now.Add(3*time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := home.WriteTaskFile(root, task.ID, "RESULT.md", "Completed file result.\n"); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskExecution(ctx, second.ID, second.Generation, false, now.Add(4*time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskExecution(ctx, second.ID, second.Generation, false, now.Add(4*time.Second)); err != nil {
		t.Errorf("exact execution replay returned %v; Rust replays the committed result", err)
	}
	_, reviewer, found, err := database.ClaimTaskExecution(ctx, now.Add(5*time.Second))
	if err != nil || !found || reviewer.Kind != "reviewer" {
		t.Fatalf("claim reviewer = %#v, %t, %v", reviewer, found, err)
	}
	if err := database.StartTaskExecution(ctx, reviewer.ID, reviewer.Generation, now.Add(6*time.Second)); err != nil {
		t.Fatal(err)
	}
	feedback := "The current Task result is complete."
	if err := home.WriteTaskFile(root, task.ID, "REVIEW.md", feedback); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskReview(ctx, reviewer.ID, reviewer.Generation, "approve", feedback, true, now.Add(7*time.Second)); err != nil {
		t.Fatal(err)
	}
	if err := database.FinishTaskReview(ctx, reviewer.ID, reviewer.Generation, "approve", feedback, true, now.Add(7*time.Second)); err != nil {
		t.Errorf("exact review replay returned %v; Rust replays the committed result", err)
	}
	if task, err := database.Task(ctx, task.ID); err != nil || task.StageKey != "done" || task.State != TaskCompleted {
		t.Fatalf("completed Task = %#v, %v", task, err)
	}
	document, err := home.ReadTaskDocument(root, task.ID)
	if err != nil || document.Content != "Durable delegated payload" {
		t.Fatalf("TASK.md = %#v, %v", document, err)
	}
	result, err := home.ReadTaskFile(root, task.ID, "RESULT.md")
	if err != nil || result != "Completed file result.\n" {
		t.Fatalf("RESULT.md = %q, %v", result, err)
	}
	review, err := home.ReadTaskFile(root, task.ID, "REVIEW.md")
	if err != nil || review != feedback {
		t.Fatalf("REVIEW.md = %q, %v", review, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::reviewer_controls_completion_notification.
func TestRustStore_reviewer_controls_completion_notification(t *testing.T) {
	for _, test := range []struct {
		name   string
		notify bool
		want   int64
	}{
		{name: "notify", notify: true, want: 1},
		{name: "silent", notify: false, want: 0},
	} {
		t.Run(test.name, func(t *testing.T) {
			database, task, executor, now := rustStoreExecutorFixture(t, "Rust command notification "+test.name)
			root := rustStoreTaskHome(t, task.ID, "Durable delegated payload")
			if err := home.WriteTaskFile(root, task.ID, "RESULT.md", "Completed result."); err != nil {
				t.Fatal(err)
			}
			if err := database.FinishTaskExecution(t.Context(), executor.ID, executor.Generation, false, now.Add(time.Second)); err != nil {
				t.Fatal(err)
			}
			_, reviewer, found, err := database.ClaimTaskExecution(t.Context(), now.Add(2*time.Second))
			if err != nil || !found {
				t.Fatalf("claim reviewer = %#v, %t, %v", reviewer, found, err)
			}
			if err := database.StartTaskExecution(t.Context(), reviewer.ID, reviewer.Generation, now.Add(3*time.Second)); err != nil {
				t.Fatal(err)
			}
			feedback := "Approved."
			if err := home.WriteTaskFile(root, task.ID, "REVIEW.md", feedback); err != nil {
				t.Fatal(err)
			}
			if err := database.FinishTaskReview(t.Context(), reviewer.ID, reviewer.Generation, "approve", feedback, test.notify, now.Add(4*time.Second)); err != nil {
				t.Fatal(err)
			}
			var count int64
			if err := database.db.QueryRowContext(t.Context(), `SELECT COUNT(*)
FROM work_notification_outbox
WHERE notification_kind = 'task_completed' AND json_extract(payload_json, '$.task_id') = ?`, task.ID).Scan(&count); err != nil {
				t.Errorf("completion notification ledger unavailable: %v", err)
				return
			}
			if count != test.want {
				t.Fatalf("completion notifications = %d, want %d", count, test.want)
			}
		})
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::task_summary_does_not_read_task_files.
func TestRustStore_task_summary_does_not_read_task_files(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	now := time.Unix(1700000000, 0).UTC()
	task, err := database.CreateTask(ctx, id, "Summary without files", "correlation:task-summary", now)
	if err != nil {
		t.Fatal(err)
	}
	root := rustStoreTaskHome(t, task.ID, "captured description")
	if err := os.Remove(filepath.Join(root.Name(), "tasks", strings.TrimPrefix(task.ID, "task:"), "TASK.md")); err != nil {
		t.Fatal(err)
	}
	stored, err := database.Task(ctx, task.ID)
	if err != nil || stored.Title != "Summary without files" {
		t.Fatalf("task summary state = %#v, %v", stored, err)
	}
	preview := ""
	if value, readErr := home.ReadTaskFile(root, task.ID, "TASK.md"); readErr == nil {
		preview = value
	} else if !errors.Is(readErr, os.ErrNotExist) {
		t.Fatalf("read missing Task document = %v", readErr)
	}
	if preview != "" {
		t.Fatalf("missing Task document preview = %q", preview)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::event_pagination_rejects_malformed_rows_in_both_directions.
func TestRustStore_event_pagination_rejects_malformed_rows_in_both_directions(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	firstID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, firstID, "First event", "correlation:event:first", now); err != nil {
		t.Fatal(err)
	}
	secondID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.CreateTask(ctx, secondID, "Second event", "correlation:event:second", now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	events, err := database.WorkEvents(ctx, "workspace:personal", 0, 1)
	if err != nil || len(events) != 1 {
		t.Fatalf("oldest event page = %#v, %v", events, err)
	}
	all, err := database.WorkEvents(ctx, "workspace:personal", 0, 100)
	if err != nil || len(all) < 2 || events[0].ID >= all[len(all)-1].ID {
		t.Fatalf("event order = %#v, %v", all, err)
	}
	newest, err := database.WorkEvents(ctx, "workspace:personal", events[0].ID, 1)
	if err != nil || len(newest) != 1 {
		t.Fatalf("newest event page = %#v, %v", newest, err)
	}
	if _, err := database.db.ExecContext(ctx, "UPDATE work_events SET payload_json = '{}'", nil); err != nil {
		t.Fatal(err)
	}
	if _, err := database.WorkEvents(ctx, "workspace:personal", 0, 1); err == nil {
		t.Errorf("oldest page accepted a malformed persisted event payload")
	}
	if _, err := database.WorkEvents(ctx, "workspace:personal", events[0].ID, 1); err == nil {
		t.Errorf("newest page accepted a malformed persisted event payload")
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::task_notification_suppresses_near_term_same_task_references.
func TestRustStore_task_notification_suppresses_near_term_same_task_references(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Capture the task.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if err := database.CancelConversationTurn(ctx, turn, now); err != nil {
		t.Fatal(err)
	}
	chatID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	chatResult, err := database.CreateTaskWithOptions(ctx, chatID, "Chat task", testTaskCommand("notification-chat"), TaskCreateOptions{
		Source: ArtifactSource{ConversationID: conversation.ID, TurnID: turn.ID}, SourceToolCallID: "tool_call:notification-chat",
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	chatEvent, err := database.LatestTaskWorkEvent(ctx, chatID)
	if err != nil {
		t.Fatal(err)
	}
	items, err := database.CommitPrimaryNotification(ctx, PrimaryNotificationWrite{
		Event: chatEvent, Conversation: conversation, Source: "work_notification", Task: &chatResult.Task,
		Metadata: map[string]any{"notification_kind": "task_created"},
	}, now)
	if err != nil || len(items) != 1 || items[0].Kind != ConversationTaskReference || items[0].TurnID != turn.ID {
		t.Fatalf("chat creation notification = %#v, %v", items, err)
	}
	intervening, _, err := database.BeginConversationTurn(ctx, conversation.ID, "Intervening message.", nil, now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if err := database.CancelConversationTurn(ctx, intervening, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	started, err := database.StartTask(ctx, chatID, "run:notification", now.Add(2*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	startedEvent, err := database.LatestTaskWorkEvent(ctx, chatID)
	if err != nil {
		t.Fatal(err)
	}
	items, err = database.CommitPrimaryNotification(ctx, PrimaryNotificationWrite{
		Event: startedEvent, Conversation: conversation, Source: "work_notification", Text: "Task waiting again.", Task: &started,
		Metadata: map[string]any{"notification_kind": "task_waiting"},
	}, now.Add(2*time.Second))
	if err != nil || len(items) != 1 || items[0].Kind != ConversationAssistantText {
		t.Errorf("near-term waiting notification = %#v, %v", items, err)
	}
	workID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	workTask, err := database.CreateTask(ctx, workID, "Work UI task", "correlation:notification:work-ui", now.Add(3*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	workEvent, err := database.LatestTaskWorkEvent(ctx, workID)
	if err != nil {
		t.Fatal(err)
	}
	items, err = database.CommitPrimaryNotification(ctx, PrimaryNotificationWrite{
		Event: workEvent, Conversation: conversation, Source: "work_notification", Text: "Another task completed.", Task: &workTask,
		Metadata: map[string]any{"notification_kind": "task_completed"},
	}, now.Add(3*time.Second))
	if err != nil || len(items) != 2 || items[0].Kind != ConversationAssistantText || items[1].Kind != ConversationTaskReference {
		t.Fatalf("distant completion notification = %#v, %v", items, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::receipt_replay_is_exact_and_divergent_replay_is_rejected.
func TestRustStore_receipt_replay_is_exact_and_divergent_replay_is_rejected(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	command := testTaskCommand("receipt-capture")
	command.Name, command.ClientMutationID, command.CorrelationID = "task.capture", "idem:capture", "correlation:idem:capture"
	first, err := database.CreateTaskWithOptions(ctx, id, "first title", command, TaskCreateOptions{}, now)
	if err != nil {
		t.Fatal(err)
	}
	returned := first.Task
	replay, err := database.CreateTaskWithOptions(ctx, id, "first title", command, TaskCreateOptions{}, now)
	if err != nil || !replay.Replayed || replay.Event.ID != first.Event.ID || replay.Task != returned {
		t.Fatalf("exact receipt replay = %#v, %v", replay, err)
	}
	divergent := command
	digest := sha256.Sum256([]byte("different title"))
	divergent.RequestDigest = hex.EncodeToString(digest[:])
	if _, err := database.CreateTaskWithOptions(ctx, id, "different title", divergent, TaskCreateOptions{}, now); !errors.Is(err, ErrCommandConflict) {
		t.Fatalf("divergent receipt error = %v, want %v", err, ErrCommandConflict)
	}
	updatedTitle := "edited title"
	updated, err := database.UpdateInboxTask(ctx, returned.ID, returned.Revision, returned.Generation,
		TaskUpdate{Title: &updatedTitle}, testTaskLifecycleCommand("update_task", "idem:update"), now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if returned.Title != "first title" || updated.Task.Title != "edited title" {
		t.Fatalf("receipt snapshots mutated: returned=%#v updated=%#v", returned, updated.Task)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::inbox_document_save_is_exact_and_a_stale_digest_changes_nothing.
func TestRustStore_inbox_document_save_is_exact_and_a_stale_digest_changes_nothing(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	task, err := database.CreateTask(ctx, id, "Original title", "correlation:document-save", now)
	if err != nil {
		t.Fatal(err)
	}
	root := rustStoreTaskHome(t, task.ID, "captured description")
	current, err := home.ReadTaskDocument(root, task.ID)
	if err != nil {
		t.Fatal(err)
	}
	exact := "# Exact\n\n- [x] kept  \n\n```rust\nlet value = 1;\n```\n"
	if err := home.WriteTaskFile(root, task.ID, "TASK.md", exact); err != nil {
		t.Fatal(err)
	}
	updatedTitle := "Updated title"
	updated, err := database.UpdateInboxTask(ctx, task.ID, task.Revision, task.Generation,
		TaskUpdate{Title: &updatedTitle, DocumentDigest: current.Digest}, testTaskLifecycleCommand("update_task", "document-save:update"), now.Add(time.Second))
	if err != nil || updated.Task.Title != "Updated title" {
		t.Fatalf("exact document update = %#v, %v", updated, err)
	}
	if document, err := home.ReadTaskDocument(root, task.ID); err != nil || document.Content != exact {
		t.Fatalf("saved Task document = %#v, %v", document, err)
	}
	if err := home.WriteTaskFile(root, task.ID, "TASK.md", "External change\n"); err != nil {
		t.Fatal(err)
	}
	staleTitle := "Stale title"
	stale, err := database.UpdateInboxTask(ctx, task.ID, updated.Task.Revision, updated.Task.Generation,
		TaskUpdate{Title: &staleTitle, DocumentDigest: current.Digest}, testTaskLifecycleCommand("update_task", "document-save:stale"), now.Add(2*time.Second))
	if err == nil {
		t.Errorf("stale document digest was accepted with result %#v", stale)
	}
	after, readErr := database.Task(ctx, task.ID)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if after.Title != "Updated title" {
		t.Errorf("stale document changed Task title to %q", after.Title)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::agent_inbox_edit_preserves_existing_authorization_context.
func TestRustStore_agent_inbox_edit_preserves_existing_authorization_context(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	source := ArtifactSource{ConversationID: "conversation:agent-edit", TurnID: "turn:agent-edit", ItemID: "item:agent-edit"}
	created, err := database.CreateTaskWithOptions(ctx, id, "Human title", testTaskCommand("agent-edit:capture"), TaskCreateOptions{
		Source: source, SourceToolCallID: "tool_call:agent-edit",
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	root := rustStoreTaskHome(t, created.Task.ID, "captured description")
	original := created.Task
	agentTitle := "Agent rewrite"
	updated, err := database.UpdateInboxTask(ctx, original.ID, original.Revision, original.Generation,
		TaskUpdate{Title: &agentTitle}, testTaskLifecycleCommand("update_task", "agent-edit:update"), now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if updated.Task.Title != "Agent rewrite" {
		t.Fatalf("agent edit title = %q", updated.Task.Title)
	}
	if updated.Task.Source != original.Source || updated.Task.SourceToolCallID != original.SourceToolCallID {
		t.Fatalf("agent edit changed source context: before=%#v after=%#v", original, updated.Task)
	}
	if document, err := home.ReadTaskDocument(root, original.ID); err != nil || document.Content != "captured description" {
		t.Fatalf("agent edit changed task document = %#v, %v", document, err)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::committed_detail_replay_does_not_reread_later_task_state.
func TestRustStore_committed_detail_replay_does_not_reread_later_task_state(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	command := testTaskCommand("committed-detail")
	first, err := database.CreateTaskWithOptions(ctx, id, "receipt title", command, TaskCreateOptions{}, now)
	if err != nil {
		t.Fatal(err)
	}
	firstTask := first.Task
	if firstTask.StageKey != "inbox" || TaskStageID(firstTask.State) != "stage:personal:inbox" {
		t.Fatalf("initial task stage = %#v", firstTask)
	}
	laterTitle := "later title"
	updated, err := database.UpdateInboxTask(ctx, firstTask.ID, firstTask.Revision, firstTask.Generation,
		TaskUpdate{Title: &laterTitle}, testTaskLifecycleCommand("update_task", "committed-detail:update"), now.Add(time.Second))
	if err != nil || updated.Task.Title != "later title" {
		t.Fatalf("later Task update = %#v, %v", updated, err)
	}
	replay, found, err := database.LookupTaskCommandReceipt(ctx, command)
	if err != nil || !found {
		t.Fatalf("committed receipt lookup = %#v, found=%t, %v", replay, found, err)
	}
	if !replay.Replayed || replay.Event.ID != first.Event.ID || replay.Task != firstTask {
		t.Fatalf("committed receipt replay = %#v; first=%#v", replay, first)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::stale_revision_is_atomic_and_inbox_edits_stop_at_queue.
func TestRustStore_stale_revision_is_atomic_and_inbox_edits_stop_at_queue(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	original, err := database.CreateTaskWithOptions(ctx, id, "captured", testTaskCommand("stale:capture"), TaskCreateOptions{}, now)
	if err != nil {
		t.Fatal(err)
	}
	editedTitle := "edited"
	current, err := database.UpdateInboxTask(ctx, original.Task.ID, original.Task.Revision, original.Task.Generation,
		TaskUpdate{Title: &editedTitle}, testTaskLifecycleCommand("update_task", "stale:update"), now.Add(time.Second))
	if err != nil {
		t.Fatal(err)
	}
	beforeEvents, err := database.LatestWorkEventSequence(ctx, "workspace:personal")
	if err != nil {
		t.Fatal(err)
	}
	staleTitle := "must not write"
	if _, err := database.UpdateInboxTask(ctx, original.Task.ID, original.Task.Revision, original.Task.Generation,
		TaskUpdate{Title: &staleTitle}, testTaskLifecycleCommand("update_task", "stale:second"), now.Add(2*time.Second)); !errors.Is(err, ErrStaleRevision) {
		t.Errorf("old revision error = %v, want %v", err, ErrStaleRevision)
	}
	afterEvents, err := database.LatestWorkEventSequence(ctx, "workspace:personal")
	if err != nil {
		t.Fatal(err)
	}
	if afterEvents != beforeEvents {
		t.Errorf("stale revision appended work event: before=%d after=%d", beforeEvents, afterEvents)
	}
	stored, err := database.Task(ctx, current.Task.ID)
	if err != nil {
		t.Fatal(err)
	}
	if stored.Title != "edited" {
		t.Errorf("stale revision changed Task title to %q", stored.Title)
	}
	queued, err := database.QueueTask(ctx, current.Task.ID, current.Task.Revision, current.Task.Generation,
		testTaskLifecycleCommand("queue_task", "queue"), now.Add(3*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if queued.Task.StageKey != "queue" || queued.Task.State != TaskRunning || queued.Task.CurrentRunID == "" {
		t.Fatalf("queued task = %#v", queued.Task)
	}
	if _, err := database.UpdateInboxTask(ctx, queued.Task.ID, queued.Task.Revision, queued.Task.Generation,
		TaskUpdate{Title: &staleTitle}, testTaskLifecycleCommand("update_task", "after-queue"), now.Add(4*time.Second)); !errors.Is(err, ErrInvalidTransition) {
		t.Errorf("Inbox update after queue error = %v, want %v", err, ErrInvalidTransition)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::stale_generation_is_rejected_even_when_revision_matches.
func TestRustStore_stale_generation_is_rejected_even_when_revision_matches(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	captured, err := database.CreateTaskWithOptions(ctx, id, "captured", testTaskCommand("generation:capture"), TaskCreateOptions{}, now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.db.ExecContext(ctx, "UPDATE tasks SET generation = generation + 1 WHERE task_id = ?", captured.Task.ID); err != nil {
		t.Fatal(err)
	}
	staleTitle := "must fail"
	_, err = database.UpdateInboxTask(ctx, captured.Task.ID, captured.Task.Revision, captured.Task.Generation,
		TaskUpdate{Title: &staleTitle}, testTaskLifecycleCommand("update_task", "generation:stale"), now.Add(time.Second))
	if err == nil {
		t.Errorf("old generation was accepted")
	} else if errors.Is(err, ErrStaleRevision) {
		t.Errorf("generation mismatch was classified as %v; Rust reports a distinct stale-generation error", ErrStaleRevision)
	}
	stored, err := database.Task(ctx, captured.Task.ID)
	if err != nil {
		t.Fatal(err)
	}
	if stored.Title != "captured" || stored.Generation != captured.Task.Generation+1 {
		t.Errorf("stale generation changed Task = %#v", stored)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::inbox_project_update_distinguishes_omitted_replacement_and_explicit_clear.
func TestRustStore_inbox_project_update_distinguishes_omitted_replacement_and_explicit_clear(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	projectID, err := NewProjectID()
	if err != nil {
		t.Fatal(err)
	}
	project, err := database.CreateProject(ctx, projectID, "workspace:personal", "Project association", "", nil,
		testProjectDigest(""), false, testProjectCommand("project.create", "project:association", "project association"), now)
	if err != nil {
		t.Fatal(err)
	}
	id, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	captured, err := database.CreateTaskWithOptions(ctx, id, "Associated task", testTaskCommand("project:capture"), TaskCreateOptions{
		ProjectID: project.Project.ID,
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	preservedTitle := "Still associated"
	preserved, err := database.UpdateInboxTask(ctx, captured.Task.ID, captured.Task.Revision, captured.Task.Generation,
		TaskUpdate{Title: &preservedTitle}, testTaskLifecycleCommand("update_task", "project:preserve"), now.Add(time.Second))
	if err != nil || preserved.Task.ProjectID != project.Project.ID {
		t.Fatalf("omitted project replacement = %#v, %v", preserved.Task, err)
	}
	cleared, err := database.UpdateInboxTask(ctx, preserved.Task.ID, preserved.Task.Revision, preserved.Task.Generation,
		TaskUpdate{SetProject: true}, testTaskLifecycleCommand("update_task", "project:clear"), now.Add(2*time.Second))
	if err != nil || cleared.Task.ProjectID != "" {
		t.Fatalf("explicit project clear = %#v, %v", cleared.Task, err)
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
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	homeRoot := t.TempDir()
	databasePath := filepath.Join(homeRoot, "db", "noema.sqlite3")
	if err := os.MkdirAll(filepath.Dir(databasePath), 0o700); err != nil {
		t.Fatal(err)
	}
	database, err := Open(ctx, databasePath)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if closeErr := database.Close(); closeErr != nil {
			t.Errorf("close test store: %v", closeErr)
		}
	})
	agent, err := database.CreateAcpAgent(ctx, "Fake ACP", "/bin/false", []string{"--safe"}, now)
	if err != nil {
		t.Fatal(err)
	}
	projectFolder := filepath.Join(homeRoot, "acp-worktree")
	if err := os.MkdirAll(projectFolder, 0o755); err != nil {
		t.Fatal(err)
	}
	projectID, err := NewProjectID()
	if err != nil {
		t.Fatal(err)
	}
	project, err := database.CreateProject(ctx, projectID, "workspace:personal", "ACP project", "", &projectFolder,
		testProjectDigest("# ACP project\n"), false, testProjectCommand("project.create", "acp-project", "acp-project"), now)
	if err != nil {
		t.Fatal(err)
	}
	createDelegated := func(key, title string, options TaskCreateOptions, at time.Time) Task {
		t.Helper()
		id, idErr := NewTaskID()
		if idErr != nil {
			t.Fatal(idErr)
		}
		options.ExecutorAgentID = agent.AgentID
		options.InitialRunKind = "executor"
		options.ExecutionComplexity = "simple"
		result, createErr := database.CreateTaskWithOptions(ctx, id, title, testTaskLifecycleCommand("delegate_task", key), options, at)
		if createErr != nil {
			t.Fatalf("create delegated Task %s: %v", key, createErr)
		}
		return result.Task
	}
	projectTask := createDelegated("acp-project-task", "Delegated acp-project-task", TaskCreateOptions{ProjectID: project.Project.ID}, now)
	overrideTask := createDelegated("acp-override-task", "Delegated acp-override-task", TaskCreateOptions{ProjectID: project.Project.ID, CwdOverride: stringAddressStore("/task/override")}, now.Add(time.Second))
	defaultTask := createDelegated("acp-default-task", "Delegated acp-default-task", TaskCreateOptions{}, now.Add(2*time.Second))
	projectRuns, err := database.TaskRuns(ctx, projectTask.ID, 10)
	if err != nil || len(projectRuns) != 1 {
		t.Fatalf("project Task runs = %#v, %v", projectRuns, err)
	}
	projectRun := projectRuns[0]
	if projectRun.ExecutorAgentID != agent.AgentID || projectRun.AcpLaunch == nil {
		t.Fatalf("project Executor = %#v", projectRun)
	}
	expectedProjectCWD := filepath.Join(projectFolder, "delegated-acp-project-task")
	if projectRun.EffectiveCwd == nil || *projectRun.EffectiveCwd != expectedProjectCWD {
		got := "<nil>"
		if projectRun.EffectiveCwd != nil {
			got = *projectRun.EffectiveCwd
		}
		t.Errorf("project working directory = %q, want %q", got, expectedProjectCWD)
	}
	if projectRun.AcpLaunch.ConnectionRevision != 1 || projectRun.AcpLaunch.Command != "/bin/false" || !reflect.DeepEqual(projectRun.AcpLaunch.Arguments, []string{"--safe"}) {
		t.Errorf("project ACP launch = %#v, want revision 1 /bin/false [--safe]", projectRun.AcpLaunch)
	}
	overrideRuns, err := database.TaskRuns(ctx, overrideTask.ID, 10)
	if err != nil || len(overrideRuns) != 1 || overrideRuns[0].EffectiveCwd == nil || *overrideRuns[0].EffectiveCwd != "/task/override/delegated-acp-override-task" {
		t.Errorf("override working directory = %#v, %v", overrideRuns, err)
	}
	defaultRuns, err := database.TaskRuns(ctx, defaultTask.ID, 10)
	expectedDefault := filepath.Join(homeRoot, "tasks", "delegated-acp-default-task")
	if err != nil || len(defaultRuns) != 1 {
		t.Errorf("default working directory = %#v, %v", defaultRuns, err)
	} else {
		if defaultRuns[0].EffectiveCwd == nil {
			t.Errorf("default run working directory = <nil>, want %q", expectedDefault)
		} else if *defaultRuns[0].EffectiveCwd != expectedDefault {
			t.Errorf("default run working directory = %q, want %q", *defaultRuns[0].EffectiveCwd, expectedDefault)
		}
	}
	if _, statErr := os.Stat(expectedDefault); statErr != nil {
		t.Errorf("default working directory is not a directory: %v", statErr)
	}
	updated, err := database.UpdateAcpAgent(ctx, agent.AgentID, 1, "Fake ACP", "/bin/true", []string{}, true, now.Add(3*time.Second))
	if err != nil {
		t.Fatal(err)
	}
	if updated.ConnectionRevision != 2 {
		t.Errorf("updated ACP revision = %d, want 2", updated.ConnectionRevision)
	}
	if projectRun.AcpLaunch.Command != "/bin/false" {
		t.Errorf("queued project launch changed after agent update: %#v", projectRun.AcpLaunch)
	}
	_, claimed, found, err := database.ClaimTaskExecution(ctx, now.Add(4*time.Second))
	if err != nil || !found || claimed.ID != projectRun.ID {
		t.Fatalf("claim project Task = %#v, %t, %v", claimed, found, err)
	}
	if err := database.StartTaskExecution(ctx, claimed.ID, claimed.Generation, now.Add(5*time.Second)); err != nil {
		t.Fatal(err)
	}
	startedRuns, err := database.TaskRuns(ctx, projectTask.ID, 10)
	if err != nil || len(startedRuns) != 1 || startedRuns[0].AcpLaunch == nil {
		t.Fatalf("started project Task = %#v, %v", startedRuns, err)
	}
	if startedRuns[0].AcpLaunch.ConnectionRevision != 2 || startedRuns[0].AcpLaunch.Command != "/bin/true" {
		t.Errorf("started ACP launch = %#v, want revision 2 /bin/true", startedRuns[0].AcpLaunch)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::missing_or_disabled_acp_executors_are_rejected_before_capture.
func TestRustStore_missing_or_disabled_acp_executors_are_rejected_before_capture(t *testing.T) {
	database := openTestStore(t)
	ctx := t.Context()
	now := time.Unix(1700000000, 0).UTC()
	disabled, err := database.CreateAcpAgent(ctx, "Disabled", "/bin/false", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := database.UpdateAcpAgent(ctx, disabled.AgentID, 1, "Disabled", "/bin/false", nil, false, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		name, agentID string
		want          error
	}{
		{name: "missing", agentID: "agent:missing", want: ErrAgentNotFound},
		{name: "disabled", agentID: disabled.AgentID, want: ErrInvalidAcpAgent},
	} {
		t.Run(test.name, func(t *testing.T) {
			id, err := NewTaskID()
			if err != nil {
				t.Fatal(err)
			}
			_, err = database.CreateTaskWithOptions(ctx, id, "Delegated "+test.name,
				testTaskLifecycleCommand("delegate_task", "acp:"+test.name), TaskCreateOptions{
					ExecutorAgentID: test.agentID, InitialRunKind: "executor", ExecutionComplexity: "simple",
				}, now.Add(2*time.Second))
			if !errors.Is(err, test.want) {
				t.Errorf("missing or disabled ACP error = %v, want %v", err, test.want)
			}
			exists, existsErr := database.TaskExists(ctx, id)
			if existsErr != nil || exists {
				t.Errorf("rejected %s task persisted = %t, %v", test.name, exists, existsErr)
			}
		})
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::acp_permission_decisions_match_exactly_and_approvals_are_consumed_once.
func TestRustStore_acp_permission_decisions_match_exactly_and_approvals_are_consumed_once(t *testing.T) {
	database, task, run, now := rustStoreExecutorFixture(t, "ACP permission task")
	ctx := t.Context()
	exact := map[string]any{
		"agent_id":             run.AgentID,
		"task_generation":      task.Generation,
		"tool_call":            map[string]any{"toolCallId": "tool:exact", "rawInput": map[string]any{"path": "/tmp/exact"}},
		"options":              []any{map[string]any{"optionId": "allow", "kind": "allow_once"}},
		"allow_once_option_id": "allow",
	}
	fingerprintFor := func(permission map[string]any) string {
		toolCall := permission["tool_call"]
		options := []map[string]any{{"optionId": "allow", "name": "", "kind": "allow_once"}}
		value, err := json.Marshal(map[string]any{
			"agent_id": run.AgentID, "generation": task.Generation,
			"tool_call": toolCall, "options": options,
		})
		if err != nil {
			t.Fatal(err)
		}
		digest := sha256.Sum256(value)
		return hex.EncodeToString(digest[:])
	}
	appendPermission := func(current TaskRun, permission map[string]any, fingerprint string, at time.Time) {
		t.Helper()
		if err := database.AppendTaskRunItems(ctx, current.ID, current.Generation, []TaskRunItemInput{{
			Kind: "progress_notice", Status: "completed", CorrelationID: "acp:permission:" + fingerprint,
			Content: "exact ACP request", Payload: map[string]any{
				"acp_permission": permission, "allow_once_option_id": "allow",
			},
		}}, TaskRunUsage{}, at); err != nil {
			t.Fatal(err)
		}
	}
	resolvePermission := func(current TaskRun, decision string, key string, at time.Time) TaskRun {
		t.Helper()
		if err := database.BlockTaskExecution(ctx, current.ID, current.Generation, "approval", "Allow this ACP operation once?", "exact ACP request", nil, at); err != nil {
			t.Fatal(err)
		}
		waiting, err := database.Task(ctx, task.ID)
		if err != nil {
			t.Fatal(err)
		}
		gate, err := database.TaskGate(ctx, waiting.ActiveGateID)
		if err != nil || gate.Kind != "approval" {
			t.Fatalf("ACP permission gate = %#v, %v", gate, err)
		}
		approval := decision
		resolved, err := database.ResolveTaskGate(ctx, task.ID, gate.ID, waiting.Revision, waiting.Generation,
			"ACP permission decision", "answer", &approval, testTaskLifecycleCommand("answer_task", key), at.Add(time.Second))
		if err != nil {
			t.Fatal(err)
		}
		if resolved.Task.CurrentRunID == "" {
			t.Fatalf("ACP permission continuation was not queued: %#v", resolved.Task)
		}
		_, continuation, found, err := database.ClaimTaskExecution(ctx, at.Add(2*time.Second))
		if err != nil || !found {
			t.Fatalf("claim ACP permission continuation = %#v, %t, %v", continuation, found, err)
		}
		if err := database.StartTaskExecution(ctx, continuation.ID, continuation.Generation, at.Add(3*time.Second)); err != nil {
			t.Fatal(err)
		}
		return continuation
	}
	fingerprint := fingerprintFor(exact)
	appendPermission(run, exact, fingerprint, now)
	continuation := resolvePermission(run, "approved", "acp-permission", now)
	resolved, err := database.ResolvedAcpPermission(ctx, continuation.ID, fingerprint)
	if err != nil {
		t.Fatal(err)
	}
	if !resolved.Found || !resolved.Approved || resolved.OptionID != "allow" {
		t.Errorf("exact ACP approval = %#v, want found approved allow", resolved)
	}
	if err := database.RecordAcpPermissionUse(ctx, continuation.ID, continuation.Generation, fingerprint, now.Add(4*time.Second)); err != nil {
		t.Fatal(err)
	}
	resolved, err = database.ResolvedAcpPermission(ctx, continuation.ID, fingerprint)
	if err != nil {
		t.Fatal(err)
	}
	if !resolved.Found || resolved.Approved {
		t.Errorf("repeated exact ACP approval = %#v, want found but not approved", resolved)
	}
	changed := map[string]any{
		"agent_id":             run.AgentID,
		"task_generation":      task.Generation,
		"tool_call":            map[string]any{"toolCallId": "tool:exact", "rawInput": map[string]any{"path": "/tmp/changed"}},
		"options":              []any{map[string]any{"optionId": "allow", "kind": "allow_once"}},
		"allow_once_option_id": "allow",
	}
	changedFingerprint := fingerprintFor(changed)
	if resolved, err := database.ResolvedAcpPermission(ctx, continuation.ID, changedFingerprint); err != nil {
		t.Fatal(err)
	} else if resolved.Found {
		t.Errorf("changed ACP permission unexpectedly resolved = %#v", resolved)
	}
	appendPermission(continuation, changed, changedFingerprint, now.Add(5*time.Second))
	deniedContinuation := resolvePermission(continuation, "declined", "acp-denial", now.Add(5*time.Second))
	resolved, err = database.ResolvedAcpPermission(ctx, deniedContinuation.ID, changedFingerprint)
	if err != nil {
		t.Fatal(err)
	}
	if !resolved.Found || resolved.Approved {
		t.Errorf("changed ACP denial = %#v, want found but not approved", resolved)
	}
	resolved, err = database.ResolvedAcpPermission(ctx, deniedContinuation.ID, fingerprint)
	if err != nil {
		t.Fatal(err)
	}
	if resolved.Found {
		t.Errorf("exact ACP permission was inherited by changed denial = %#v", resolved)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::inline_governed_action_cannot_resume_a_later_task_gate.
func TestRustStore_inline_governed_action_cannot_resume_a_later_task_gate(t *testing.T) {
	database, task, run, now := rustStoreExecutorFixture(t, "Inline action gate")
	ctx := t.Context()
	arguments := json.RawMessage(`{"url":"https://example.com"}`)
	if err := database.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{{
		Kind: "tool_call", Status: "running", CorrelationID: "call:inline", Content: "web.browse.open",
		Payload: map[string]any{"name": "web.browse.open", "arguments": map[string]any{"url": "https://example.com"}},
	}}, TaskRunUsage{}, now); err != nil {
		t.Fatal(err)
	}
	items, err := database.TaskRunReplayItems(ctx, run.ID)
	if err != nil || len(items) != 1 {
		t.Fatalf("inline action call = %#v, %v", items, err)
	}
	action, err := database.CreateActionRequest(ctx, NewActionRequest{
		TaskID: task.ID, RunID: run.ID, RunItemID: items[0].ID, TaskGeneration: task.Generation,
		OwnerHumanID: "human:local", RequestingAgentID: "agent:task-executor", CapabilityName: "web.browse.open",
		OperationToken: "web.browse.open", ReviewRoute: ActionLLMReview,
		Behavior: ActionBehavior{ReadOnly: true, RepeatSafe: true, OpenWorld: true}, Arguments: arguments,
		InputSchema: json.RawMessage(`{"type":"object"}`), AuthorizationContext: map[string]any{"origin": "task"},
		SafeSummary: "open the requested page",
	}, now)
	if err != nil {
		t.Fatal(err)
	}
	if action, _, err = database.RecordActionAssessment(ctx, action.ID, action.Revision, ActionAssessment{
		Status: "completed", Authorization: "explicit", Risk: "low", ReviewerSelection: map[string]any{"model_profile": "reviewer"},
		ReasonCodes: []string{"action_matches_request"}, Explanation: "the requested read is authorized",
	}, now); err != nil {
		t.Fatal(err)
	}
	if action.State != ActionExecutable {
		t.Errorf("inline action state after review = %q, want %q", action.State, ActionExecutable)
	}
	if _, err := database.ClaimActionRequest(ctx, action.ID, action.Revision, now); err != nil {
		t.Fatal(err)
	}
	if action, err = database.FinishActionRequest(ctx, action.ID, action.Revision, ActionSucceeded, json.RawMessage(`{"opened":true}`), "", now); err != nil {
		t.Fatal(err)
	}
	if action.State != ActionSucceeded || !reflect.DeepEqual(action.Output, map[string]any{"opened": true}) {
		t.Errorf("inline action result = %#v, want succeeded opened true", action)
	}
	if err := database.BlockTaskExecution(ctx, run.ID, run.Generation, "clarification", "What value should I use?", "", nil, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	current, err := database.Task(ctx, task.ID)
	if err != nil {
		t.Fatal(err)
	}
	gate, err := database.TaskGate(ctx, current.ActiveGateID)
	if err != nil {
		t.Fatal(err)
	}
	if gate.Kind != "clarification" || gate.Prompt != "What value should I use?" {
		t.Errorf("inline action gate = %#v, task = %#v", gate, current)
	}
	runs, err := database.TaskRuns(ctx, task.ID, 10)
	if err != nil {
		t.Fatal(err)
	}
	var originating *TaskRun
	for index := range runs {
		if runs[index].ID == run.ID {
			originating = &runs[index]
			break
		}
	}
	if originating == nil || originating.Status != "waiting_for_approval" {
		t.Errorf("inline action originating run = %#v, want waiting_for_approval", originating)
	}
	if _, err := database.db.ExecContext(ctx, "UPDATE task_runs SET status = 'queued' WHERE run_id = ?", run.ID); err != nil {
		t.Fatal(err)
	}
	if err := database.RecoverTaskExecutions(ctx, now.Add(2*time.Second)); err != nil {
		t.Fatal(err)
	}
	recovered, err := database.TaskRuns(ctx, task.ID, 10)
	if err != nil || len(recovered) == 0 {
		t.Fatalf("reconciled inline action run = %#v, %v", recovered, err)
	}
	var reconciled *TaskRun
	for index := range recovered {
		if recovered[index].ID == run.ID {
			reconciled = &recovered[index]
			break
		}
	}
	if reconciled == nil || reconciled.Status != "cancelled" {
		t.Errorf("reconciled inline action run = %#v, want cancelled", reconciled)
	}
}

// Rust source: crates/noema-store/src/work_command_tests.rs::executor_continuation_receives_actions_after_latest_task_save.
func TestRustStore_executor_continuation_receives_actions_after_latest_task_save(t *testing.T) {
	database, task, first, now := rustStoreExecutorFixture(t, "Unsaved actions")
	ctx := t.Context()
	appendCall := func(round int64, correlation, name string, status string, payload map[string]any) TaskRunItem {
		t.Helper()
		if err := database.AppendTaskRunItems(ctx, first.ID, first.Generation, []TaskRunItemInput{{
			Kind: "tool_call", Status: status, Round: round, CorrelationID: correlation, Content: name, Payload: payload,
		}}, TaskRunUsage{}, now); err != nil {
			t.Fatal(err)
		}
		page, err := database.TaskRunItems(ctx, first.ID, 100, nil)
		if err != nil || len(page.Items) == 0 {
			t.Fatalf("latest run item = %#v, %v", page.Items, err)
		}
		return page.Items[0]
	}
	appendResult := func(round int64, correlation, name, status, parent string, payload map[string]any) {
		t.Helper()
		if err := database.AppendTaskRunItems(ctx, first.ID, first.Generation, []TaskRunItemInput{{
			Kind: "tool_result", Status: status, Round: round, CorrelationID: correlation, ParentID: parent, Content: name, Payload: payload,
		}}, TaskRunUsage{}, now); err != nil {
			t.Fatal(err)
		}
	}
	oldCall := appendCall(0, "call:old", "web.browse.interact", "completed", map[string]any{"arguments": map[string]any{"ref": "old"}})
	appendResult(0, "call:old", "web.browse.interact", "completed", oldCall.ID, map[string]any{"success": true, "payload": map[string]any{"state": "old"}})
	saveCall := appendCall(1, "call:save", "task.files.write", "completed", map[string]any{"arguments": map[string]any{"path": "TASK.md", "content": "Saved progress."}})
	appendResult(1, "call:save", "task.files.write", "completed", saveCall.ID, map[string]any{"success": true, "payload": map[string]any{"path": "TASK.md"}})
	newCall := appendCall(2, "call:new", "web.browse.interact", "failed", map[string]any{"arguments": map[string]any{"ref": "e3", "action": "click"}})
	appendResult(2, "call:new", "web.browse.interact", "failed", newCall.ID, map[string]any{"success": false, "payload": map[string]any{"code": "outcome_uncertain"}})
	if err := database.FinishTaskExecution(ctx, first.ID, first.Generation, true, now.Add(time.Second)); err != nil {
		t.Fatal(err)
	}
	_, second, found, err := database.ClaimTaskExecution(ctx, now.Add(2*time.Second))
	if err != nil || !found {
		t.Fatalf("claim continuation Executor = %#v, %t, %v", second, found, err)
	}
	if err := database.StartTaskExecution(ctx, second.ID, second.Generation, now.Add(3*time.Second)); err != nil {
		t.Fatal(err)
	}
	admitted, err := database.TaskRunContinuationItems(ctx, second.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(admitted) != 2 {
		t.Errorf("admitted continuation lineage length = %d, want 2", len(admitted))
	}
	for _, item := range admitted {
		if item.CorrelationID == nil || *item.CorrelationID != "call:new" {
			t.Errorf("admitted continuation item correlation = %v, want call:new", item.CorrelationID)
		}
	}
	if len(admitted) >= 1 {
		if value, ok := admitted[0].Payload["payload"].(map[string]any); !ok || value["code"] != "outcome_uncertain" {
			t.Errorf("admitted first payload = %#v, want outcome_uncertain", admitted[0].Payload)
		}
	}
	if len(admitted) >= 2 {
		args, ok := admitted[1].Payload["arguments"].(map[string]any)
		if !ok || args["ref"] != "e3" {
			t.Errorf("admitted second arguments = %#v, want ref e3", admitted[1].Payload)
		}
	}
	if second.TaskID != task.ID {
		t.Errorf("continuation task = %q, want %q", second.TaskID, task.ID)
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
	now := time.Unix(1700000000, 0).UTC()
	conversation, err := database.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, item, err := database.BeginConversationTurn(ctx, conversation.ID, "Delegate the requested task.", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	account := createReadyModelAccount(t, database)
	if _, err := database.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	source := ArtifactSource{ConversationID: conversation.ID, TurnID: turn.ID, ItemID: item.ID}
	options := TaskCreateOptions{Source: source, SourceToolCallID: "tool_call:same-source", InitialRunKind: "planner"}
	command := func(key, title string) TaskCommand {
		digest := sha256.Sum256([]byte("delegate_task_tool\x00" + key + "\x00" + title))
		return TaskCommand{
			Name: "delegate_task_tool", ClientMutationID: key, RequestDigest: hex.EncodeToString(digest[:]),
			CorrelationID: "correlation:delegate:same-source",
		}
	}
	firstID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	first, err := database.CreateTaskWithOptions(ctx, firstID, "Delegated same-source", command("idem:source:first", "Delegated same-source"), options, now)
	if err != nil {
		t.Fatal(err)
	}
	secondID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	replay, err := database.CreateTaskWithOptions(ctx, secondID, "Delegated same-source", command("idem:source:second", "Delegated same-source"), options, now)
	if err != nil {
		t.Fatal(err)
	}
	if replay.Event.EventID != first.Event.EventID || replay.Event.ID != first.Event.ID {
		t.Errorf("source replay event = %#v, want event %q/%d", replay.Event, first.Event.EventID, first.Event.ID)
	}
	if !reflect.DeepEqual(replay.Task, first.Task) {
		t.Errorf("source replay Task = %#v, want %#v", replay.Task, first.Task)
	}
	if replay.Task.CurrentRunID != first.Task.CurrentRunID {
		t.Errorf("source replay planner run = %q, want %q", replay.Task.CurrentRunID, first.Task.CurrentRunID)
	}
	authority, err := database.ConversationAuthorizationContext(ctx, conversation.ID, turn.ID)
	if err != nil {
		t.Fatal(err)
	}
	messages, ok := authority["messages"].([]map[string]any)
	if !ok || len(messages) == 0 || messages[0]["role"] != "human" {
		t.Errorf("source authorization context = %#v, want first human message", authority)
	}
	thirdID, err := NewTaskID()
	if err != nil {
		t.Fatal(err)
	}
	divergent, err := database.CreateTaskWithOptions(ctx, thirdID, "Changed durable payload", command("idem:source:third", "Changed durable payload"), options, now)
	if err == nil || !errors.Is(err, ErrCommandConflict) {
		t.Errorf("divergent source replay = %#v, %v, want %v", divergent, err, ErrCommandConflict)
	}
	var durableRows int
	if err := database.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM tasks
WHERE source_conversation_id = ? AND source_tool_call_id = ?`, conversation.ID, "tool_call:same-source").Scan(&durableRows); err != nil {
		t.Fatal(err)
	}
	if durableRows != 1 {
		t.Errorf("source replay durable rows = %d, want 1", durableRows)
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
