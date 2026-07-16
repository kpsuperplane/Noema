//! Provider model/profile catalog refresh helpers.

use std::time::Duration;

use noema_home::NoemaPaths;
use reqwest::StatusCode;
use serde_json::Value;

use crate::adapters::{
    account_service::ProviderCredentialAccessHandle,
    codex::oauth::{CodexOAuthClient, CodexTokenStore},
    reqwest_transport_error,
    responses::normalize_base_url,
};
use crate::{
    CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS, CodexOAuthConfig, DEFAULT_CODEX_BASE_URL,
    PersistProviderModelCatalogRequest, ProviderAccountRecord, ProviderAccountStatus,
    ProviderError, ProviderModelCatalogPersistence, ProviderModelProfile,
};

mod client_version;
mod parsing;

use client_version::{
    CODEX_CLIENT_VERSION_ENDPOINT, MODEL_CATALOG_TIMEOUT_SECONDS, current_unix_timestamp,
    is_valid_codex_client_version, resolve_codex_client_version, timestamp_is_fresh,
};
use parsing::profile_values_from_model_list;

pub(crate) use client_version::latest_codex_client_version;

#[cfg(test)]
use client_version::{MODEL_CATALOG_TTL_SECONDS, fetch_latest_codex_client_version};

const MODEL_METADATA_VERSION: u64 = 3;

/// Best-effort refresh of provider model profiles for Settings.
///
/// This stores only non-secret display metadata under provider account metadata.
/// Callers should treat errors as advisory and keep rendering existing account
/// state rather than inventing fallback model lists.
///
/// # Errors
///
/// Returns [`ProviderError`] when the provider catalog endpoint, provider
/// credentials, or metadata persistence fails.
pub async fn refresh_provider_model_profiles(
    persistence: &dyn ProviderModelCatalogPersistence,
    paths: &NoemaPaths,
    account: &ProviderAccountRecord,
) -> Result<(), ProviderError> {
    refresh_provider_model_profiles_at_version_endpoint(
        persistence,
        paths,
        account,
        CODEX_CLIENT_VERSION_ENDPOINT,
    )
    .await
}

pub(crate) async fn fetch_provider_model_catalog(
    credentials: &ProviderCredentialAccessHandle,
    account: &ProviderAccountRecord,
) -> Result<Option<CodexModelCatalog>, ProviderError> {
    fetch_provider_model_catalog_at_version_endpoint(
        credentials,
        account,
        CODEX_CLIENT_VERSION_ENDPOINT,
    )
    .await
}

async fn fetch_provider_model_catalog_at_version_endpoint(
    credentials: &ProviderCredentialAccessHandle,
    account: &ProviderAccountRecord,
    version_endpoint: &str,
) -> Result<Option<CodexModelCatalog>, ProviderError> {
    if metadata_profiles_are_current(account) || !should_refresh_model_profiles(account) {
        return Ok(None);
    }

    match account.provider_kind.as_str() {
        "codex" => {
            fetch_codex_model_profiles_with_credentials(credentials, account, version_endpoint)
                .await
                .map(Some)
        }
        _ => Ok(None),
    }
}

async fn refresh_provider_model_profiles_at_version_endpoint(
    persistence: &dyn ProviderModelCatalogPersistence,
    paths: &NoemaPaths,
    account: &ProviderAccountRecord,
    version_endpoint: &str,
) -> Result<(), ProviderError> {
    if metadata_profiles_are_current(account) || !should_refresh_model_profiles(account) {
        return Ok(());
    }

    let catalog = match account.provider_kind.as_str() {
        "codex" => fetch_codex_model_profiles(paths, account, version_endpoint).await?,
        _ => return Ok(()),
    };

    if catalog.profiles.is_empty() {
        return Ok(());
    }

    persist_model_catalog_refresh(persistence, account, catalog)
        .await
        .map(|_| ())
}

pub(crate) async fn persist_model_catalog_refresh(
    persistence: &dyn ProviderModelCatalogPersistence,
    account: &ProviderAccountRecord,
    catalog: CodexModelCatalog,
) -> Result<ProviderAccountRecord, ProviderError> {
    persistence
        .persist_provider_model_catalog(PersistProviderModelCatalogRequest {
            provider_account_id: account.provider_account_id.clone(),
            profiles: catalog.profiles,
            refreshed_at_unix: current_unix_timestamp(),
            source: format!("{}_models_endpoint", account.provider_kind),
            metadata_version: MODEL_METADATA_VERSION,
            client_version: catalog.client_version,
            client_version_refreshed_at_unix: catalog.client_version_refreshed_at_unix,
            resulting_status: ProviderAccountStatus::Authenticated,
        })
        .await
        .map_err(|source| ProviderError::ProviderUnavailable {
            provider: account.provider_kind.clone(),
            message: format!("failed to persist model catalog: {source}"),
        })
}

fn should_refresh_model_profiles(account: &ProviderAccountRecord) -> bool {
    match account.provider_kind.as_str() {
        // Codex OAuth credentials live in Noema's provider account home. A
        // successful catalog read is the authoritative auth check for Settings.
        "codex" => true,
        _ => account.status == ProviderAccountStatus::Authenticated,
    }
}

