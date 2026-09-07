package main

import "testing"

// Rust source: crates/noema-host/src/config/tests.rs:75::configuration_source_and_provider_contracts
func TestRustHost_configuration_source_and_provider_contracts(t *testing.T) {
	// Existing Go owner: TestEnvironmentOpenAISetupStoresProtectedSecretAndAssignments.
	TestEnvironmentOpenAISetupStoresProtectedSecretAndAssignments(t)
}

// Rust source: crates/noema-host/src/config/tests/default_provider.rs:3::provider_resolution_precedence_and_secrecy_contracts
func TestRustHost_provider_resolution_precedence_and_secrecy_contracts(t *testing.T) {
	// Existing Go owners cover provider setup and explicit selection precedence.
	TestEnvironmentOpenAISetupStoresProtectedSecretAndAssignments(t)
	TestEnvironmentOpenAISetupUsesExplicitCurrentProfile(t)
}

// Rust source: crates/noema-host/src/config/tests/validation.rs:3::configuration_validation_contracts
func TestRustHost_configuration_validation_contracts(t *testing.T) {
	// Existing Go owner: TestEnvironmentOpenAISetupRejectsInvalidExplicitSelection.
	TestEnvironmentOpenAISetupRejectsInvalidExplicitSelection(t)
}
