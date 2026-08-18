//! SQLite-authoritative provider-selection validation and writer transactions.

use std::time::Duration;

use noema_providers::{
    LOCAL_MODELS_PROVIDER_ACCOUNT_ID, ModelPreferenceSelection, ProviderInstanceKey,
    ProviderReadySelection, ProviderReadySelectionError, ProviderRegistry, ProviderRegistryError,
    ProviderSelectionSnapshot, provider_account_instance_key,
};
use rusqlite::{ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{NoemaStore, StoreError};

const WRITER_RETRY_LIMIT: usize = 8;

/// Whether a local selection must be the currently active canonical route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionEligibility {
    /// Startup may persist a registered hosted route before authentication.
    ConfiguredDefault,
    /// A newly chosen canonical route must name the active local installation.
    Canonical,
    /// A durable retry may preserve an inactive route until retirement wins.
    PreservedFutureReference,
}

#[derive(Clone, Copy)]
pub(crate) enum CanonicalPreferenceOwner<'a> {
    Default,
    Agent(&'a str),
    Auxiliary(&'a str),
}

impl NoemaStore {
    /// Run a complete SQLite writer unit under an immediate transaction.
    ///
    /// A busy/locked failure restarts the closure from the beginning so callers
    /// never commit a selection captured by an earlier transaction attempt.
    pub(crate) async fn with_immediate_transaction_retry<T>(
        &self,
        mut work: impl FnMut(&Transaction<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.with_connection(|conn| {
            conn.busy_timeout(Duration::from_secs(5))?;
            for attempt in 0..WRITER_RETRY_LIMIT {
                let transaction = match conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                {
                    Ok(transaction) => transaction,
                    Err(error) if sqlite_is_busy(&error) && attempt + 1 < WRITER_RETRY_LIMIT => {
                        continue;
                    }
                    Err(error) => return Err(StoreError::Sqlite(error)),
                };
                match work(&transaction) {
                    Ok(value) => match transaction.commit() {
                        Ok(()) => return Ok(value),
                        Err(error)
                            if sqlite_is_busy(&error) && attempt + 1 < WRITER_RETRY_LIMIT =>
                        {
                            continue;
                        }
                        Err(error) => return Err(StoreError::Sqlite(error)),
                    },
                    Err(StoreError::Sqlite(error))
                        if sqlite_is_busy(&error) && attempt + 1 < WRITER_RETRY_LIMIT =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            unreachable!("writer retry loop returns on its final attempt")
        })
        .await
    }
}

/// Resolve an optional instance key and validate the resulting exact snapshot.
pub(crate) fn resolve_provider_selection_tx(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    eligibility: SelectionEligibility,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let mut selection = selection
        .normalized()
        .map_err(|error| StoreError::InvariantViolation {
            message: error.to_string(),
        })?;
    if selection.provider_instance_key.is_none() {
        selection.provider_instance_key = Some(resolve_instance_key_tx(
            transaction,
            &selection,
            eligibility,
        )?);
    }
    validate_provider_selection_tx(transaction, &selection, eligibility)
}

/// Resolve a newly selected canonical route and require readiness proof for its
/// exact provider instance before the transaction is allowed to commit it.
pub(crate) fn resolve_new_canonical_selection_tx(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    ready_selection: Option<&ProviderReadySelection>,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let selection =
        resolve_provider_selection_tx(transaction, selection, SelectionEligibility::Canonical)?;
    let ready_selection =
        ready_selection.ok_or_else(|| StoreError::ProviderInstanceUnavailable {
            provider_instance_key: selection
                .provider_instance_key
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| selection.provider_account_id.clone()),
        })?;
    validate_ready_selection_proof(&selection, ready_selection)?;
    Ok(selection)
}

pub(crate) fn write_preference_tx(
    transaction: &Transaction<'_>,
    owner: CanonicalPreferenceOwner<'_>,
    selection: &ProviderSelectionSnapshot,
    preference: &ModelPreferenceSelection,
    overwrite: bool,
) -> Result<(), StoreError> {
    let (table, owner_column, owner_id) = match owner {
        CanonicalPreferenceOwner::Default => {
            ("default_model_preference", "preference_id", "default")
        }
        CanonicalPreferenceOwner::Agent(agent_id) => {
            ("agent_runtime_preferences", "agent_id", agent_id)
        }
        CanonicalPreferenceOwner::Auxiliary(task_id) => {
            ("auxiliary_model_preferences", "task_id", task_id)
        }
    };
    let key = selection
        .provider_instance_key
        .as_ref()
        .ok_or(StoreError::ProviderInstanceKeyMissing)?;
    let conflict = if overwrite {
        "DO UPDATE SET provider_kind = excluded.provider_kind, provider_account_id = excluded.provider_account_id, provider_instance_key = excluded.provider_instance_key, selection_mode = excluded.selection_mode, model_profile = excluded.model_profile, reasoning_effort = excluded.reasoning_effort, fast_mode = excluded.fast_mode, updated_at = excluded.updated_at"
    } else {
        "DO NOTHING"
    };
    transaction.execute(
        &format!(
            "INSERT INTO {table} ({owner_column}, provider_kind, provider_account_id, provider_instance_key, selection_mode, model_profile, reasoning_effort, fast_mode, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             ON CONFLICT({owner_column}) {conflict}"
        ),
        params![
            owner_id,
            selection.provider_kind,
            selection.provider_account_id,
            key.as_str(),
            preference.as_str(),
            preference.model_profile(),
            preference
                .reasoning_effort()
                .map(noema_providers::ReasoningEffort::as_persistence_str),
            selection.fast_mode,
        ],
    )?;
    Ok(())
}

pub(crate) fn write_task_pool_preference_tx(
    transaction: &Transaction<'_>,
    pool_entry_id: &str,
    complexity: &str,
    selection: &ProviderSelectionSnapshot,
    preference: &ModelPreferenceSelection,
    overwrite: bool,
) -> Result<(), StoreError> {
    let key = selection
        .provider_instance_key
        .as_ref()
        .ok_or(StoreError::ProviderInstanceKeyMissing)?;
    let conflict = if overwrite {
        "DO UPDATE SET provider_kind = excluded.provider_kind, provider_account_id = excluded.provider_account_id, provider_instance_key = excluded.provider_instance_key, selection_mode = excluded.selection_mode, model_profile = excluded.model_profile, reasoning_effort = excluded.reasoning_effort, fast_mode = excluded.fast_mode, enabled = 1, sort_order = 0, updated_at = excluded.updated_at"
    } else {
        "DO NOTHING"
    };
    transaction.execute(
        &format!(
            "INSERT INTO task_model_pool_entries (pool_entry_id, complexity, label, provider_kind, provider_account_id, provider_instance_key, selection_mode, model_profile, reasoning_effort, fast_mode, enabled, sort_order, updated_at) \
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             ON CONFLICT(pool_entry_id) {conflict}"
        ),
        params![
            pool_entry_id,
            complexity,
            selection.provider_kind,
            selection.provider_account_id,
            key.as_str(),
            preference.as_str(),
            preference.model_profile(),
            preference
                .reasoning_effort()
                .map(noema_providers::ReasoningEffort::as_persistence_str),
            selection.fast_mode,
        ],
    )?;
    Ok(())
}

pub(crate) fn explicit_model_preference(
    selection: &ProviderSelectionSnapshot,
) -> Result<ModelPreferenceSelection, StoreError> {
    let model_profile =
        selection
            .model_profile
            .clone()
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "explicit provider selection has no model profile".to_string(),
            })?;
    Ok(ModelPreferenceSelection::ExplicitProfile {
        model_profile,
        reasoning_effort: selection.reasoning_effort,
    })
}

/// Verify that an opaque registry proof covers this exact normalized snapshot.
pub(crate) fn validate_ready_selection_proof(
    selection: &ProviderSelectionSnapshot,
    ready_selection: &ProviderReadySelection,
) -> Result<(), StoreError> {
    let selection =
        selection
            .normalized_for_persistence()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })?;
    let ready = ready_selection.selection();
    let same_route = selection.provider_kind == ready.provider_kind
        && selection.provider_account_id == ready.provider_account_id
        && selection.provider_instance_key == ready.provider_instance_key
        && selection.selection_mode == ready.selection_mode
        && selection.model_profile == ready.model_profile
        && selection.reasoning_effort == ready.reasoning_effort;
    if !same_route {
        return Err(StoreError::ProviderInstanceUnavailable {
            provider_instance_key: selection
                .provider_instance_key
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| selection.provider_account_id.clone()),
        });
    }
    Ok(())
}

