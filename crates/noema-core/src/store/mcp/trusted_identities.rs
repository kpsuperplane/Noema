use super::{
    NewTrustedIdentitySelector, TrustedIdentitySelectorRecord, mcp_record_fragment,
    rows::{TrustedIdentitySelectorRow, trusted_identity_selector_from_row},
};
use crate::{
    normalize_trusted_identity_value,
    store::{NoemaStore, StoreError},
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
        self.db
            .query(
                r#"
                CREATE type::record('trusted_identity_selectors', $record_id) SET
                  selector_id = $selector_id,
                  owner_scope_id = $owner_scope_id,
                  selector_kind = $selector_kind,
                  normalized_value = $normalized_value,
                  effect = $effect,
                  issuer_actor_id = $issuer_actor_id,
                  revoked_at = NONE,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", mcp_record_fragment(&selector.selector_id)))
            .bind(("selector_id", selector.selector_id.clone()))
            .bind(("owner_scope_id", selector.owner_scope_id))
            .bind(("selector_kind", selector.selector_kind.as_str().to_string()))
            .bind(("normalized_value", normalized_value))
            .bind(("effect", selector.effect.as_str().to_string()))
            .bind(("issuer_actor_id", selector.issuer_actor_id))
            .await?
            .check()?;
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT selector_id, owner_scope_id, selector_kind, normalized_value,
                  effect, issuer_actor_id, revoked_at
                FROM trusted_identity_selectors
                WHERE selector_id = $selector_id
                LIMIT 1;
                "#,
            )
            .bind(("selector_id", selector_id.to_string()))
            .await?;
        let rows: Vec<TrustedIdentitySelectorRow> = response.take(0)?;
        rows.into_iter()
            .next()
            .map(trusted_identity_selector_from_row)
            .transpose()
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
        let mut response = self
            .db
            .query(
                r#"
                SELECT selector_id, owner_scope_id, selector_kind, normalized_value,
                  effect, issuer_actor_id, revoked_at
                FROM trusted_identity_selectors
                WHERE owner_scope_id = $owner_scope_id;
                "#,
            )
            .bind(("owner_scope_id", owner_scope_id.to_string()))
            .await?;
        let rows: Vec<TrustedIdentitySelectorRow> = response.take(0)?;
        let mut selectors: Vec<TrustedIdentitySelectorRecord> = rows
            .into_iter()
            .map(trusted_identity_selector_from_row)
            .collect::<Result<_, _>>()?;
        selectors.sort_by(|left, right| {
            left.selector_kind
                .as_str()
                .cmp(right.selector_kind.as_str())
                .then_with(|| left.normalized_value.cmp(&right.normalized_value))
        });
        Ok(selectors)
    }
}
