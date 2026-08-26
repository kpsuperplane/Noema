//! GraphQL projections for local-model catalog, installation, and runtime records.

use async_graphql::Result;

use super::*;

pub(super) async fn load_catalog_views(
    manager: &noema_providers::LocalModelManager,
) -> Result<Vec<GraphqlLocalModelCatalogEntry>> {
    let manager = manager.clone();
    tokio::task::spawn_blocking(move || {
        manager
            .catalog_snapshot()
            .map(catalog_views)
            .map_err(graphql_error)
    })
    .await
    .map_err(|error| {
        async_graphql::Error::new(format!("local-model hardware probe failed: {error}"))
    })?
}

fn catalog_views(
    snapshot: noema_providers::LocalModelCatalogSnapshot,
) -> Vec<GraphqlLocalModelCatalogEntry> {
    snapshot
        .entries
        .into_iter()
        .map(|entry| {
            let model = entry.model;
            GraphqlLocalModelCatalogEntry {
                model_id: model.id.clone(),
                name: model.name.clone(),
                license: model.license.clone(),
                priority: i64::from(model.priority),
                repo: model.repo.clone(),
                revision: model.revision.clone(),
                selected_build: entry.selected_build.as_ref().map(Into::into),
                compatible_backend: entry
                    .compatible_hardware
                    .map(|hardware| hardware.backend.into()),
                hardware_fit: entry.compatible_hardware.map(|hardware| {
                    GraphqlLocalModelHardwareFit {
                        backend: hardware.backend.into(),
                        ram_gb: unsigned_to_graphql_int(hardware.ram_gb),
                        vram_gb: hardware.vram_gb.map(unsigned_to_graphql_int),
                        unified_memory: hardware.unified_memory,
                        explanation: entry.compatibility_explanation.unwrap_or_default(),
                    }
                }),
                is_recommended: entry.is_recommended,
            }
        })
        .collect()
}

pub(super) fn installation_view(
    installation: noema_providers::LocalModelInstallationRecord,
) -> GraphqlLocalModelInstallation {
    let installed = installation.status == noema_providers::LocalModelInstallationStatus::Installed;
    GraphqlLocalModelInstallation {
        installation_id: installation.installation_id,
        model_id: installation.model_id,
        name: installation.display_name,
        file: installation
            .source_file
            .unwrap_or_else(|| "imported-model.gguf".to_string()),
        source_kind: match installation.source_kind {
            noema_providers::LocalModelSourceKind::Catalog => GraphqlLocalModelSourceKind::Catalog,
            noema_providers::LocalModelSourceKind::HuggingFace => {
                GraphqlLocalModelSourceKind::PublicGguf
            }
            noema_providers::LocalModelSourceKind::LocalFile => {
                GraphqlLocalModelSourceKind::LocalFile
            }
        },
        status: match installation.status {
            noema_providers::LocalModelInstallationStatus::Queued => {
                GraphqlLocalModelInstallationStatus::Queued
            }
            noema_providers::LocalModelInstallationStatus::Downloading => {
                GraphqlLocalModelInstallationStatus::Downloading
            }
            noema_providers::LocalModelInstallationStatus::Verifying => {
                GraphqlLocalModelInstallationStatus::Verifying
            }
            noema_providers::LocalModelInstallationStatus::Installed => {
                GraphqlLocalModelInstallationStatus::Installed
            }
            noema_providers::LocalModelInstallationStatus::Cancelled => {
                GraphqlLocalModelInstallationStatus::Cancelled
            }
            noema_providers::LocalModelInstallationStatus::Failed => {
                GraphqlLocalModelInstallationStatus::Failed
            }
        },
        sha256: installation.sha256,
        completed_bytes: unsigned_to_graphql_int(installation.downloaded_bytes),
        total_bytes: installation.expected_bytes.map(unsigned_to_graphql_int),
        disk_bytes: if installed {
            unsigned_to_graphql_int(
                installation
                    .expected_bytes
                    .unwrap_or(installation.downloaded_bytes),
            )
        } else {
            unsigned_to_graphql_int(installation.downloaded_bytes)
        },
        backend: Some(installation.backend.into()),
        is_active: installation.is_active,
        error_code: installation.error_code,
        error_message: installation.error_message,
        created_at: installation.created_at,
        updated_at: installation.updated_at,
    }
}

pub(super) fn default_preference_view(
    preference: noema_providers::DefaultModelPreferenceRecord,
) -> GraphqlDefaultModelPreference {
    let selection_mode = (&preference.selection).into();
    let (model_profile, reasoning_effort) = match preference.selection {
        noema_providers::ModelPreferenceSelection::NoemaRecommended => (None, None),
        noema_providers::ModelPreferenceSelection::ExplicitProfile {
            model_profile,
            reasoning_effort,
        } => (
            Some(model_profile),
            reasoning_effort.map(|effort| effort.as_persistence_str().to_string()),
        ),
    };
    GraphqlDefaultModelPreference {
        provider_kind: preference.provider_kind,
        provider_account_id: preference.provider_account_id,
        selection_mode,
        model_profile,
        reasoning_effort,
        fast_mode: preference.fast_mode,
    }
}

pub(super) fn event_view(
    event: noema_providers::LocalModelEventRecord,
    installation: Option<GraphqlLocalModelInstallation>,
) -> GraphqlLocalModelEvent {
    let kind = match event.kind {
        noema_providers::LocalModelEventKind::Progress => {
            GraphqlLocalModelEventKind::TransferProgress
        }
        noema_providers::LocalModelEventKind::Activated => {
            GraphqlLocalModelEventKind::ActiveModelChanged
        }
        noema_providers::LocalModelEventKind::Queued
        | noema_providers::LocalModelEventKind::Verifying
        | noema_providers::LocalModelEventKind::Installed
        | noema_providers::LocalModelEventKind::Failed
        | noema_providers::LocalModelEventKind::Cancelled
        | noema_providers::LocalModelEventKind::Removed => {
            GraphqlLocalModelEventKind::InstallationUpdated
        }
    };
    GraphqlLocalModelEvent {
        cursor: event.cursor.to_string(),
        kind,
        installation_id: Some(event.installation_id),
        model_id: installation
            .as_ref()
            .map(|installation| installation.model_id.clone()),
        installation,
        runtime_status: None,
        created_at: event.created_at,
    }
}

fn unsigned_to_graphql_int(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub(super) fn runtime_status_view(
    status: noema_providers::LocalModelRuntimeStatus,
) -> GraphqlLocalModelRuntimeStatus {
    match status {
        noema_providers::LocalModelRuntimeStatus::Stopped => {
            GraphqlLocalModelRuntimeStatus::Inactive
        }
        noema_providers::LocalModelRuntimeStatus::Starting { .. }
        | noema_providers::LocalModelRuntimeStatus::Retrying { .. } => {
            GraphqlLocalModelRuntimeStatus::Starting
        }
        noema_providers::LocalModelRuntimeStatus::Ready { .. } => {
            GraphqlLocalModelRuntimeStatus::Running
        }
        noema_providers::LocalModelRuntimeStatus::Failed { .. } => {
            GraphqlLocalModelRuntimeStatus::Failed
        }
    }
}
