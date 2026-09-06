package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
)

const webPushHTTPSBlocker = "Web Push requires an HTTPS public origin"

func (r *Resolver) webPushStatus(ctx context.Context, endpoint *string) (*model.WebPushStatus, error) {
	if r.Notifications == nil || !r.Notifications.Available() {
		blocker := webPushHTTPSBlocker
		return &model.WebPushStatus{Available: false, Blocker: &blocker}, nil
	}
	hash, ok := auth.BrowserSessionHash(ctx)
	if !ok {
		return nil, errors.New("browser session is unavailable")
	}
	var id string
	var err error
	if endpoint != nil {
		id, err = r.Notifications.Status(ctx, hash, *endpoint)
		if err != nil {
			return nil, err
		}
	}
	key := r.Notifications.ApplicationServerKey()
	status := &model.WebPushStatus{Available: true, ApplicationServerKey: &key}
	if id != "" {
		status.SubscriptionID = &id
	}
	return status, nil
}

func (r *Resolver) registerWebPushSubscription(
	ctx context.Context, input model.RegisterWebPushSubscriptionInput,
) (*model.WebPushStatus, error) {
	hash, ok := auth.BrowserSessionHash(ctx)
	if !ok || r.Notifications == nil {
		return nil, errors.New("browser session is unavailable")
	}
	id, err := r.Notifications.Register(ctx, hash, input.Endpoint, input.P256Dh, input.Auth)
	if err != nil {
		return nil, err
	}
	key := r.Notifications.ApplicationServerKey()
	return &model.WebPushStatus{Available: true, ApplicationServerKey: &key, SubscriptionID: &id}, nil
}

func (r *Resolver) removeWebPushSubscription(ctx context.Context, id string) (bool, error) {
	hash, ok := auth.BrowserSessionHash(ctx)
	if !ok || r.Notifications == nil {
		return false, errors.New("browser session is unavailable")
	}
	return r.Notifications.Remove(ctx, hash, id)
}

func (r *Resolver) webPushPresence(
	ctx context.Context, id string,
) (<-chan *model.WebPushPresenceEvent, error) {
	hash, ok := auth.BrowserSessionHash(ctx)
	if !ok || r.Notifications == nil {
		return nil, errors.New("browser session is unavailable")
	}
	lease, err := r.Notifications.Presence(ctx, hash, id)
	if err != nil {
		return nil, err
	}
	return relayWebPushPresence(ctx, id, lease), nil
}

func relayWebPushPresence(
	ctx context.Context, id string, lease <-chan struct{},
) <-chan *model.WebPushPresenceEvent {
	events := make(chan *model.WebPushPresenceEvent, 1)
	go func() {
		defer close(events)
		select {
		case _, ok := <-lease:
			if !ok {
				return
			}
		case <-ctx.Done():
			return
		}
		select {
		case events <- &model.WebPushPresenceEvent{SubscriptionID: id, Ready: true}:
		case <-ctx.Done():
			return
		}
		select {
		case <-lease:
		case <-ctx.Done():
		}
	}()
	return events
}
