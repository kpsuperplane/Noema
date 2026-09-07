package graphql

import (
	"testing"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
)

// Rust source: crates/noema-host/src/onboarding.rs:233::local_model_and_cloud_are_alternative_onboarding_paths
func TestRustHost_local_model_and_cloud_are_alternative_onboarding_paths(t *testing.T) {
	local := localModelOnboardingStep(true)
	if local.Status != model.OnboardingStepStatusComplete || local.ID != "install_local_model" || local.ProviderAccountStatus != model.ProviderAccountStatusAuthenticated {
		t.Fatalf("ready local onboarding step = %#v", local)
	}
	cloud := providerOnboardingStep(provider.Account{
		ID: "provider_account:codex:default", ProviderKind: "codex", AccountKey: "default",
		DisplayName: "Codex", AuthMethod: provider.AuthOAuthDeviceCode, Status: provider.StatusAuthenticated,
	}, true)
	if cloud.Status != model.OnboardingStepStatusComplete || cloud.ID != "connect_provider_account" || cloud.ProviderKind != "codex" {
		t.Fatalf("ready cloud onboarding step = %#v", cloud)
	}
	blockedLocal := localModelOnboardingStep(false)
	blockedCloud := providerOnboardingStep(provider.Account{
		ID: "provider_account:codex:default", ProviderKind: "codex", AccountKey: "default",
		DisplayName: "Codex", AuthMethod: provider.AuthOAuthDeviceCode, Status: provider.StatusUnknown,
	}, false)
	if blockedLocal.Status != model.OnboardingStepStatusBlocked || blockedCloud.Status != model.OnboardingStepStatusBlocked {
		t.Fatalf("blocked onboarding steps = %#v, %#v", blockedLocal, blockedCloud)
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
			step := localModelOnboardingStep(test.ready)
			status := &model.OnboardingStatus{IsUserOnboarded: test.ready, Steps: []*model.OnboardingStep{step}}
			if status.IsUserOnboarded != test.ready || (status.Steps[0].Status == model.OnboardingStepStatusComplete) != test.ready {
				t.Fatalf("runtime readiness %t = %#v", test.ready, status)
			}
		})
	}
}

// Rust source: crates/noema-host/src/onboarding.rs:272::onboarding_blocks_and_preserves_provider_readiness_status
func TestRustHost_onboarding_blocks_and_preserves_provider_readiness_status(t *testing.T) {
	for _, status := range []provider.AccountStatus{
		provider.StatusUnknown, provider.StatusChecking, provider.StatusUnauthenticated, provider.StatusUnavailable,
	} {
		step := providerOnboardingStep(provider.Account{
			ID: "provider_account:codex:default", ProviderKind: "codex", AccountKey: "default",
			DisplayName: "Codex", AuthMethod: provider.AuthOAuthDeviceCode, Status: status,
		}, false)
		if step.Status != model.OnboardingStepStatusBlocked || string(step.ProviderAccountStatus) != string(status) {
			t.Fatalf("provider status %q = %#v", status, step)
		}
	}
}

// Rust source: crates/noema-host/src/onboarding.rs:288::onboarding_does_not_fabricate_an_account_when_none_is_connected
func TestRustHost_onboarding_does_not_fabricate_an_account_when_none_is_connected(t *testing.T) {
	step := localModelOnboardingStep(false)
	if step.ID != "install_local_model" || step.ProviderAccountID != "provider_account:local_models:default" || step.Status != model.OnboardingStepStatusBlocked {
		t.Fatalf("unconnected onboarding step = %#v", step)
	}
}
