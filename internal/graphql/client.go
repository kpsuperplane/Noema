package graphql

import (
	"context"
	"errors"
	"time"

	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) clients(ctx context.Context) ([]*model.Client, error) {
	if r.Auth == nil {
		return nil, errors.New("native client authority is unavailable")
	}
	records, err := r.Auth.Clients(ctx)
	if err != nil {
		return nil, err
	}
	current := auth.ClientID(ctx)
	result := make([]*model.Client, 0, len(records))
	for _, record := range records {
		result = append(result, nativeClientModel(record, current))
	}
	return result, nil
}

func (r *Resolver) revokeClient(ctx context.Context, clientID string) (*model.Client, error) {
	if r.Auth == nil {
		return nil, errors.New("native client authority is unavailable")
	}
	record, err := r.Auth.RevokeClient(ctx, clientID)
	if err != nil {
		return nil, err
	}
	return nativeClientModel(record, auth.ClientID(ctx)), nil
}

func (r *Resolver) revokeAllClients(ctx context.Context) (int, error) {
	if r.Auth == nil {
		return 0, errors.New("native client authority is unavailable")
	}
	return r.Auth.RevokeAllClients(ctx)
}

func nativeClientModel(record store.NativeOAuthClient, current string) *model.Client {
	result := &model.Client{
		ClientID: record.ClientID, DisplayName: record.DisplayName,
		CreatedAt: record.CreatedAt.Format(time.RFC3339Nano), IsCurrent: record.ClientID == current,
	}
	if record.RevokedAt != nil {
		value := record.RevokedAt.Format(time.RFC3339Nano)
		result.RevokedAt = &value
	}
	return result
}
