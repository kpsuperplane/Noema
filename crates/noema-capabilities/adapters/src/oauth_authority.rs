//! Stored OAuth application, account, grant, and token authorities.

use crate::{
    AccountIdentityProbe, Oauth2CallbackMode, Oauth2ClientAuthentication, Oauth2CredentialSetup,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Meaning of an OAuth token response that omits its `scope` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OauthScopeResponsePolicy {
    /// Omission is invalid because granted scopes cannot be proven.
    RequireScope,
    /// Omission means the exact requested set was granted.
    RequestedScopes,
}

/// Reviewed, content-addressed OAuth service behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OauthProfileV1 {
    /// Exact profile schema. Only version 1 is accepted.
    pub schema_version: u16,
    /// Stable profile family identity.
    pub profile_id: String,
    /// Human-facing profile label.
    pub display_name: String,
    /// Fixed authorization endpoint.
    pub authorization_endpoint: String,
    /// Fixed token endpoint.
    pub token_endpoint: String,
    /// Token endpoint client authentication.
    pub client_authentication: Oauth2ClientAuthentication,
    /// Callback-specific application setup contracts.
    pub setups: Vec<Oauth2CredentialSetup>,
    /// Fixed provider authorization parameters.
    #[serde(default)]
    pub authorization_parameters: BTreeMap<String, String>,
    /// Parameters added only when another account must be selected.
    #[serde(default)]
    pub account_selection_parameters: BTreeMap<String, String>,
    /// Stable token audience shared by compatible API definitions.
    pub grant_audience: String,
    /// Reviewed meaning of a token response without `scope`.
    pub omitted_scope_policy: OauthScopeResponsePolicy,
    /// Whether an expanded grant can retain an omitted refresh token.
    pub preserve_refresh_token_on_expansion: bool,
    /// Optional reviewed operation used to resolve stable account identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_identity: Option<AccountIdentityProbe>,
}

/// Desired lifecycle state for one OAuth application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OauthApplicationStatus {
    /// The application can start authorization attempts.
    Active,
    /// The human suspended new authorization attempts.
    Suspended,
}

/// Public descriptor for one provider OAuth client registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OauthApplicationV1 {
    /// Exact application schema. Only version 1 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex application identity.
    pub application_id: String,
    /// Exact reviewed OAuth profile digest.
    pub profile_digest: String,
    /// Callback registration used by this application.
    pub callback_mode: Oauth2CallbackMode,
    /// Public provider client identifier.
    pub client_id: String,
    /// Optional human-visible provider project label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_label: Option<String>,
    /// Current protected client-secret generation.
    pub credential_generation: String,
    /// Exact application descriptor revision.
    pub revision: u64,
    /// Desired application lifecycle state.
    pub status: OauthApplicationStatus,
}

/// Protected immutable OAuth application credential generation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OauthApplicationCredentialV1 {
    /// Exact credential schema. Only version 1 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex generation identity.
    pub generation_id: String,
    /// Optional confidential-client secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

impl std::fmt::Debug for OauthApplicationCredentialV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OauthApplicationCredentialV1")
            .field("schema_version", &self.schema_version)
            .field("generation_id", &self.generation_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// One stable provider account identity known to Noema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalAccountV1 {
    /// Exact account schema. Only version 1 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex local identity.
    pub account_id: String,
    /// Exact OAuth profile digest.
    pub profile_digest: String,
    /// Stable provider subject when the reviewed identity probe returned one.
    pub provider_subject: String,
    /// Optional human-visible account label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    /// Exact account descriptor revision.
    pub revision: u64,
}

/// Desired authorization state for one account grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationGrantStatus {
    /// Current tokens authorize at least the recorded granted scopes.
    Active,
    /// Browser authorization is required.
    AuthenticationRequired,
    /// The provider or human revoked the grant.
    Revoked,
    /// Stored links or reviewed profile authority are unavailable.
    Blocked,
}

/// Public descriptor for one application and external-account authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationGrantV1 {
    /// Exact grant schema. Only version 1 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex grant identity.
    pub grant_id: String,
    /// OAuth application used for this authorization.
    pub application_id: String,
    /// External account identity when a reviewed probe resolved it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Exact reviewed token audience.
    pub audience: String,
    /// Exact scopes Noema currently wants.
    pub desired_scopes: Vec<String>,
    /// Exact scopes returned or qualified by the reviewed profile.
    pub granted_scopes: Vec<String>,
    /// Revision used by delayed calls and scope expansion.
    pub authority_revision: u64,
    /// Current immutable token generation when authorized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_generation: Option<String>,
    /// Token rotation revision. It is not delayed-call authority.
    pub token_revision: u64,
    /// Current authorization status.
    pub status: AuthorizationGrantStatus,
}

/// Protected immutable OAuth access and refresh token generation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OauthGrantTokenV1 {
    /// Exact token schema. Only version 1 is accepted.
    pub schema_version: u16,
    /// Stable random lower-hex generation identity.
    pub generation_id: String,
    /// Current bearer access token.
    pub access_token: String,
    /// Optional refresh token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Optional Unix expiry timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_epoch_seconds: Option<u64>,
}

impl std::fmt::Debug for OauthGrantTokenV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OauthGrantTokenV1")
            .field("schema_version", &self.schema_version)
            .field("generation_id", &self.generation_id)
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .finish()
    }
}
