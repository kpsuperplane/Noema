package foundation

import (
	"context"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// Host is the assembled Foundation-backed host state.
type Host struct {
	Store *store.Store
}

// AssembleHost probes the configured Foundation bridge, creates the host
// store, and persists the selected profile as the default model preference.
func AssembleHost(ctx context.Context, paths home.Paths, bridgePath, profile string) (*Host, error) {
	selection, err := ResolveDefaultSelection(ctx, bridgePath, profile)
	if err != nil {
		return nil, err
	}
	database, err := store.Open(ctx, paths.Database())
	if err != nil {
		return nil, err
	}
	closeOnError := true
	defer func() {
		if closeOnError {
			_ = database.Close()
		}
	}()
	if err := database.EnsureBuiltinProviderAccounts(ctx, time.Now()); err != nil {
		return nil, err
	}
	accountID := "provider_account:" + selection.ProviderKind + ":default"
	if _, err := database.ProviderAccount(ctx, accountID); err != nil {
		return nil, err
	}
	if err := database.SetProviderAccountStatus(ctx, accountID, provider.StatusAuthenticated, "", "", time.Now()); err != nil {
		return nil, err
	}
	if _, err := database.SaveDefaultModelPreference(ctx, store.ModelAssignment{
		ProviderKind:      selection.ProviderKind,
		ProviderAccountID: accountID,
		SelectionMode:     store.ModelSelectionExplicitProfile,
		ModelProfile:      selection.ModelProfile,
	}, time.Now()); err != nil {
		return nil, err
	}
	closeOnError = false
	return &Host{Store: database}, nil
}

// Shutdown closes the assembled host store.
func (h *Host) Shutdown() error {
	if h == nil || h.Store == nil {
		return nil
	}
	err := h.Store.Close()
	h.Store = nil
	return err
}
