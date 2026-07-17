//! Structural validation for persisted local-model lifecycle state.

use std::collections::HashMap;

use crate::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, LocalModelInstallationRecord, LocalModelInstallationStatus,
    LocalModelReconstructionSnapshot, ProviderInstanceKey, local_model_provider_instance_key,
};

use super::LocalModelManagerError;

pub(super) fn validate_reconstruction_snapshot(
    snapshot: &LocalModelReconstructionSnapshot,
) -> Result<Vec<LocalModelInstallationRecord>, LocalModelManagerError> {
    let mut by_key = HashMap::new();
    for installation in &snapshot.installations {
        let key = instance_key(installation)?;
        if by_key.insert(key.clone(), installation).is_some() {
            return Err(LocalModelManagerError::DuplicateInstanceIdentity {
                provider_instance_key: key,
            });
        }
    }
    let mut required = HashMap::new();
    for reference in &snapshot.references {
        let installation = by_key
            .get(&reference.provider_instance_key)
            .ok_or_else(|| LocalModelManagerError::ReferencedInstallationMissing {
                provider_instance_key: reference.provider_instance_key.clone(),
            })?;
        if installation.retirement_claimed_at.is_some() {
            return Err(LocalModelManagerError::ReferencedInstallationClaimed {
                provider_instance_key: reference.provider_instance_key.clone(),
            });
        }
        if installation.runtime_retired_at.is_some() {
            return Err(
                LocalModelManagerError::ReferencedInstallationRuntimeRetired {
                    provider_instance_key: reference.provider_instance_key.clone(),
                },
            );
        }
        validate_runtime_installation(installation)?;
        required.insert(
            reference.provider_instance_key.clone(),
            (*installation).clone(),
        );
    }
    for installation in snapshot
        .installations
        .iter()
        .filter(|installation| installation.is_active)
    {
        ensure_unclaimed(installation)?;
        if installation.runtime_retired_at.is_some() {
            return Err(
                LocalModelManagerError::ReferencedInstallationRuntimeRetired {
                    provider_instance_key: installation.provider_instance_key.clone(),
                },
            );
        }
        validate_runtime_installation(installation)?;
        required.insert(
            installation.provider_instance_key.clone(),
            installation.clone(),
        );
    }
    let mut required = required.into_values().collect::<Vec<_>>();
    required.sort_by(|left, right| {
        left.provider_instance_key
            .as_str()
            .cmp(right.provider_instance_key.as_str())
    });
    Ok(required)
}

pub(super) fn ensure_unclaimed(
    installation: &LocalModelInstallationRecord,
) -> Result<(), LocalModelManagerError> {
    if installation.retirement_claimed_at.is_some() {
        return Err(LocalModelManagerError::ReferencedInstallationClaimed {
            provider_instance_key: installation.provider_instance_key.clone(),
        });
    }
    Ok(())
}

pub(super) fn validate_runtime_installation(
    installation: &LocalModelInstallationRecord,
) -> Result<(), LocalModelManagerError> {
    if installation.status != LocalModelInstallationStatus::Installed {
        return Err(LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "installation status is not installed",
        });
    }
    if installation.sha256.is_none() || installation.blob_relative_path.is_none() {
        return Err(LocalModelManagerError::InstallationNotReady {
            installation_id: installation.installation_id.clone(),
            reason: "verified artifact metadata is incomplete",
        });
    }
    Ok(())
}

pub(super) fn instance_key(
    installation: &LocalModelInstallationRecord,
) -> Result<ProviderInstanceKey, LocalModelManagerError> {
    let expected = local_model_provider_instance_key(
        LOCAL_MODELS_PROVIDER_ACCOUNT_ID,
        &installation.installation_id,
        &installation.model_id,
    )
    .map_err(|_| LocalModelManagerError::InstallationIdentityMismatch {
        installation_id: installation.installation_id.clone(),
    })?;
    if expected != installation.provider_instance_key {
        return Err(LocalModelManagerError::InstallationIdentityMismatch {
            installation_id: installation.installation_id.clone(),
        });
    }
    Ok(installation.provider_instance_key.clone())
}
