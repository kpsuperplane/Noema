package foundation

import (
	"context"
	"errors"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// ConfigureDefault probes the configured Foundation bridge and persists its
// selected profile through the host's existing store.
func ConfigureDefault(ctx context.Context, database *store.Store, bridgePath, profile string, now time.Time) error {
	if database == nil {
		return errors.New("Foundation host store is unavailable")
	}
	selection, err := ResolveDefaultSelection(ctx, bridgePath, profile)
	if err != nil {
		return err
	}
	accountID := "provider_account:" + selection.ProviderKind + ":default"
	if _, err := database.ProviderAccount(ctx, accountID); err != nil {
		return err
	}
	if err := database.SetProviderAccountStatus(ctx, accountID, provider.StatusAuthenticated, "", "", now); err != nil {
		return err
	}
	if _, err := database.SaveDefaultModelPreference(ctx, store.ModelAssignment{
		ProviderKind:      selection.ProviderKind,
		ProviderAccountID: accountID,
		SelectionMode:     store.ModelSelectionExplicitProfile,
		ModelProfile:      selection.ModelProfile,
	}, now); err != nil {
		return err
	}
	return nil
}
