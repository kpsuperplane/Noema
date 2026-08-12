//! Closed validation rules for reusable OAuth authority objects.

use crate::{
    AuthorizationGrantStatus, AuthorizationGrantV1, ExternalAccountV1,
    OauthApplicationCredentialV1, OauthApplicationV1, OauthGrantTokenV1, OauthProfileV1,
    SemanticDigest, oauth_authority_store::OauthAuthorityStoreError,
};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

pub(super) fn validate_profile(profile: &OauthProfileV1) -> Result<(), OauthAuthorityStoreError> {
    if profile.schema_version != 1
        || !valid_text(&profile.profile_id, 128)
        || !valid_text(&profile.display_name, 128)
        || !valid_text(&profile.grant_audience, 256)
        || profile.setups.is_empty()
        || profile.setups.len() > 4
        || !valid_https_url(&profile.authorization_endpoint)
        || !valid_https_url(&profile.token_endpoint)
        || !valid_parameters(&profile.authorization_parameters)
        || !valid_parameters(&profile.account_selection_parameters)
    {
        return Err(OauthAuthorityStoreError::Integrity("profile_shape"));
    }
    let callback_modes = profile
        .setups
        .iter()
        .map(|setup| setup.callback_mode)
        .collect::<BTreeSet<_>>();
    if callback_modes.len() != profile.setups.len() {
        return Err(OauthAuthorityStoreError::Integrity("profile_setup"));
    }
    Ok(())
}

pub(super) fn validate_application(
    application: &OauthApplicationV1,
    credential: &OauthApplicationCredentialV1,
    profile: &OauthProfileV1,
) -> Result<(), OauthAuthorityStoreError> {
    let secret_required = profile.client_authentication != crate::Oauth2ClientAuthentication::None;
    if application.schema_version != 1
        || !valid_hex(&application.application_id, 32)
        || SemanticDigest::parse(application.profile_digest.clone()).is_err()
        || !profile
            .setups
            .iter()
            .any(|setup| setup.callback_mode == application.callback_mode)
        || !valid_text(&application.client_id, 512)
        || !valid_optional_text(application.project_label.as_deref(), 256)
        || application.revision == 0
        || credential.schema_version != 1
        || credential.generation_id != application.credential_generation
        || !valid_hex(&credential.generation_id, 32)
        || secret_required != credential.client_secret.is_some()
        || !valid_optional_text(credential.client_secret.as_deref(), 16 * 1024)
    {
        return Err(OauthAuthorityStoreError::Integrity("application_shape"));
    }
    Ok(())
}

pub(super) fn validate_account(
    account: &ExternalAccountV1,
) -> Result<(), OauthAuthorityStoreError> {
    if account.schema_version != 1
        || !valid_hex(&account.account_id, 32)
        || SemanticDigest::parse(account.profile_digest.clone()).is_err()
        || !valid_text(&account.provider_subject, 1024)
        || !valid_optional_text(account.account_label.as_deref(), 256)
        || account.revision == 0
    {
        return Err(OauthAuthorityStoreError::Integrity("account_shape"));
    }
    Ok(())
}

pub(super) fn validate_grant(
    grant: &AuthorizationGrantV1,
    token: Option<&OauthGrantTokenV1>,
    application: &OauthApplicationV1,
    account: Option<&ExternalAccountV1>,
    profile: &OauthProfileV1,
) -> Result<(), OauthAuthorityStoreError> {
    let token_matches = match (grant.token_generation.as_deref(), token) {
        (Some(generation), Some(token)) => {
            token.schema_version == 1
                && token.generation_id == generation
                && valid_hex(generation, 32)
                && valid_text(&token.access_token, 64 * 1024)
                && valid_optional_text(token.refresh_token.as_deref(), 64 * 1024)
        }
        (None, None) => true,
        _ => false,
    };
    if grant.schema_version != 1
        || !valid_hex(&grant.grant_id, 32)
        || grant.application_id != application.application_id
        || grant.audience != profile.grant_audience
        || !valid_scope_set(&grant.desired_scopes)
        || !valid_scope_set(&grant.granted_scopes)
        || grant.authority_revision == 0
        || grant.token_revision == 0
        || !token_matches
        || account.is_some_and(|account| account.profile_digest != application.profile_digest)
        || grant.account_id.as_deref() != account.map(|account| account.account_id.as_str())
        || (grant.status == AuthorizationGrantStatus::Active && token.is_none())
        || (grant.status != AuthorizationGrantStatus::Active && token.is_some())
    {
        return Err(OauthAuthorityStoreError::Integrity("grant_shape"));
    }
    Ok(())
}

pub(super) fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_scope_set(scopes: &[String]) -> bool {
    !scopes.is_empty()
        && scopes.len() <= 256
        && scopes.iter().all(|scope| valid_text(scope, 1_024))
        && scopes.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_parameters(parameters: &BTreeMap<String, String>) -> bool {
    parameters.len() <= 32
        && parameters
            .iter()
            .all(|(name, value)| valid_text(name, 128) && valid_text(value, 1_024))
}

fn valid_https_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    })
}

fn valid_optional_text(value: Option<&str>, max: usize) -> bool {
    value.is_none_or(|value| valid_text(value, max))
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.trim() == value && !value.contains('\0')
}