fn metadata_profiles_are_current(account: &ProviderAccountRecord) -> bool {
    let has_profiles = account
        .metadata
        .get("profiles")
        .and_then(Value::as_array)
        .is_some_and(|profiles| !profiles.is_empty());
    if !has_profiles {
        return false;
    }
    if account.provider_kind != "codex" {
        return true;
    }
    account
        .metadata
        .get("models_metadata_version")
        .and_then(Value::as_u64)
        == Some(MODEL_METADATA_VERSION)
        && account
            .metadata
            .get("models_client_version")
            .and_then(Value::as_str)
            .is_some_and(is_valid_codex_client_version)
        && timestamp_is_fresh(
            account.metadata.get("models_refreshed_at"),
            current_unix_timestamp(),
        )
}

async fn fetch_codex_model_profiles(
    paths: &NoemaPaths,
    account: &ProviderAccountRecord,
    version_endpoint: &str,
) -> Result<CodexModelCatalog, ProviderError> {
    let base_url = account
        .metadata
        .get("base_url")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_CODEX_BASE_URL);
    let base_url = normalize_base_url(base_url.to_string(), "codex models base URL")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(MODEL_CATALOG_TIMEOUT_SECONDS))
        .build()
        .map_err(|source| {
            reqwest_transport_error(
                &account.provider_kind,
                "build_model_catalog_client",
                &source,
            )
        })?;
    let token_store = CodexTokenStore::new(
        paths.provider_account_home(&account.provider_kind, &account.account_key),
    );
    let oauth_client = CodexOAuthClient::new(CodexOAuthConfig::default())?;
    let access_token = token_store
        .access_token(&oauth_client, CODEX_ACCESS_TOKEN_REFRESH_SKEW_SECONDS)
        .await?;
    let client_version =
        resolve_codex_client_version(&client, &account.metadata, version_endpoint).await;
    let models_url = format!("{base_url}/models?client_version={}", client_version.value);

    let value = match fetch_model_list(&client, &models_url, &access_token).await {
        Ok(value) => value,
        Err(ProviderError::AuthenticationFailure { .. }) => {
            let refreshed = token_store.refresh_access_token(&oauth_client).await?;
            fetch_model_list(&client, &models_url, &refreshed).await?
        }
        Err(error) => return Err(error),
    };
    Ok(CodexModelCatalog {
        profiles: profile_values_from_model_list(&value),
        client_version: client_version.value,
        client_version_refreshed_at_unix: client_version.refreshed_at_unix,
    })
}

async fn fetch_codex_model_profiles_with_credentials(
    credentials: &ProviderCredentialAccessHandle,
    account: &ProviderAccountRecord,
    version_endpoint: &str,
) -> Result<CodexModelCatalog, ProviderError> {
    let base_url = account
        .metadata
        .get("base_url")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_CODEX_BASE_URL);
    let base_url = normalize_base_url(base_url.to_string(), "codex models base URL")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(MODEL_CATALOG_TIMEOUT_SECONDS))
        .build()
        .map_err(|source| {
            reqwest_transport_error(
                &account.provider_kind,
                "build_model_catalog_client",
                &source,
            )
        })?;
    let access_token = credentials
        .codex_access_token(&account.provider_account_id)
        .await?
        .into_secret();
    let client_version =
        resolve_codex_client_version(&client, &account.metadata, version_endpoint).await;
    let models_url = format!("{base_url}/models?client_version={}", client_version.value);

    let value = match fetch_model_list(&client, &models_url, &access_token).await {
        Ok(value) => value,
        Err(ProviderError::AuthenticationFailure { .. }) => {
            let refreshed = credentials
                .refresh_codex_access_token(&account.provider_account_id)
                .await?
                .into_secret();
            fetch_model_list(&client, &models_url, &refreshed).await?
        }
        Err(error) => return Err(error),
    };
    Ok(CodexModelCatalog {
        profiles: profile_values_from_model_list(&value),
        client_version: client_version.value,
        client_version_refreshed_at_unix: client_version.refreshed_at_unix,
    })
}

#[derive(Debug)]
pub(crate) struct CodexModelCatalog {
    profiles: Vec<ProviderModelProfile>,
    client_version: String,
    client_version_refreshed_at_unix: Option<u64>,
}

async fn fetch_model_list(
    client: &reqwest::Client,
    models_url: &str,
    bearer_token: &str,
) -> Result<Value, ProviderError> {
    let response = client
        .get(models_url)
        .bearer_auth(bearer_token)
        .send()
        .await
        .map_err(|source| reqwest_transport_error("codex", "fetch_model_catalog", &source))?;
    let status = response.status();
    let request_id = response
        .headers()
        .get("x-request-id")
        .or_else(|| response.headers().get("x-oai-request-id"))
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let text = response.text().await.map_err(|source| {
        reqwest_transport_error("codex", "read_model_catalog_response", &source)
    })?;

    if !status.is_success() {
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(ProviderError::AuthenticationFailure {
                message: "model catalog authentication failed".to_string(),
                request_id,
            });
        }
        return Err(ProviderError::ApiError {
            status: status.as_u16(),
            message: text,
            request_id,
        });
    }

    serde_json::from_str(&text).map_err(|source| ProviderError::MalformedResponse {
        message: format!("failed to parse model catalog JSON: {source}"),
    })
}

#[cfg(test)]
fn now_string() -> String {
    current_unix_timestamp().to_string()
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