/// Mint and retain a readiness proof when a transaction is about to establish
/// a new future reference to an exact provider instance.
pub(crate) fn prove_selection_ready(
    selection: &ProviderSelectionSnapshot,
    registry: &ProviderRegistry,
) -> Result<ProviderReadySelection, StoreError> {
    selection
        .provider_instance_key
        .as_ref()
        .ok_or(StoreError::ProviderInstanceKeyMissing)?;
    registry
        .prove_ready_selection(selection.clone())
        .map_err(ready_selection_error)
}

fn ready_selection_error(error: ProviderReadySelectionError) -> StoreError {
    match error {
        ProviderReadySelectionError::Registry(
            ProviderRegistryError::Missing { key }
            | ProviderRegistryError::Unready { key }
            | ProviderRegistryError::Retiring { key, .. }
            | ProviderRegistryError::LeaseCountExhausted { key, .. },
        ) => StoreError::ProviderInstanceUnavailable {
            provider_instance_key: key.to_string(),
        },
        ProviderReadySelectionError::InvalidSelection(error) => StoreError::InvariantViolation {
            message: error.to_string(),
        },
        other @ ProviderReadySelectionError::Registry(
            ProviderRegistryError::ForeignRegistration | ProviderRegistryError::GenerationExhausted,
        ) => StoreError::InvariantViolation {
            message: other.to_string(),
        },
    }
}

