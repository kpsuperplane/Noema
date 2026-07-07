use rusqlite::{OptionalExtension, params};

use super::{NoemaStore, StoreError};
use crate::ProviderAccountRecord;

/// Persisted binding from a model-visible tool to a concrete provider capability/account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilityBindingRecord {
    /// Stable binding id derived from tool and capability.
    pub binding_id: String,
    /// Model-visible tool name.
    pub tool_name: String,
    /// Bound provider capability id.
    pub capability_id: String,
    /// Provider account selected for this binding.
    pub provider_account_id: String,
}

impl NoemaStore {
    /// Create or update one provider capability binding.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write/read fails.
    pub async fn upsert_provider_capability_binding(
        &self,
        tool_name: &str,
        capability_id: &str,
        provider_account_id: &str,
    ) -> Result<ProviderCapabilityBindingRecord, StoreError> {
        let account = self
            .validate_provider_capability_binding(tool_name, capability_id, provider_account_id)
            .await?;
        let binding_id = binding_id(tool_name, capability_id);
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO provider_capability_bindings
                  (binding_id, tool_name, capability_id, provider_account_id, updated_at)
                VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(tool_name, capability_id) DO UPDATE SET
                  provider_account_id = excluded.provider_account_id,
                  updated_at = excluded.updated_at
                "#,
                params![
                    binding_id,
                    tool_name,
                    capability_id,
                    account.provider_account_id,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.provider_capability_binding(tool_name, capability_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!(
                    "provider capability binding {binding_id} was not readable after save"
                ),
            })
    }

    /// Read one provider capability binding by model-visible tool and capability.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn provider_capability_binding(
        &self,
        tool_name: &str,
        capability_id: &str,
    ) -> Result<Option<ProviderCapabilityBindingRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT binding_id, tool_name, capability_id, provider_account_id
                FROM provider_capability_bindings
                WHERE tool_name = ?1
                  AND capability_id = ?2
                LIMIT 1
                "#,
                params![tool_name, capability_id],
                provider_capability_binding_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    async fn validate_provider_capability_binding(
        &self,
        tool_name: &str,
        capability_id: &str,
        provider_account_id: &str,
    ) -> Result<ProviderAccountRecord, StoreError> {
        validate_binding_pair(tool_name, capability_id)?;
        let account = self
            .get_provider_account(provider_account_id)
            .await?
            .or_else(|| {
                self.system_provider_accounts()
                    .into_iter()
                    .find(|account| account.provider_account_id == provider_account_id)
            })
            .ok_or_else(|| StoreError::ProviderAccountNotFound {
                provider_account_id: provider_account_id.to_string(),
            })?;
        let declares_capability = account
            .capabilities
            .iter()
            .any(|capability| capability.capability_id.as_str() == capability_id);
        if !declares_capability {
            return Err(StoreError::InvalidEnum {
                kind: "provider_capability_binding_provider_account",
                value: account.provider_account_id,
            });
        }
        Ok(account)
    }
}

fn provider_capability_binding_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ProviderCapabilityBindingRecord> {
    Ok(ProviderCapabilityBindingRecord {
        binding_id: row.get(0)?,
        tool_name: row.get(1)?,
        capability_id: row.get(2)?,
        provider_account_id: row.get(3)?,
    })
}

fn binding_id(tool_name: &str, capability_id: &str) -> String {
    format!("provider_capability_binding:{tool_name}:{capability_id}")
}

fn validate_binding_pair(tool_name: &str, capability_id: &str) -> Result<(), StoreError> {
    match (tool_name, capability_id) {
        ("web.search", "web.search") | ("web.fetch", "web.fetch") => Ok(()),
        _ => Err(StoreError::InvalidEnum {
            kind: "provider_capability_binding_pair",
            value: format!("{tool_name}:{capability_id}"),
        }),
    }
}
