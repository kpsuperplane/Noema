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
	setup := localModelSetup()
	if setup.IsReady || setup.RuntimeStatus != model.LocalModelRuntimeStatusInactive {
		t.Fatalf("fresh local model setup = %#v", setup)
	}
	if setup.RecommendedModel != nil || setup.Installation != nil {
		t.Fatalf("fresh local model setup advertises unavailable state: %#v", setup)
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