/// Validate an exact snapshot without guessing or replacing its identity.
pub(crate) fn validate_provider_selection_tx(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    eligibility: SelectionEligibility,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let selection =
        selection
            .normalized_for_persistence()
            .map_err(|error| StoreError::InvariantViolation {
                message: error.to_string(),
            })?;
    let key = selection
        .provider_instance_key
        .as_ref()
        .ok_or(StoreError::ProviderInstanceKeyMissing)?;
    validate_account_and_profile_tx(transaction, &selection, eligibility)?;
    if selection.provider_kind == "local_models" {
        validate_local_instance_tx(transaction, &selection, key, eligibility)?;
    } else {
        let expected =
            provider_account_instance_key(&selection.provider_account_id).map_err(|_| {
                StoreError::ProviderInstanceKeyMismatch {
                    provider_instance_key: key.to_string(),
                }
            })?;
        if &expected != key {
            return Err(StoreError::ProviderInstanceKeyMismatch {
                provider_instance_key: key.to_string(),
            });
        }
    }
    Ok(selection)
}

fn resolve_instance_key_tx(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    eligibility: SelectionEligibility,
) -> Result<ProviderInstanceKey, StoreError> {
    if selection.provider_kind != "local_models" {
        return provider_account_instance_key(&selection.provider_account_id).map_err(|error| {
            StoreError::InvariantViolation {
                message: error.to_string(),
            }
        });
    }
    if selection.provider_account_id != LOCAL_MODELS_PROVIDER_ACCOUNT_ID {
        return Err(StoreError::ProviderInstanceUnavailable {
            provider_instance_key: selection.provider_account_id.clone(),
        });
    }
    let model_profile =
        selection
            .model_profile
            .as_deref()
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "local-model selections require an explicit model profile".to_string(),
            })?;
    let active_clause = match eligibility {
        SelectionEligibility::ConfiguredDefault | SelectionEligibility::Canonical => {
            "AND is_active = 1"
        }
        SelectionEligibility::PreservedFutureReference => "",
    };
    let sql = format!(
        "SELECT provider_instance_key FROM local_model_installations \
         WHERE model_id = ?1 AND status = 'installed' \
         AND runtime_retired_at IS NULL AND retirement_claimed_at IS NULL {active_clause} \
         ORDER BY is_active DESC, installation_id LIMIT 2"
    );
    let mut statement = transaction.prepare(&sql)?;
    let mut rows = statement.query([model_profile])?;
    let Some(row) = rows.next()? else {
        return Err(StoreError::ProviderInstanceUnavailable {
            provider_instance_key: model_profile.to_string(),
        });
    };
    let key = ProviderInstanceKey::new(row.get::<_, String>(0)?).map_err(|error| {
        StoreError::InvariantViolation {
            message: error.to_string(),
        }
    })?;
    if rows.next()?.is_some() {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "local model profile resolves to multiple eligible installations: {model_profile}"
            ),
        });
    }
    Ok(key)
}

