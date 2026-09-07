package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/notification"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) apnsProviderStatus(ctx context.Context) (*model.ApnsProviderStatus, error) {
	_, browser := auth.BrowserSessionHash(ctx)
	if (!browser && !auth.DesktopAccess(ctx)) || r.Notifications == nil {
		return nil, errors.New("browser session authentication required")
	}
	value, err := r.Notifications.APNSProviderStatus()
	if err != nil {
		return nil, err
	}
	return apnsProviderModel(value), nil
}

func (r *Resolver) configureAPNS(ctx context.Context, input model.ConfigureApnsProviderInput) (*model.ApnsProviderStatus, error) {
	_, browser := auth.BrowserSessionHash(ctx)
	if (!browser && !auth.DesktopAccess(ctx)) || r.Notifications == nil {
		return nil, errors.New("browser session authentication required")
	}
	value, err := r.Notifications.ConfigureAPNS(input.TeamID, input.KeyID, input.PrivateKeyPem, input.ExpectedRevision)
	if err != nil {
		return nil, err
	}
	return apnsProviderModel(value), nil
}

func (r *Resolver) removeAPNS(ctx context.Context, expected int) (*model.ApnsProviderStatus, error) {
	_, browser := auth.BrowserSessionHash(ctx)
	if (!browser && !auth.DesktopAccess(ctx)) || r.Notifications == nil {
		return nil, errors.New("browser session authentication required")
	}
	value, err := r.Notifications.RemoveAPNS(ctx, expected)
	if err != nil {
		return nil, err
	}
	return apnsProviderModel(value), nil
}

func (r *Resolver) clientNotificationStatus(ctx context.Context) (*model.ClientNotificationStatus, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return nil, err
	}
	value, err := r.Notifications.ClientNotificationStatus(ctx, clientID)
	if err != nil {
		return nil, err
	}
	return clientNotificationModel(value), nil
}

func (r *Resolver) registerClientNotifications(ctx context.Context, input model.RegisterClientNotificationsInput) (*model.ClientNotificationStatus, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return nil, err
	}
	value, err := r.Notifications.RegisterClientNotifications(ctx, clientID, input.DeviceToken, apnsEnvironment(input.Environment))
	if err != nil {
		return nil, err
	}
	return clientNotificationModel(value), nil
}

func (r *Resolver) disableClientNotifications(ctx context.Context) (*model.ClientNotificationStatus, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return nil, err
	}
	value, err := r.Notifications.DisableClientNotifications(ctx, clientID)
	if err != nil {
		return nil, err
	}
	return clientNotificationModel(value), nil
}

func (r *Resolver) clientLiveActivityStatus(ctx context.Context) (*model.ClientLiveActivityStatus, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return nil, err
	}
	value, err := r.Notifications.ClientLiveActivityStatus(ctx, clientID)
	if err != nil {
		return nil, err
	}
	return clientLiveActivityModel(value), nil
}

func (r *Resolver) registerClientLiveActivities(ctx context.Context, input model.RegisterClientLiveActivitiesInput) (*model.ClientLiveActivityStatus, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return nil, err
	}
	value, err := r.Notifications.RegisterClientLiveActivities(ctx, clientID, input.PushToStartToken,
		apnsEnvironment(input.Environment), input.ActiveActivityIds)
	if err != nil {
		return nil, err
	}
	return clientLiveActivityModel(value), nil
}

func (r *Resolver) registerClientLiveActivityUpdate(ctx context.Context, input model.RegisterClientLiveActivityUpdateInput) (bool, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return false, err
	}
	return r.Notifications.RegisterClientLiveActivityUpdate(ctx, clientID, input.ActivityID, input.UpdateToken)
}

func (r *Resolver) dismissClientLiveActivity(ctx context.Context, activityID string) (bool, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return false, err
	}
	return r.Notifications.DismissClientLiveActivity(ctx, clientID, activityID)
}

func (r *Resolver) disableClientLiveActivities(ctx context.Context) (*model.ClientLiveActivityStatus, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return nil, err
	}
	value, err := r.Notifications.DisableClientLiveActivities(ctx, clientID)
	if err != nil {
		return nil, err
	}
	return clientLiveActivityModel(value), nil
}

func (r *Resolver) clientNotificationPresence(ctx context.Context) (<-chan *model.ClientNotificationPresenceEvent, error) {
	clientID, err := nativeNotificationClient(ctx, r.Notifications)
	if err != nil {
		return nil, err
	}
	lease, err := r.Notifications.ClientPresence(ctx, clientID)
	if err != nil {
		return nil, err
	}
	events := make(chan *model.ClientNotificationPresenceEvent, 1)
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
		case events <- &model.ClientNotificationPresenceEvent{ClientID: clientID, Ready: true}:
		case <-ctx.Done():
			return
		}
		select {
		case <-lease:
		case <-ctx.Done():
		}
	}()
	return events, nil
}

func nativeNotificationClient(ctx context.Context, service *notification.Service) (string, error) {
	clientID := auth.ClientID(ctx)
	if clientID == "" || service == nil {
		return "", errors.New("paired client authentication required")
	}
	return clientID, nil
}

func apnsProviderModel(value notification.APNSProviderStatus) *model.ApnsProviderStatus {
	return &model.ApnsProviderStatus{Configured: value.Configured, TeamID: value.TeamID, KeyID: value.KeyID,
		Topic: value.Topic, KeyFingerprint: value.KeyFingerprint, Revision: value.Revision,
		UpdatedAt: value.UpdatedAt, LastErrorCode: value.LastErrorCode, LastErrorAt: value.LastErrorAt}
}

func clientNotificationModel(value notification.ClientNotificationStatus) *model.ClientNotificationStatus {
	return &model.ClientNotificationStatus{Available: value.Available, Blocker: value.Blocker,
		Enabled: value.Enabled, Environment: graphqlAPNSEnvironment(value.Environment)}
}

func clientLiveActivityModel(value notification.ClientLiveActivityStatus) *model.ClientLiveActivityStatus {
	return &model.ClientLiveActivityStatus{Available: value.Available, Blocker: value.Blocker,
		Enabled: value.Enabled, Registered: value.Registered, Environment: graphqlAPNSEnvironment(value.Environment)}
}

func apnsEnvironment(value model.ApnsEnvironment) store.APNSEnvironment {
	if value == model.ApnsEnvironmentDevelopment {
		return store.APNSDevelopment
	}
	return store.APNSProduction
}

func graphqlAPNSEnvironment(value *store.APNSEnvironment) *model.ApnsEnvironment {
	if value == nil {
		return nil
	}
	result := model.ApnsEnvironmentProduction
	if *value == store.APNSDevelopment {
		result = model.ApnsEnvironmentDevelopment
	}
	return &result
}
