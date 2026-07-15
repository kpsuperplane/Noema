//! GraphQL projections for local-model catalog, installation, and runtime records.

use async_graphql::Result;

use super::*;

pub(super) async fn load_catalog_views() -> Result<Vec<GraphqlLocalModelCatalogEntry>> {
    tokio::task::spawn_blocking(catalog_views)
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!("local-model hardware probe failed: {error}"))
        })?
}

fn catalog_views() -> Result<Vec<GraphqlLocalModelCatalogEntry>> {
    let catalog = crate::LocalModelCatalog::bundled()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let hardware = crate::local_models::detect_local_hardware_profiles()
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let recommended_model_id = catalog
        .recommend_with_fallback(&hardware)
        .map(|recommendation| recommendation.model.id.as_str());

    Ok(catalog
        .models()
        .iter()
        .map(|model| {
            let selection = catalog.select_build(&model.id, &hardware);
            GraphqlLocalModelCatalogEntry {
                model_id: model.id.clone(),
                name: model.name.clone(),
                license: model.license.clone(),
                priority: i64::from(model.priority),
                repo: model.repo.clone(),
                revision: model.revision.clone(),
                builds: model.builds.iter().map(Into::into).collect(),
                selected_build: selection.map(|selection| selection.build.into()),
                compatible_backend: selection.map(|selection| selection.hardware.backend.into()),
                hardware_fit: selection.map(|selection| GraphqlLocalModelHardwareFit {
                    backend: selection.hardware.backend.into(),
                    ram_gb: unsigned_to_graphql_int(selection.hardware.ram_gb),
                    vram_gb: selection.hardware.vram_gb.map(unsigned_to_graphql_int),
                    unified_memory: selection.hardware.unified_memory,
                    explanation: selection.explanation(),
                }),
                is_recommended: recommended_model_id == Some(model.id.as_str()),
            }
        })
        .collect())
}

pub(super) fn installation_view(
    installation: crate::local_models::LocalModelInstallationRecord,
) -> GraphqlLocalModelInstallation {
    let installed =
        installation.status == crate::local_models::LocalModelInstallationStatus::Installed;
    GraphqlLocalModelInstallation {
        installation_id: installation.installation_id,
        model_id: installation.model_id,
        name: installation.display_name,
        file: installation
            .source_file
            .unwrap_or_else(|| "imported-model.gguf".to_string()),
        source_kind: match installation.source_kind {
            crate::local_models::LocalModelSourceKind::Catalog => {
                GraphqlLocalModelSourceKind::Catalog
            }
            crate::local_models::LocalModelSourceKind::HuggingFace => {
                GraphqlLocalModelSourceKind::PublicGguf
            }
            crate::local_models::LocalModelSourceKind::LocalFile => {
                GraphqlLocalModelSourceKind::LocalFile
            }
        },
        status: match installation.status {
            crate::local_models::LocalModelInstallationStatus::Queued => {
                GraphqlLocalModelInstallationStatus::Queued
            }
            crate::local_models::LocalModelInstallationStatus::Downloading => {
                GraphqlLocalModelInstallationStatus::Downloading
            }
            crate::local_models::LocalModelInstallationStatus::Verifying => {
                GraphqlLocalModelInstallationStatus::Verifying
            }
            crate::local_models::LocalModelInstallationStatus::Installed => {
                GraphqlLocalModelInstallationStatus::Installed
            }
            crate::local_models::LocalModelInstallationStatus::Cancelled => {
                GraphqlLocalModelInstallationStatus::Cancelled
            }
            crate::local_models::LocalModelInstallationStatus::Failed => {
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
    preference: crate::local_models::DefaultModelPreferenceRecord,
) -> GraphqlDefaultModelPreference {
    GraphqlDefaultModelPreference {
        provider_kind: preference.provider_kind,
        provider_account_id: preference.provider_account_id,
        model_profile: preference.model_profile,
        reasoning_effort: preference.reasoning_effort,
    }
}

pub(super) fn event_view(
    event: crate::local_models::LocalModelEventRecord,
    installation: Option<GraphqlLocalModelInstallation>,
) -> GraphqlLocalModelEvent {
    let kind = match event.kind {
        crate::local_models::LocalModelEventKind::Progress => {
            GraphqlLocalModelEventKind::TransferProgress
        }
        crate::local_models::LocalModelEventKind::Activated => {
            GraphqlLocalModelEventKind::ActiveModelChanged
        }
        crate::local_models::LocalModelEventKind::Queued
        | crate::local_models::LocalModelEventKind::Verifying
        | crate::local_models::LocalModelEventKind::Installed
        | crate::local_models::LocalModelEventKind::Failed
        | crate::local_models::LocalModelEventKind::Cancelled
        | crate::local_models::LocalModelEventKind::Removed => {
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
    status: crate::local_models::LocalModelRuntimeStatus,
) -> GraphqlLocalModelRuntimeStatus {
    match status {
        crate::local_models::LocalModelRuntimeStatus::Stopped => {
            GraphqlLocalModelRuntimeStatus::Inactive
        }
        crate::local_models::LocalModelRuntimeStatus::Starting { .. }
        | crate::local_models::LocalModelRuntimeStatus::Retrying { .. } => {
            GraphqlLocalModelRuntimeStatus::Starting
        }
        crate::local_models::LocalModelRuntimeStatus::Ready { .. } => {
            GraphqlLocalModelRuntimeStatus::Running
        }
        crate::local_models::LocalModelRuntimeStatus::Failed { .. } => {
            GraphqlLocalModelRuntimeStatus::Failed
        }
    }
}
