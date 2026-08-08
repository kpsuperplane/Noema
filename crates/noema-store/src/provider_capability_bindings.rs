use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use noema_providers::{
    ProviderCapabilityAccountReference, ProviderCapabilityAssignment,
    ProviderCapabilityAssignmentKey,
};

use super::{NoemaStore, StoreError};

impl NoemaStore {
    /// Create or update one provider capability binding.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write/read fails.
    pub(super) async fn upsert_provider_capability_binding(
        &self,
        tool_name: &str,
        capability_id: &str,
        account_reference: &ProviderCapabilityAccountReference,
    ) -> Result<ProviderCapabilityAssignment, StoreError> {
        let binding_id = binding_id(tool_name, capability_id);
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            require_persisted_account(&transaction, account_reference)?;
            let provider_account_id = account_reference.provider_account_id();
            transaction.execute(
                r#"
                INSERT INTO provider_capability_bindings
                  (binding_id, tool_name, capability_id, provider_account_id, updated_at)
                VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                ON CONFLICT(tool_name, capability_id) DO UPDATE SET
                  provider_account_id = excluded.provider_account_id,
                  updated_at = excluded.updated_at
                "#,
                params![binding_id, tool_name, capability_id, provider_account_id,],
            )?;
            let assignment = transaction.query_row(
                r#"
                SELECT binding_id, tool_name, capability_id, provider_account_id
                FROM provider_capability_bindings
                WHERE tool_name = ?1
                  AND capability_id = ?2
                LIMIT 1
                "#,
                params![tool_name, capability_id],
                provider_capability_binding_from_row,
            )?;
            transaction.commit()?;
            Ok(assignment)
        })
        .await
    }

    /// Read one provider capability binding by model-visible tool and capability.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub(crate) async fn provider_capability_binding(
        &self,
        tool_name: &str,
        capability_id: &str,
    ) -> Result<Option<ProviderCapabilityAssignment>, StoreError> {
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

    /// Remove a set of provider capability bindings in one transaction.
    pub(super) async fn clear_provider_capability_bindings(
        &self,
        keys: &[ProviderCapabilityAssignmentKey],
    ) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for key in keys {
                transaction.execute(
                    "DELETE FROM provider_capability_bindings WHERE tool_name = ?1 AND capability_id = ?2",
                    params![key.tool_name_str(), key.capability_id_str()],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }
}

fn require_persisted_account(
    transaction: &Transaction<'_>,
    account_reference: &ProviderCapabilityAccountReference,
) -> Result<(), StoreError> {
    let provider_account_id = account_reference.provider_account_id();
    let account_exists = transaction
        .query_row(
            "SELECT 1 FROM provider_accounts WHERE provider_account_id = ?1 LIMIT 1",
            [provider_account_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !account_exists {
        return Err(StoreError::ProviderAccountNotFound {
            provider_account_id: provider_account_id.to_string(),
        });
    }
    Ok(())
}

fn provider_capability_binding_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ProviderCapabilityAssignment> {
    Ok(ProviderCapabilityAssignment {
        assignment_id: row.get(0)?,
        tool_name: row.get(1)?,
        capability_id: row.get(2)?,
        provider_account_id: row.get(3)?,
    })
}

fn binding_id(tool_name: &str, capability_id: &str) -> String {
    format!("provider_capability_binding:{tool_name}:{capability_id}")
}
