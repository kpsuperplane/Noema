use serde::Deserialize;
use surrealdb::types::SurrealValue;

use super::{NoemaStore, StoreError, ids::record_fragment};
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
        self.db
            .query(
                r#"
                UPSERT type::record('provider_capability_bindings', $record_id) SET
                  binding_id = $binding_id,
                  tool_name = $tool_name,
                  capability_id = $capability_id,
                  provider_account_id = $provider_account_id,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&binding_id)))
            .bind(("binding_id", binding_id.clone()))
            .bind(("tool_name", tool_name.to_string()))
            .bind(("capability_id", capability_id.to_string()))
            .bind(("provider_account_id", account.provider_account_id))
            .await?
            .check()?;
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT binding_id, tool_name, capability_id, provider_account_id
                FROM provider_capability_bindings
                WHERE tool_name = $tool_name
                  AND capability_id = $capability_id
                LIMIT 1;
                "#,
            )
            .bind(("tool_name", tool_name.to_string()))
            .bind(("capability_id", capability_id.to_string()))
            .await?;
        let rows: Vec<ProviderCapabilityBindingRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(Into::into))
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

#[derive(Debug, Deserialize, SurrealValue)]
struct ProviderCapabilityBindingRow {
    binding_id: String,
    tool_name: String,
    capability_id: String,
    provider_account_id: String,
}

impl From<ProviderCapabilityBindingRow> for ProviderCapabilityBindingRecord {
    fn from(row: ProviderCapabilityBindingRow) -> Self {
        Self {
            binding_id: row.binding_id,
            tool_name: row.tool_name,
            capability_id: row.capability_id,
            provider_account_id: row.provider_account_id,
        }
    }
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
