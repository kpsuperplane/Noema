package graphql

import (
	"context"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
)

func TestFreshHomeReportsTruthfulUnavailableLocalModel(t *testing.T) {
	resolver := openProviderTestResolver(t)
	status, err := resolver.localStatus(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if status.PrimaryAgentDisplayName != nil {
		t.Fatalf("fresh primary agent name = %q, want nil", *status.PrimaryAgentDisplayName)
	}
	setup, err := localModelSetup(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if setup.IsReady || setup.RuntimeStatus != model.LocalModelRuntimeStatusInactive {
		t.Fatalf("fresh local model setup = %#v", setup)
	}
	if setup.Installation != nil {
		t.Fatalf("fresh local model setup advertises unavailable state: %#v", setup)
	}
}

func TestLocalModelSettingsReadContract(t *testing.T) {
	resolver := openProviderTestResolver(t)
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	response := postGraphQL(t, server.URL, `query {
  localModelSetup { isReady runtimeStatus recommendedModel { modelId } }
  localModelCatalog {
    modelId name license priority repo revision isRecommended compatibleBackend
    selectedBuild { file sha256 downloadGb backends }
    hardwareFit { backend ramGb vramGb unifiedMemory explanation }
  }
  localModelInstallations { installationId }
  defaultModelPreference { providerKind }
}`, nil)
	setup := response.Data["localModelSetup"].(map[string]any)
	if setup["isReady"] != false || setup["runtimeStatus"] != "INACTIVE" {
		t.Fatalf("local model setup = %#v", setup)
	}
	catalog := response.Data["localModelCatalog"].([]any)
	if len(catalog) != 1 || catalog[0].(map[string]any)["modelId"] != "gemma-4-e4b-it" {
		t.Fatalf("local model catalog = %#v", catalog)
	}
	if installations := response.Data["localModelInstallations"].([]any); len(installations) != 0 {
		t.Fatalf("local model installations = %#v", installations)
	}
	if response.Data["defaultModelPreference"] != nil {
		t.Fatalf("default model preference = %#v", response.Data["defaultModelPreference"])
	}
}

func TestLocalModelManagementReturnsUnavailable(t *testing.T) {
	resolver := openProviderTestResolver(t)
	if _, err := resolver.MutationRoot().InstallLocalModel(context.Background(), model.InstallLocalModelInput{ModelID: "gemma-4-e4b-it"}); err != errLocalModelManagementUnavailable {
		t.Fatalf("install error = %v", err)
	}
	if status, err := resolver.MutationRoot().RetryLocalModelRuntime(context.Background()); status != model.LocalModelRuntimeStatusInactive || err != errLocalModelManagementUnavailable {
		t.Fatalf("retry result = %s, %v", status, err)
	}
}

func TestLocalStatusReturnsStoredPrimaryAgentName(t *testing.T) {
	resolver := openProviderTestResolver(t)
	if _, err := resolver.Store.UpdatePrimaryAgentDisplayName(
		context.Background(), "Mira", time.Now(),
	); err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	response := postGraphQL(t, server.URL, `query { localStatus { primaryAgentDisplayName } }`, nil)
	local := response.Data["localStatus"].(map[string]any)
	if local["primaryAgentDisplayName"] != "Mira" {
		t.Fatalf("localStatus = %#v", local)
	}
}

func TestUnavailableEventStreamsStayOpenUntilDisconnect(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	localEvents := localModelEvents(ctx)
	taskEvents := tasksEvents(ctx)
	select {
	case <-localEvents:
		t.Fatal("local model event stream closed before disconnect")
	default:
	}
	select {
	case <-taskEvents:
		t.Fatal("Tasks event stream closed before disconnect")
	default:
	}
	cancel()
	if _, open := <-localEvents; open {
		t.Fatal("local model event stream stayed open after disconnect")
	}
	if _, open := <-taskEvents; open {
		t.Fatal("Tasks event stream stayed open after disconnect")
	}
}
