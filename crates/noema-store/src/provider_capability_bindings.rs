use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use noema_providers::{
    ProviderCapabilityAccountReference, ProviderCapabilityAssignment,
    ProviderCapabilityAssignmentKey, ReplaceProviderCapabilityRouteRequest,
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
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            require_persisted_account(&transaction, account_reference)?;
            let mut account_ids = if tool_name == "web.browse" {
                provider_capability_route_account_ids(&transaction, tool_name, capability_id)?
            } else {
                Vec::new()
            };
            account_ids.retain(|account_id| account_id != account_reference.provider_account_id());
            account_ids.insert(0, account_reference.provider_account_id().to_string());
            replace_provider_capability_route_rows(
                &transaction,
                tool_name,
                capability_id,
                &account_ids,
            )?;
            let assignment =
                provider_capability_route_rows(&transaction, tool_name, capability_id)?
                    .into_iter()
                    .next()
                    .ok_or_else(|| StoreError::InvariantViolation {
                        message: "provider capability route replacement returned no rows"
                            .to_string(),
                    })?;
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
                  AND route_position = 0
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

    /// Read all provider capability bindings in route order.
    pub(crate) async fn provider_capability_route(
        &self,
        tool_name: &str,
        capability_id: &str,
    ) -> Result<Vec<ProviderCapabilityAssignment>, StoreError> {
        self.with_connection(|conn| provider_capability_route_rows(conn, tool_name, capability_id))
            .await
    }

    /// Replace one ordered provider capability route.
    pub(super) async fn replace_provider_capability_route(
        &self,
        request: &ReplaceProviderCapabilityRouteRequest,
    ) -> Result<Vec<ProviderCapabilityAssignment>, StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for reference in request.account_references() {
                require_persisted_account(&transaction, reference)?;
            }
            let account_ids = request
                .account_references()
                .iter()
                .map(|reference| reference.provider_account_id().to_string())
                .collect::<Vec<_>>();
            replace_provider_capability_route_rows(
                &transaction,
                request.key().tool_name_str(),
                request.key().capability_id_str(),
                &account_ids,
            )?;
            let route = provider_capability_route_rows(
                &transaction,
                request.key().tool_name_str(),
                request.key().capability_id_str(),
            )?;
            transaction.commit()?;
            Ok(route)
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

fn provider_capability_route_rows(
    connection: &rusqlite::Connection,
    tool_name: &str,
    capability_id: &str,
) -> Result<Vec<ProviderCapabilityAssignment>, StoreError> {
    let mut statement = connection.prepare(
        r#"
        SELECT binding_id, tool_name, capability_id, provider_account_id
        FROM provider_capability_bindings
        WHERE tool_name = ?1 AND capability_id = ?2
        ORDER BY route_position ASC
        "#,
    )?;
    statement
        .query_map(
            params![tool_name, capability_id],
            provider_capability_binding_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
}

fn provider_capability_route_account_ids(
    connection: &rusqlite::Connection,
    tool_name: &str,
    capability_id: &str,
) -> Result<Vec<String>, StoreError> {
    Ok(
        provider_capability_route_rows(connection, tool_name, capability_id)?
            .into_iter()
            .map(|assignment| assignment.provider_account_id)
            .collect(),
    )
}

fn replace_provider_capability_route_rows(
    transaction: &Transaction<'_>,
    tool_name: &str,
    capability_id: &str,
    account_ids: &[String],
) -> Result<(), StoreError> {
    transaction.execute(
        "DELETE FROM provider_capability_bindings WHERE tool_name = ?1 AND capability_id = ?2",
        params![tool_name, capability_id],
    )?;
    for (position, provider_account_id) in account_ids.iter().enumerate() {
        transaction.execute(
            r#"
            INSERT INTO provider_capability_bindings (
              binding_id, tool_name, capability_id, provider_account_id, route_position
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                binding_id(tool_name, capability_id, position),
                tool_name,
                capability_id,
                provider_account_id,
                position,
            ],
        )?;
    }
    Ok(())
}

fn binding_id(tool_name: &str, capability_id: &str, position: usize) -> String {
    if position == 0 {
        format!("provider_capability_binding:{tool_name}:{capability_id}")
    } else {
        format!("provider_capability_binding:{tool_name}:{capability_id}:{position}")
    }
}
