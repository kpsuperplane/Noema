package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestHostedModelAssignmentsFreshSchema(t *testing.T) {
	database := openTestStore(t)
	var version int
	if err := database.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != schemaVersion {
		t.Fatalf("schema version = %d, want %d", version, schemaVersion)
	}
	var exists bool
	if err := database.db.QueryRow(`
SELECT EXISTS(SELECT 1 FROM sqlite_schema
WHERE type = 'table' AND name = 'hosted_model_assignments')`).Scan(&exists); err != nil {
		t.Fatal(err)
	}
	if !exists {
		t.Fatal("hosted_model_assignments table is missing")
	}
}

func TestHostedModelAssignmentsUpgradeFromVersionFive(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v5.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(schemaAtVersion(5) + "\nPRAGMA user_version = 5;"); err != nil {
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}

	upgraded, err := Open(context.Background(), path)
	if err != nil {
		t.Fatalf("upgrade version 5 schema: %v", err)
	}
	defer upgraded.Close()
	var version int
	if err := upgraded.db.QueryRow("PRAGMA user_version").Scan(&version); err != nil || version != schemaVersion {
		t.Fatalf("upgraded schema version = %d, %v", version, err)
	}
	assignments, err := upgraded.HostedModelAssignments(context.Background())
	if err != nil || len(assignments) != 0 {
		t.Fatalf("upgraded assignments = %#v, %v", assignments, err)
	}
}

