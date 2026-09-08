package graphql

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/localmodel"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// Rust source: crates/noema-host/src/onboarding.rs:233::local_model_and_cloud_are_alternative_onboarding_paths
func TestRustHost_local_model_and_cloud_are_alternative_onboarding_paths(t *testing.T) {
	ctx := context.Background()
	localResolver := openRustHostLocalResolver(t, true)
	local, err := localResolver.onboardingStatus(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if !local.IsUserOnboarded || len(local.Steps) != 1 || local.Steps[0].ID != "install_local_model" ||
		local.Steps[0].Status != model.OnboardingStepStatusComplete ||
		local.Steps[0].ProviderAccountStatus != model.ProviderAccountStatusAuthenticated {
		t.Fatalf("ready local onboarding status = %#v", local)
	}

	cloudResolver := openRustHostCloudResolver(t, provider.StatusAuthenticated)
	cloud, err := cloudResolver.onboardingStatus(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if !cloud.IsUserOnboarded || len(cloud.Steps) != 2 ||
		cloud.Steps[0].Status != model.OnboardingStepStatusBlocked ||
		cloud.Steps[1].Status != model.OnboardingStepStatusComplete ||
		cloud.Steps[1].ID != "connect_provider_account" || cloud.Steps[1].ProviderKind != "codex" {
		t.Fatalf("ready cloud onboarding status = %#v", cloud)
	}

	blockedLocalResolver := openRustHostLocalResolver(t, false)
	blockedLocal, err := blockedLocalResolver.onboardingStatus(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if blockedLocal.IsUserOnboarded || len(blockedLocal.Steps) != 1 ||
		blockedLocal.Steps[0].Status != model.OnboardingStepStatusBlocked {
		t.Fatalf("blocked local onboarding status = %#v", blockedLocal)
	}
	blockedCloudResolver := openRustHostCloudResolver(t, provider.StatusUnknown)
	blockedCloud, err := blockedCloudResolver.onboardingStatus(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if blockedCloud.IsUserOnboarded || len(blockedCloud.Steps) != 2 ||
		blockedCloud.Steps[0].Status != model.OnboardingStepStatusBlocked ||
		blockedCloud.Steps[1].Status != model.OnboardingStepStatusBlocked {
		t.Fatalf("blocked cloud onboarding status = %#v", blockedCloud)
	}
}

// Rust source: crates/noema-host/src/onboarding.rs:250::onboarding_requires_live_local_model_runtime
func TestRustHost_onboarding_requires_live_local_model_runtime(t *testing.T) {
	for _, test := range []struct {
		name  string
		ready bool
	}{
		{name: "stopped", ready: false},
		{name: "running", ready: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			resolver := openRustHostLocalResolver(t, test.ready)
			status, err := resolver.onboardingStatus(context.Background())
			if err != nil {
				t.Fatal(err)
			}
			if status.IsUserOnboarded != test.ready || len(status.Steps) != 1 ||
				(status.Steps[0].Status == model.OnboardingStepStatusComplete) != test.ready {
				t.Fatalf("runtime readiness %t = %#v", test.ready, status)
			}
		})
	}
}

// Rust source: crates/noema-host/src/onboarding.rs:272::onboarding_blocks_and_preserves_provider_readiness_status
func TestRustHost_onboarding_blocks_and_preserves_provider_readiness_status(t *testing.T) {
	for _, accountStatus := range []provider.AccountStatus{
		provider.StatusUnknown, provider.StatusChecking, provider.StatusUnauthenticated, provider.StatusUnavailable,
	} {
		t.Run(string(accountStatus), func(t *testing.T) {
			resolver := openRustHostCloudResolver(t, accountStatus)
			status, err := resolver.onboardingStatus(context.Background())
			if err != nil {
				t.Fatal(err)
			}
			if status.IsUserOnboarded || len(status.Steps) != 2 ||
				status.Steps[1].Status != model.OnboardingStepStatusBlocked ||
				status.Steps[1].ProviderAccountStatus != providerAccountStatusModel(accountStatus) {
				t.Fatalf("provider status %q = %#v", accountStatus, status)
			}
		})
	}
}

// Rust source: crates/noema-host/src/onboarding.rs:288::onboarding_does_not_fabricate_an_account_when_none_is_connected
func TestRustHost_onboarding_does_not_fabricate_an_account_when_none_is_connected(t *testing.T) {
	resolver := openProviderTestResolver(t)
	status, err := resolver.onboardingStatus(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if status.IsUserOnboarded || len(status.Steps) != 1 || status.Steps[0].ID != "install_local_model" ||
		status.Steps[0].ProviderAccountID != "provider_account:local_models:default" ||
		status.Steps[0].Status != model.OnboardingStepStatusBlocked {
		t.Fatalf("unconnected onboarding status = %#v", status)
	}
}

func openRustHostCloudResolver(t *testing.T, status provider.AccountStatus) *Resolver {
	t.Helper()
	resolver := openProviderTestResolver(t)
	rustAPIConfigureModelAccount(t, resolver, "codex")
	ctx := context.Background()
	accountID := "provider_account:codex:default"
	account, err := resolver.ProviderAccounts.LoadAccount(ctx, accountID)
	if err != nil {
		t.Fatal(err)
	}
	account, err = resolver.Store.UpdateProviderCredential(
		ctx, accountID, account.Metadata.CredentialRevision(), provider.AuthOAuthDeviceCode, true, nil, time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	assignments := make([]store.ModelAssignment, 0, len(store.HostedModelRoles()))
	for _, role := range store.HostedModelRoles() {
		assignments = append(assignments, store.ModelAssignment{
			Role: role, ProviderKind: account.ProviderKind, ProviderAccountID: account.ID,
			SelectionMode: store.ModelSelectionNoemaRecommended,
		})
	}
	if _, err := resolver.Store.ConfirmHostedModelAssignments(ctx, account.ID, assignments); err != nil {
		t.Fatal(err)
	}
	if status != provider.StatusAuthenticated {
		if err := resolver.Store.SetProviderAccountStatus(ctx, account.ID, status, "", "", time.Now()); err != nil {
			t.Fatal(err)
		}
	}
	return resolver
}

func openRustHostLocalResolver(t *testing.T, live bool) *Resolver {
	t.Helper()
	resolver := openProviderTestResolver(t)
	ctx := context.Background()
	modelID := "rust-host-local-model"
	blob := []byte("GGUF rust host local model")
	digest := sha256.Sum256(blob)
	digestText := hex.EncodeToString(digest[:])
	blobRelative := filepath.ToSlash(filepath.Join("models", "blobs", digestText+".gguf"))
	blobPath := filepath.Join(resolver.home.Name(), filepath.FromSlash(blobRelative))
	if err := os.MkdirAll(filepath.Dir(blobPath), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blobPath, blob, 0o600); err != nil {
		t.Fatal(err)
	}
	now := time.Now()
	queued, err := resolver.Store.QueueLocalModel(ctx, store.LocalModelInstallation{
		ID: "local_model_installation:rust-host", ModelID: modelID, Name: "Rust host local model",
		File: "rust-host-local.gguf", SourceKind: "local_file", Backend: "cpu",
		TotalBytes: int64(len(blob)), CreatedAt: now,
	})
	if err != nil {
		t.Fatal(err)
	}
	verifying, err := resolver.Store.UpdateLocalModel(ctx, queued.ID, "verifying", int64(len(blob)), int64(len(blob)), 0, "", "", "", "", now)
	if err != nil {
		t.Fatal(err)
	}
	installed, err := resolver.Store.UpdateLocalModel(ctx, verifying.ID, "installed", int64(len(blob)), int64(len(blob)), int64(len(blob)), digestText, blobRelative, "", "", now)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := resolver.Store.ActivateLocalModel(ctx, installed.ID, true, now); err != nil {
		t.Fatal(err)
	}
	runtimeRoot := ""
	if live {
		runtimeRoot = filepath.Join(t.TempDir(), "runtime")
		buildRustHostFakeRuntime(t, runtimeRoot)
	}
	service, err := localmodel.New(resolver.Store, resolver.home.Name(), runtimeRoot)
	if err != nil {
		t.Fatal(err)
	}
	resolver.SetLocalModels(service)
	t.Cleanup(service.Close)
	if live {
		status, err := service.Retry(ctx)
		if err != nil || status != localmodel.RuntimeRunning {
			t.Fatalf("local runtime status = %q, %v", status, err)
		}
	}
	return resolver
}

func buildRustHostFakeRuntime(t *testing.T, runtimeRoot string) {
	t.Helper()
	executable := "llama-server"
	if runtime.GOOS == "windows" {
		executable += ".exe"
	}
	path := filepath.Join(runtimeRoot, "cpu", executable)
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	command := exec.Command("go", "build", "-trimpath", "-o", path, "./testdata/fake_runtime")
	command.Env = append(os.Environ(), "CGO_ENABLED=0")
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("build deterministic local runtime fixture: %v\n%s", err, output)
	}
}
