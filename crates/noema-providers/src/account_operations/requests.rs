//! Pathless provider account operation requests.

use std::fmt;

use crate::ProviderAuthMethod;

/// Validated provider secret whose debug representation is always redacted.
#[derive(PartialEq, Eq)]
pub(crate) struct ProviderSecret(pub(crate) String);

impl ProviderSecret {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ()> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(());
        }
        Ok(Self(value.to_string()))
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderSecret([REDACTED])")
    }
}

/// Request to create one secret-backed provider account.
#[derive(PartialEq, Eq)]
pub struct CreateSecretProviderAccountRequest {
    /// Provider kind selected from the account catalog.
    pub provider_kind: String,
    /// Optional user-facing account label.
    pub display_name: Option<String>,
    pub(crate) secret: ProviderSecret,
}

impl CreateSecretProviderAccountRequest {
    /// Build a request after validating the write-only secret.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied secret is blank.
    pub fn new(
        provider_kind: impl Into<String>,
        display_name: Option<String>,
        secret: impl Into<String>,
    ) -> Result<Self, crate::ProviderAccountOperationError> {
        Ok(Self {
            provider_kind: provider_kind.into(),
            display_name,
            secret: ProviderSecret::new(secret)
                .map_err(|()| crate::ProviderAccountOperationError::InvalidSecret)?,
        })
    }
}

impl fmt::Debug for CreateSecretProviderAccountRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateSecretProviderAccountRequest")
            .field("provider_kind", &self.provider_kind)
            .field("display_name", &self.display_name)
            .field("secret", &self.secret)
            .finish()
    }
}

/// Request to replace one provider account's write-only secret.
#[derive(PartialEq, Eq)]
pub struct SaveProviderAccountSecretRequest {
    /// Stable provider account id.
    pub provider_account_id: String,
    pub(crate) secret: ProviderSecret,
}

impl SaveProviderAccountSecretRequest {
    /// Build a request after validating the write-only secret.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied secret is blank.
    pub fn new(
        provider_account_id: impl Into<String>,
        secret: impl Into<String>,
    ) -> Result<Self, crate::ProviderAccountOperationError> {
        Ok(Self {
            provider_account_id: provider_account_id.into(),
            secret: ProviderSecret::new(secret)
                .map_err(|()| crate::ProviderAccountOperationError::InvalidSecret)?,
        })
    }
}

impl fmt::Debug for SaveProviderAccountSecretRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaveProviderAccountSecretRequest")
            .field("provider_account_id", &self.provider_account_id)
            .field("secret", &self.secret)
            .finish()
    }
}

/// Pathless request to start one provider authentication attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartProviderAuthRequest {
    /// Provider kind expected by the API caller.
    pub provider_kind: String,
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Authentication method expected by the API caller.
    pub method: ProviderAuthMethod,
    /// Exact callback URL owned by the serving shell, when required.
    pub callback_url: Option<String>,
}

/// Validated fields from one provider OAuth callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompleteProviderAuthCallbackRequest {
    /// One-use opaque attempt identifier.
    pub attempt_id: String,
    /// Short-lived authorization code returned by the provider.
    pub code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_requests_reject_blank_secrets() {
        assert_eq!(
            CreateSecretProviderAccountRequest::new("exa", None, " "),
            Err(crate::ProviderAccountOperationError::InvalidSecret),
        );
        assert_eq!(
            SaveProviderAccountSecretRequest::new("provider_account:exa:test", "\n\t",),
            Err(crate::ProviderAccountOperationError::InvalidSecret),
        );
    }

    #[test]
    fn credential_request_debug_output_is_redacted() {
        let create = CreateSecretProviderAccountRequest::new(
            "exa",
            Some("Research".to_string()),
            "create-secret-value",
        )
        .expect("create request");
        let save =
            SaveProviderAccountSecretRequest::new("provider_account:exa:test", "save-secret-value")
                .expect("save request");
        let debug = format!("{create:?} {save:?}");

        assert!(!debug.contains("create-secret-value"));
        assert!(!debug.contains("save-secret-value"));
        assert!(debug.contains("[REDACTED]"));
        assert_eq!(create.secret.0, "create-secret-value");
        assert_eq!(save.secret.0, "save-secret-value");
    }
}
