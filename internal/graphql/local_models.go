package graphql

import (
	"context"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/localmodel"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

var errLocalModelManagementUnavailable = errors.New("local model installation is not available in this server build")

func (r *Resolver) localModelCatalog(ctx context.Context) ([]*model.LocalModelCatalogEntry, error) {
	if r.LocalModels == nil {
		return localModelCatalog(ctx)
	}
	items, err := r.LocalModels.Catalog(ctx)
	if err != nil {
		return nil, err
	}
	return localModelCatalogViews(items), nil
}

func localModelCatalog(ctx context.Context) ([]*model.LocalModelCatalogEntry, error) {
	items, err := provider.DiscoverLocalModelCatalog(ctx)
	if err != nil {
		return nil, err
	}
	return localModelCatalogViews(items), nil
}

func (r *Resolver) localModelSetup(ctx context.Context) (*model.LocalModelSetup, error) {
	if r.LocalModels == nil {
		return localModelSetup(ctx)
	}
	catalog, err := r.localModelCatalog(ctx)
	if err != nil {
		return nil, err
	}
	recommended := recommendedLocalModel(catalog)
	installations, err := r.LocalModels.Installations(ctx)
	if err != nil {
		return nil, err
	}
	var selected *store.LocalModelInstallation
	for index := range installations {
		if installations[index].Active {
			selected = &installations[index]
			break
		}
	}
	if selected == nil && recommended != nil {
		for index := range installations {
			if installations[index].ModelID == recommended.ModelID {
				selected = &installations[index]
				break
			}
		}
	}
	runtimeStatus := localModelRuntimeStatusView(r.LocalModels.RuntimeStatus())
	var installation *model.LocalModelInstallation
	if selected != nil {
		installation = localModelInstallationView(*selected)
	}
	return &model.LocalModelSetup{
		RecommendedModel: recommended,
		Installation:     installation,
		RuntimeStatus:    runtimeStatus,
		IsReady: installation != nil && installation.IsActive &&
			installation.Status == model.LocalModelInstallationStatusInstalled &&
			runtimeStatus == model.LocalModelRuntimeStatusRunning,
	}, nil
}

func localModelSetup(ctx context.Context) (*model.LocalModelSetup, error) {
	catalog, err := localModelCatalog(ctx)
	if err != nil {
		return nil, err
	}
	return &model.LocalModelSetup{
		RecommendedModel: recommendedLocalModel(catalog),
		RuntimeStatus:    model.LocalModelRuntimeStatusInactive,
	}, nil
}

func recommendedLocalModel(catalog []*model.LocalModelCatalogEntry) *model.LocalModelCatalogEntry {
	for _, item := range catalog {
		if item.IsRecommended {
			return item
		}
	}
	return nil
}

func (r *Resolver) localModelInstallations(ctx context.Context) ([]*model.LocalModelInstallation, error) {
	if r.LocalModels == nil {
		return []*model.LocalModelInstallation{}, nil
	}
	installations, err := r.LocalModels.Installations(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]*model.LocalModelInstallation, 0, len(installations))
	for _, installation := range installations {
		result = append(result, localModelInstallationView(installation))
	}
	return result, nil
}

func (r *Resolver) defaultModelPreference(ctx context.Context) (*model.DefaultModelPreference, error) {
	preference, err := r.Store.DefaultModelPreference(ctx)
	if err != nil || preference == nil {
		return nil, err
	}
	return defaultModelPreferenceView(*preference), nil
}

func (r *Resolver) installLocalModel(
	ctx context.Context,
	input model.InstallLocalModelInput,
) (*model.LocalModelInstallation, error) {
	if r.LocalModels == nil {
		return nil, errLocalModelManagementUnavailable
	}
	file := ""
	if input.File != nil {
		file = strings.TrimSpace(*input.File)
	}
	installation, err := r.LocalModels.Install(ctx, strings.TrimSpace(input.ModelID), file)
	if err != nil {
		return nil, err
	}
	return localModelInstallationView(installation), nil
}

