//! Stored OAuth application, account, grant, and token authorities.

use crate::{
    AccountIdentityProbe, CredentialField, CredentialInput, CredentialSetup, LuauTransform,
    Oauth2CallbackMode, Oauth2ClientAuthentication, Oauth2CredentialSetup, SemanticDigest,
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
    /// Human label used when stable provider identity is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
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

/// Return the reviewed Google OAuth profile used by Gmail and Calendar definitions.
#[must_use]
pub fn reviewed_google_oauth_profile() -> OauthProfileV1 {
    OauthProfileV1 {
        schema_version: 1,
        profile_id: "google".to_string(),
        display_name: "Google".to_string(),
        authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_endpoint: "https://oauth2.googleapis.com/token".to_string(),
        client_authentication: Oauth2ClientAuthentication::ClientSecretPost,
        setups: vec![
            Oauth2CredentialSetup {
                callback_mode: Oauth2CallbackMode::Loopback,
                setup: CredentialSetup {
                    credential_type: "Desktop app OAuth client".to_string(),
                    setup_url: "https://console.cloud.google.com/apis/credentials".to_string(),
                    instructions: vec![
                        "Create one Desktop app OAuth client.".to_string(),
                        "Download its JSON document.".to_string(),
                    ],
                    input: CredentialInput::Document {
                        media_type: "application/json".to_string(),
                        fields: google_client_fields(false),
                        normalize: LuauTransform::Luau {
                            source: "return function(input) local d = json.decode(input.document) return { client_id = d.installed.client_id, client_secret = d.installed.client_secret } end".to_string(),
                        },
                    },
                },
            },
            Oauth2CredentialSetup {
                callback_mode: Oauth2CallbackMode::Hosted,
                setup: CredentialSetup {
                    credential_type: "Web application OAuth client".to_string(),
                    setup_url: "https://console.cloud.google.com/apis/credentials".to_string(),
                    instructions: vec![
                        "Create one Web application OAuth client.".to_string(),
                        "Add the shown redirect URI.".to_string(),
                        "Download its JSON document.".to_string(),
                    ],
                    input: CredentialInput::Document {
                        media_type: "application/json".to_string(),
                        fields: google_client_fields(true),
                        normalize: LuauTransform::Luau {
                            source: "return function(input) local d = json.decode(input.document) return { client_id = d.web.client_id, client_secret = d.web.client_secret, redirect_uris = json.encode(d.web.redirect_uris) } end".to_string(),
                        },
                    },
                },
            },
        ],
        authorization_parameters: [
            ("access_type".to_string(), "offline".to_string()),
            ("include_granted_scopes".to_string(), "true".to_string()),
        ]
        .into_iter()
        .collect(),
        account_selection_parameters: [("prompt".to_string(), "select_account".to_string())]
            .into_iter()
            .collect(),
        grant_audience: "google-apis".to_string(),
        omitted_scope_policy: OauthScopeResponsePolicy::RequestedScopes,
        preserve_refresh_token_on_expansion: true,
        account_identity: None,
    }
}

/// Return the exact content address of the reviewed Google OAuth profile.
#[must_use]
pub fn reviewed_google_oauth_profile_digest() -> String {
    let value = serde_json::to_value(reviewed_google_oauth_profile())
        .expect("reviewed Google OAuth profile serializes");
    let bytes = crate::digest::canonical_json_bytes(&value)
        .expect("reviewed Google OAuth profile is canonical");
    SemanticDigest::compute(&bytes).to_string()
}

fn google_client_fields(include_redirects: bool) -> Vec<CredentialField> {
    let mut fields = vec![
        CredentialField {
            id: "client_id".to_string(),
            label: "Client ID".to_string(),
        },
        CredentialField {
            id: "client_secret".to_string(),
            label: "Client secret".to_string(),
        },
    ];
    if include_redirects {
        fields.push(CredentialField {
            id: "redirect_uris".to_string(),
            label: "Registered redirect URIs".to_string(),
        });
    }
    fields
}
