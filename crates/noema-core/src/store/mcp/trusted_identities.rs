use rusqlite::{OptionalExtension, params};

use super::{
    NewTrustedIdentitySelector, TrustedIdentitySelectorRecord,
    rows::trusted_identity_selector_from_row,
};
use crate::{
    normalize_trusted_identity_value,
    store::{NoemaStore, StoreError, sqlite::now_timestamp_sql},
};

impl NoemaStore {
    /// Create one trusted identity selector after normalizing the raw value.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the raw selector value is invalid, or when
    /// the embedded store write/read fails.
    pub async fn create_trusted_identity_selector(
        &self,
        selector: NewTrustedIdentitySelector,
    ) -> Result<TrustedIdentitySelectorRecord, StoreError> {
        let normalized_value =
            normalize_trusted_identity_value(selector.selector_kind, &selector.raw_value)
                .ok_or_else(|| {
                    StoreError::Schema(format!(
                        "invalid trusted identity selector value for {}",
                        selector.selector_kind.as_str()
                    ))
                })?;
        self.with_connection(|conn| {
            conn.execute(
                format!(
                    r#"
                    INSERT INTO trusted_identity_selectors (
                      selector_id, owner_scope_id, selector_kind, normalized_value,
                      effect, issuer_actor_id, revoked_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, {})
                    "#,
                    now_timestamp_sql()
                )
                .as_str(),
                params![
                    selector.selector_id,
                    selector.owner_scope_id,
                    selector.selector_kind.as_str(),
                    normalized_value,
                    selector.effect.as_str(),
                    selector.issuer_actor_id,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_trusted_identity_selector(&selector.selector_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing trusted identity selector after create: {}",
                    selector.selector_id
                ))
            })
    }

    /// Return one trusted identity selector by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn get_trusted_identity_selector(
        &self,
        selector_id: &str,
    ) -> Result<Option<TrustedIdentitySelectorRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                TRUSTED_IDENTITY_SELECTOR_SELECT_BY_ID,
                params![selector_id],
                trusted_identity_selector_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }

    /// List trusted identity selectors for one owner scope in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// enum is invalid.
    pub async fn list_trusted_identity_selectors(
        &self,
        owner_scope_id: &str,
    ) -> Result<Vec<TrustedIdentitySelectorRecord>, StoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT selector_id, owner_scope_id, selector_kind, normalized_value,
                  effect, issuer_actor_id, revoked_at
                FROM trusted_identity_selectors
                WHERE owner_scope_id = ?1
                ORDER BY selector_kind, normalized_value
                "#,
            )?;
            let rows =
                statement.query_map(params![owner_scope_id], trusted_identity_selector_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
    }
}

const TRUSTED_IDENTITY_SELECTOR_SELECT_BY_ID: &str = r#"
SELECT selector_id, owner_scope_id, selector_kind, normalized_value,
  effect, issuer_actor_id, revoked_at
FROM trusted_identity_selectors
WHERE selector_id = ?1
LIMIT 1
"#;