func (r *Resolver) importLocalModel(
	ctx context.Context,
	input model.ImportLocalModelInput,
) (*model.LocalModelInstallation, error) {
	if r.LocalModels == nil {
		return nil, errLocalModelManagementUnavailable
	}
	if input.SourceKind == model.LocalModelSourceKindCatalog {
		return nil, errors.New("catalog models must use installLocalModel")
	}
	installation, err := r.LocalModels.Import(ctx, localmodel.ImportInput{
		Name:       input.Name,
		SourceKind: strings.ToLower(input.SourceKind.String()),
		LocalPath:  trimOptionalString(input.LocalPath),
		Repo:       trimOptionalString(input.Repo),
		Revision:   trimOptionalString(input.Revision),
		File:       trimOptionalString(input.File),
		SHA256:     trimOptionalString(input.Sha256),
		License:    trimOptionalString(input.License),
	})
	if err != nil {
		return nil, err
	}
	return localModelInstallationView(installation), nil
}

func (r *Resolver) cancelLocalModelInstall(
	ctx context.Context,
	installationID string,
) (*model.LocalModelInstallation, error) {
	if r.LocalModels == nil {
		return nil, errLocalModelManagementUnavailable
	}
	installation, err := r.LocalModels.Cancel(ctx, strings.TrimSpace(installationID))
	if err != nil {
		return nil, err
	}
	return localModelInstallationView(installation), nil
}

func (r *Resolver) removeLocalModel(ctx context.Context, installationID string) (bool, error) {
	if r.LocalModels == nil {
		return false, errLocalModelManagementUnavailable
	}
	return r.LocalModels.Remove(ctx, strings.TrimSpace(installationID))
}

func (r *Resolver) activateLocalModel(
	ctx context.Context,
	installationID string,
) (*model.LocalModelInstallation, error) {
	if r.LocalModels == nil {
		return nil, errLocalModelManagementUnavailable
	}
	installation, err := r.LocalModels.Activate(ctx, strings.TrimSpace(installationID))
	if err != nil {
		return nil, err
	}
	return localModelInstallationView(installation), nil
}

func (r *Resolver) saveDefaultModelPreference(
	ctx context.Context,
	input model.SaveDefaultModelPreferenceInput,
) (*model.DefaultModelPreference, error) {
	providerKind := strings.TrimSpace(strings.ToLower(input.ProviderKind))
	if providerKind == "local_models" {
		return nil, errors.New("Activate a specific local model installation to change the local default")
	}
	account, err := r.selectableModelAccount(ctx, input.ProviderAccountID)
	if err != nil {
		return nil, err
	}
	if providerKind != account.ProviderKind {
		return nil, errors.New("provider kind does not match provider account")
	}
	preference, err := assignmentFromPreference(
		account,
		store.HostedModelNoema,
		provider.ModelUsePrimary,
		input.SelectionMode,
		input.ModelProfile,
		input.ReasoningEffort,
		input.FastMode,
	)
	if err != nil {
		return nil, err
	}
	saved, err := r.Store.SaveDefaultModelPreference(ctx, preference, time.Now())
	if err != nil {
		return nil, err
	}
	return defaultModelPreferenceView(saved), nil
}

func (r *Resolver) retryLocalModelRuntime(ctx context.Context) (model.LocalModelRuntimeStatus, error) {
	if r.LocalModels == nil {
		return model.LocalModelRuntimeStatusInactive, errLocalModelManagementUnavailable
	}
	status, err := r.LocalModels.Retry(ctx)
	return localModelRuntimeStatusView(status), err
}

func (r *Resolver) localModelEvents(
	ctx context.Context,
	after *string,
) (<-chan *model.LocalModelEvent, error) {
	if r.LocalModels == nil {
		return localModelEvents(ctx), nil
	}
	cursor := ""
	if after != nil {
		cursor = strings.TrimSpace(*after)
	}
	events, err := r.LocalModels.Subscribe(ctx, cursor)
	if err != nil {
		return nil, err
	}
	output := make(chan *model.LocalModelEvent)
	go func() {
		defer close(output)
		for event := range events {
			select {
			case output <- localModelEventView(event):
			case <-ctx.Done():
				return
			}
		}
	}()
	return output, nil
}