func TestHostedModelAssignmentsValidateAndRollback(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	assignments := testModelAssignments(account, "model-a")
	assignments[len(assignments)-1].ReasoningEffort = ModelReasoningHigh

	created, err := database.ConfirmHostedModelAssignments(
		context.Background(), account.ID, assignments,
	)
	if created || !errors.Is(err, ErrInvalidModelAssignments) {
		t.Fatalf("unsupported effort result = %v, %v", created, err)
	}
	var count int
	if err := database.db.QueryRow("SELECT COUNT(*) FROM hosted_model_assignments").Scan(&count); err != nil {
		t.Fatal(err)
	}
	if count != 0 {
		t.Fatalf("failed assignment count = %d, want 0", count)
	}

	duplicate := testModelAssignments(account, "model-a")
	duplicate[len(duplicate)-1].Role = HostedModelNoema
	if _, err := database.ConfirmHostedModelAssignments(
		context.Background(), account.ID, duplicate,
	); !errors.Is(err, ErrInvalidModelAssignments) {
		t.Fatalf("duplicate role error = %v", err)
	}
	recommendedWithProfile := testModelAssignments(account, "model-a")
	recommendedWithProfile[0] = ModelAssignment{
		Role: HostedModelNoema, ProviderKind: account.ProviderKind,
		ProviderAccountID: account.ID, SelectionMode: ModelSelectionNoemaRecommended,
		ModelProfile: "model-a",
	}
	if _, err := database.ConfirmHostedModelAssignments(
		context.Background(), account.ID, recommendedWithProfile,
	); !errors.Is(err, ErrInvalidModelAssignments) {
		t.Fatalf("recommended profile error = %v", err)
	}
	mixedAccounts := testModelAssignments(account, "model-a")
	mixedAccounts[0].ProviderAccountID = "provider_account:openrouter:other"
	if _, err := database.ConfirmHostedModelAssignments(
		context.Background(), account.ID, mixedAccounts,
	); !errors.Is(err, ErrInvalidModelAssignments) {
		t.Fatalf("mixed account error = %v", err)
	}
	if _, err := database.db.Exec(
		"UPDATE provider_accounts SET status = 'unauthenticated' WHERE provider_account_id = ?",
		account.ID,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := database.ConfirmHostedModelAssignments(
		context.Background(), account.ID, testModelAssignments(account, "model-a"),
	); !errors.Is(err, ErrModelAccountNotReady) {
		t.Fatalf("unauthenticated account error = %v", err)
	}
}

func TestHostedModelAssignmentsPreserveSelectionsAndFirstCommit(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	assignments := testModelAssignments(account, "model-a")
	assignments[0] = ModelAssignment{
		Role: HostedModelNoema, ProviderKind: account.ProviderKind,
		ProviderAccountID: account.ID, SelectionMode: ModelSelectionNoemaRecommended,
		FastMode: true,
	}
	created, err := database.ConfirmHostedModelAssignments(
		context.Background(), account.ID, assignments,
	)
	if err != nil || !created {
		t.Fatalf("first confirmation = %v, %v", created, err)
	}

	stored, err := database.HostedModelAssignments(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(stored) != 9 || stored[0].ProviderKind != account.ProviderKind ||
		stored[0].ProviderAccountID != account.ID || !stored[0].FastMode ||
		stored[0].ModelProfile != "" || stored[0].ReasoningEffort != "" {
		t.Fatalf("stored assignments = %#v", stored)
	}
	created, err = database.ConfirmHostedModelAssignments(
		context.Background(), account.ID, testModelAssignments(account, "model-b"),
	)
	if err != nil || created {
		t.Fatalf("later confirmation = %v, %v", created, err)
	}
	stored, err = database.HostedModelAssignments(context.Background())
	if err != nil || stored[1].ModelProfile != "model-a" {
		t.Fatalf("later confirmation replaced stored data: %#v, %v", stored, err)
	}
}

func TestHostedModelAssignmentsConcurrentFirstCommitWins(t *testing.T) {
	database := openTestStore(t)
	account := createReadyModelAccount(t, database)
	start := make(chan struct{})
	results := make(chan bool, 2)
	errors := make(chan error, 2)
	var group sync.WaitGroup
	for _, profile := range []string{"model-a", "model-b"} {
		profile := profile
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			created, err := database.ConfirmHostedModelAssignments(
				context.Background(), account.ID, testModelAssignments(account, profile),
			)
			results <- created
			errors <- err
		}()
	}
	close(start)
	group.Wait()
	close(results)
	close(errors)
	for err := range errors {
		if err != nil {
			t.Fatal(err)
		}
	}
	winners := 0
	for created := range results {
		if created {
			winners++
		}
	}
	if winners != 1 {
		t.Fatalf("winner count = %d, want 1", winners)
	}
	stored, err := database.HostedModelAssignments(context.Background())
	if err != nil || len(stored) != 9 {
		t.Fatalf("stored assignments = %#v, %v", stored, err)
	}
	for _, assignment := range stored[1:] {
		if assignment.ModelProfile != stored[0].ModelProfile {
			t.Fatalf("mixed concurrent assignment set: %#v", stored)
		}
	}
}

func createReadyModelAccount(t *testing.T, database *Store) provider.Account {
	t.Helper()
	profiles, err := json.Marshal([]provider.ModelProfile{
		{ID: "model-a", Label: "Model A", ReasoningEfforts: []string{"low"}},
		{ID: "model-b", Label: "Model B", ReasoningEfforts: []string{"low"}},
	})
	if err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 9, 4, 12, 0, 0, 0, time.UTC)
	account := provider.Account{
		ID: "provider_account:openrouter:default", ProviderKind: "openrouter",
		AccountKey: "default", DisplayName: "OpenRouter", AuthMethod: provider.AuthSecretInput,
		IsActive: true, IsDefault: true, Status: provider.StatusAuthenticated,
		Metadata: provider.AccountMetadata{"profiles": profiles}, CreatedAt: now, UpdatedAt: now,
	}
	created, err := database.CreateProviderAccount(context.Background(), account)
	if err != nil {
		t.Fatal(err)
	}
	return created
}

func testModelAssignments(account provider.Account, profile string) []ModelAssignment {
	assignments := make([]ModelAssignment, 0, len(hostedModelRoles))
	for _, role := range hostedModelRoles {
		assignments = append(assignments, ModelAssignment{
			Role: role, ProviderKind: account.ProviderKind, ProviderAccountID: account.ID,
			SelectionMode: ModelSelectionExplicitProfile, ModelProfile: profile,
			ReasoningEffort: ModelReasoningLow, FastMode: role == HostedModelDifficultTasks,
		})
	}
	return assignments
}