fn validate_account_and_profile_tx(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    eligibility: SelectionEligibility,
) -> Result<(), StoreError> {
    let account = transaction
        .query_row(
            r#"
            SELECT provider_kind, is_active, status, metadata_json
            FROM provider_accounts
            WHERE provider_account_id = ?1
            LIMIT 1
            "#,
            [&selection.provider_account_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? != 0,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::ProviderAccountNotFound {
            provider_account_id: selection.provider_account_id.clone(),
        })?;
    if account.0 != selection.provider_kind || !account.1 {
        return Err(StoreError::ProviderInstanceUnavailable {
            provider_instance_key: selection.provider_account_id.clone(),
        });
    }
    if eligibility != SelectionEligibility::ConfiguredDefault && account.2 != "authenticated" {
        return Err(StoreError::ProviderInstanceUnavailable {
            provider_instance_key: selection.provider_account_id.clone(),
        });
    }
    if let Some(model_profile) = selection.model_profile.as_deref() {
        let metadata = serde_json::from_str::<serde_json::Value>(&account.3)?;
        if let Some(profiles) = metadata
            .get("profiles")
            .and_then(serde_json::Value::as_array)
            && !profiles.is_empty()
            && !profiles.iter().any(|profile| {
                profile.get("id").and_then(serde_json::Value::as_str) == Some(model_profile)
            })
        {
            return Err(StoreError::ProviderInstanceUnavailable {
                provider_instance_key: model_profile.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_local_instance_tx(
    transaction: &Transaction<'_>,
    selection: &ProviderSelectionSnapshot,
    key: &ProviderInstanceKey,
    eligibility: SelectionEligibility,
) -> Result<(), StoreError> {
    if selection.provider_account_id != LOCAL_MODELS_PROVIDER_ACCOUNT_ID {
        return Err(StoreError::ProviderInstanceKeyMismatch {
            provider_instance_key: key.to_string(),
        });
    }
    let installation = transaction
        .query_row(
            r#"
            SELECT model_id, status, is_active, runtime_retired_at, retirement_claimed_at
            FROM local_model_installations
            WHERE provider_instance_key = ?1
            LIMIT 1
            "#,
            [key.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? != 0,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::ProviderInstanceUnavailable {
            provider_instance_key: key.to_string(),
        })?;
    if installation.4.is_some() {
        return Err(StoreError::ProviderInstanceClaimed {
            provider_instance_key: key.to_string(),
        });
    }
    if installation.3.is_some() {
        return Err(StoreError::ProviderInstanceUnavailable {
            provider_instance_key: key.to_string(),
        });
    }
    if installation.0 != selection.model_profile.as_deref().unwrap_or_default() {
        return Err(StoreError::ProviderInstanceKeyMismatch {
            provider_instance_key: key.to_string(),
        });
    }
    if installation.1 != "installed"
        || (eligibility != SelectionEligibility::PreservedFutureReference && !installation.2)
    {
        return Err(StoreError::ProviderInstanceUnavailable {
            provider_instance_key: key.to_string(),
        });
    }
    Ok(())
}

fn sqlite_is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_classifier_is_narrow() {
        let busy = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            None,
        );
        assert!(sqlite_is_busy(&busy));
        assert!(!sqlite_is_busy(&rusqlite::Error::InvalidQuery));
    }
}