func localModelCatalogViews(items []provider.LocalModelCatalogItem) []*model.LocalModelCatalogEntry {
	result := make([]*model.LocalModelCatalogEntry, 0, len(items))
	for _, item := range items {
		view := &model.LocalModelCatalogEntry{
			ModelID: item.ID, Name: item.Name, License: item.License,
			Priority: item.Priority, Repo: item.Repo, Revision: item.Revision,
			IsRecommended: item.Recommended,
		}
		if item.SelectedBuild != nil {
			backends := make([]model.LocalModelBackend, 0, len(item.SelectedBuild.Backends))
			for _, backend := range item.SelectedBuild.Backends {
				backends = append(backends, localModelBackendView(backend))
			}
			view.SelectedBuild = &model.LocalModelBuild{
				File: item.SelectedBuild.File, Sha256: item.SelectedBuild.SHA256,
				DownloadGb: item.SelectedBuild.DownloadGB, Backends: backends,
			}
		}
		if item.Hardware != nil {
			backend := localModelBackendView(item.Hardware.Backend)
			view.CompatibleBackend = &backend
			view.HardwareFit = &model.LocalModelHardwareFit{
				Backend: backend, RAMGb: item.Hardware.RAMGB, VramGb: item.Hardware.VRAMGB,
				UnifiedMemory: item.Hardware.UnifiedMemory, Explanation: item.Explanation,
			}
		}
		result = append(result, view)
	}
	return result
}

func localModelInstallationView(value store.LocalModelInstallation) *model.LocalModelInstallation {
	result := &model.LocalModelInstallation{
		InstallationID: value.ID,
		ModelID:        value.ModelID,
		Name:           value.Name,
		File:           value.File,
		SourceKind:     model.LocalModelSourceKind(strings.ToUpper(value.SourceKind)),
		Status:         model.LocalModelInstallationStatus(strings.ToUpper(value.Status)),
		CompletedBytes: int(value.CompletedBytes),
		DiskBytes:      int(value.DiskBytes),
		IsActive:       value.Active,
		CreatedAt:      value.CreatedAt.Format(time.RFC3339Nano),
		UpdatedAt:      value.UpdatedAt.Format(time.RFC3339Nano),
	}
	result.Sha256 = nonEmptyString(value.SHA256)
	result.ErrorCode = nonEmptyString(value.ErrorCode)
	result.ErrorMessage = nonEmptyString(value.ErrorMessage)
	if value.TotalBytes > 0 {
		total := int(value.TotalBytes)
		result.TotalBytes = &total
	}
	if value.Backend != "" {
		backend := model.LocalModelBackend(strings.ToUpper(value.Backend))
		result.Backend = &backend
	}
	return result
}

func localModelEventView(value localmodel.Event) *model.LocalModelEvent {
	result := &model.LocalModelEvent{
		Cursor:    value.Cursor,
		Kind:      model.LocalModelEventKind(strings.ToUpper(value.Kind)),
		CreatedAt: value.CreatedAt.Format(time.RFC3339Nano),
	}
	result.InstallationID = nonEmptyString(value.InstallationID)
	result.ModelID = nonEmptyString(value.ModelID)
	if value.Installation != nil {
		result.Installation = localModelInstallationView(*value.Installation)
	}
	if value.Kind == "runtime_changed" {
		status := localModelRuntimeStatusView(value.RuntimeStatus)
		result.RuntimeStatus = &status
	}
	return result
}

func defaultModelPreferenceView(value store.ModelAssignment) *model.DefaultModelPreference {
	result := &model.DefaultModelPreference{
		ProviderKind:      value.ProviderKind,
		ProviderAccountID: value.ProviderAccountID,
		SelectionMode:     model.ModelPreferenceSelectionModeNoemaRecommended,
		FastMode:          value.FastMode,
	}
	if value.SelectionMode == store.ModelSelectionExplicitProfile {
		result.SelectionMode = model.ModelPreferenceSelectionModeExplicitProfile
		result.ModelProfile = nonEmptyString(value.ModelProfile)
	}
	if value.ReasoningEffort != "" {
		effort := string(value.ReasoningEffort)
		result.ReasoningEffort = &effort
	}
	return result
}

func localModelRuntimeStatusView(value localmodel.RuntimeStatus) model.LocalModelRuntimeStatus {
	status := model.LocalModelRuntimeStatus(strings.ToUpper(string(value)))
	if status.IsValid() {
		return status
	}
	return model.LocalModelRuntimeStatusInactive
}

func localModelBackendView(backend provider.LocalModelBackend) model.LocalModelBackend {
	value := model.LocalModelBackend(strings.ToUpper(string(backend)))
	if value.IsValid() {
		return value
	}
	return model.LocalModelBackendCPU
}

func trimOptionalString(value *string) string {
	if value == nil {
		return ""
	}
	return strings.TrimSpace(*value)
}

func nonEmptyString(value string) *string {
	if value == "" {
		return nil
	}
	copy := value
	return &copy
}
