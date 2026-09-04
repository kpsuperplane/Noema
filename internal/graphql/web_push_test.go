package graphql

import (
	"context"
	"testing"
)

func TestWebPushPresenceRelayHoldsLeaseUntilStreamEnds(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	lease := make(chan struct{}, 1)
	lease <- struct{}{}
	events := relayWebPushPresence(ctx, "push_subscription:test", lease)
	event := <-events
	if event == nil || !event.Ready || event.SubscriptionID != "push_subscription:test" {
		t.Fatalf("presence ready event = %#v", event)
	}
	select {
	case _, open := <-events:
		if !open {
			t.Fatal("presence stream closed before its lease ended")
		}
	default:
	}
	close(lease)
	if _, open := <-events; open {
		t.Fatal("presence stream remained open after its lease ended")
	}
}
