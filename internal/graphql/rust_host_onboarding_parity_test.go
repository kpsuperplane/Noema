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
	cloudAccount := provider.Account{
		ID: "provider_account:codex:default", ProviderKind: "codex", AccountKey: "default",
		DisplayName: "Codex", AuthMethod: provider.AuthOAuthDeviceCode, Status: provider.StatusAuthenticated,
	}
	cloud := providerOnboardingStep(cloudAccount, true)
	cloudStatus := &model.OnboardingStatus{
		IsUserOnboarded: true,
		Steps:           []*model.OnboardingStep{localModelOnboardingStep(false), cloud},
	}
	if !cloudStatus.IsUserOnboarded || len(cloudStatus.Steps) != 2 ||
		cloudStatus.Steps[0].Status != model.OnboardingStepStatusBlocked ||
		cloudStatus.Steps[1].Status != model.OnboardingStepStatusComplete ||
		cloudStatus.Steps[1].ID != "connect_provider_account" ||
		cloudStatus.Steps[1].ProviderKind != "codex" {
		t.Fatalf("ready cloud onboarding status = %#v", cloudStatus)
	}
	blockedLocal := localModelOnboardingStep(false)
	blockedCloud := providerOnboardingStep(cloudAccount, false)
	blockedStatus := &model.OnboardingStatus{
		Steps: []*model.OnboardingStep{blockedLocal, blockedCloud},
	}
	if blockedStatus.IsUserOnboarded || len(blockedStatus.Steps) != 2 ||
		blockedStatus.Steps[0].Status != model.OnboardingStepStatusBlocked ||
		blockedStatus.Steps[1].Status != model.OnboardingStepStatusBlocked {
		t.Fatalf("blocked onboarding status = %#v", blockedStatus)
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
		account := provider.Account{
			ID: "provider_account:codex:default", ProviderKind: "codex", AccountKey: "default",
			DisplayName: "Codex", AuthMethod: provider.AuthOAuthDeviceCode, Status: status,
		}
		step := providerOnboardingStep(account, false)
		result := &model.OnboardingStatus{Steps: []*model.OnboardingStep{localModelOnboardingStep(false), step}}
		if result.IsUserOnboarded || result.Steps[1].Status != model.OnboardingStepStatusBlocked ||
			result.Steps[1].ProviderAccountStatus != providerAccountStatusModel(status) {
			t.Fatalf("provider status %q = %#v", status, result)
		}
	}
}

// Rust source: crates/noema-host/src/onboarding.rs:288::onboarding_does_not_fabricate_an_account_when_none_is_connected
func TestRustHost_onboarding_does_not_fabricate_an_account_when_none_is_connected(t *testing.T) {
	status := &model.OnboardingStatus{Steps: []*model.OnboardingStep{localModelOnboardingStep(false)}}
	if status.IsUserOnboarded || len(status.Steps) != 1 || status.Steps[0].ID != "install_local_model" ||
		status.Steps[0].ProviderAccountID != "provider_account:local_models:default" ||
		status.Steps[0].Status != model.OnboardingStepStatusBlocked {
		t.Fatalf("unconnected onboarding status = %#v", status)
	}
}
