package graphql

import (
	"context"
	"testing"

	"github.com/kpsuperplane/noema/internal/graphql/model"
)

func TestFreshHomeReportsTruthfulUnavailableLocalModel(t *testing.T) {
	status := localStatus()
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
