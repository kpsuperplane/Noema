package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
)

var errLocalModelManagementUnavailable = errors.New("local model installation is not available in this server build")

func localModelCatalog(ctx context.Context) ([]*model.LocalModelCatalogEntry, error) {
	items, err := provider.DiscoverLocalModelCatalog(ctx)
	if err != nil {
		return nil, err
	}
	return localModelCatalogViews(items), nil
}

func localModelSetup(ctx context.Context) (*model.LocalModelSetup, error) {
	catalog, err := localModelCatalog(ctx)
	if err != nil {
		return nil, err
	}
	var recommended *model.LocalModelCatalogEntry
	for _, item := range catalog {
		if item.IsRecommended {
			recommended = item
			break
		}
	}
	return &model.LocalModelSetup{
		RecommendedModel: recommended,
		RuntimeStatus:    model.LocalModelRuntimeStatusInactive,
		IsReady:          false,
	}, nil
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

func localModelBackendView(backend provider.LocalModelBackend) model.LocalModelBackend {
	if backend == provider.LocalModelMetal {
		return model.LocalModelBackendMetal
	}
	return model.LocalModelBackendCPU
}
