use super::*;
use crate::{AdapterCompiler, AdapterManifestV1};
use serde_json::json;
use std::collections::BTreeMap;

fn definition() -> CompiledAdapterDefinition {
    let manifest: AdapterManifestV1 = serde_json::from_value(json!({
        "schema_version": 1,
        "definition_id": "definition:oauth",
        "adapter_id": "oauth",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "mode": "oauth2_authorization_code_pkce",
            "scopes": ["read", "write"],
            "oauth2": {
                "authorization_endpoint": "https://auth.example.test/authorize",
                "token_endpoint": "https://auth.example.test/token",
                "client_authentication": "none",
                "callback_modes": ["loopback", "hosted"],
                "extra_authorization_parameters": {"prompt": "consent"}
            }
        },
        "provider_data_policy": {"retention_allowed": false, "deletion_supported": true},
        "quota": {"cost_class": "free"},
        "operations": [{
            "operation_id": "read",
            "method": "GET",
            "path": "/v1/read",
            "effect": "read_only",
            "admission": "direct",
            "result": {
                "classification": "private",
                "model_route": "local_only",
                "model_payload": "full",
                "provider_retention": "deny",
                "persistence": "omit"
            },
            "retry": "never",
            "pagination": {"kind": "none"}
        }]
    }))
    .expect("manifest");
    AdapterCompiler::compile(&manifest).expect("compile")
}

fn authority(definition: &CompiledAdapterDefinition) -> AdapterOAuthAuthorityV1 {
    AdapterOAuthAuthorityV1 {
        human_id: "human:local".to_string(),
        connection_id: "a".repeat(32),
        account_id: Some("account:synthetic".to_string()),
        account_kind: "personal".to_string(),
        semantic_digest: definition.semantic_digest.to_string(),
        connection_revision: 1,
        credential_revision: 0,
        grant_revision: 1,
        policy_revision: 1,
    }
}

fn start(definition: &CompiledAdapterDefinition) -> AdapterOAuthAttempt {
    start_with(
        definition,
        authority(definition),
        Oauth2CallbackMode::Loopback,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        7,
    )
    .expect("start")
}

fn start_with(
    definition: &CompiledAdapterDefinition,
    authority: AdapterOAuthAuthorityV1,
    callback_mode: Oauth2CallbackMode,
    redirect_uri: &str,
    random_byte: u8,
) -> Result<AdapterOAuthAttempt, AdapterOAuthError> {
    AdapterOAuthAttempt::start_with_random(
        definition,
        "client-synthetic",
        authority,
        callback_mode,
        redirect_uri,
        100,
        300,
        &[random_byte; RANDOM_BYTES],
    )
}

fn extract_state(attempt: &AdapterOAuthAttempt) -> String {
    Url::parse(attempt.authorization_url())
        .expect("authorization")
        .query_pairs()
        .find(|(name, _)| name == "state")
        .map(|(_, value)| value.into_owned())
        .expect("state")
}

fn callback(query: &str) -> String {
    format!("http://127.0.0.1:43123/adapter/oauth/callback?{query}")
}

fn assert_error(
    attempt: AdapterOAuthAttempt,
    definition: &CompiledAdapterDefinition,
    query: &str,
    now_epoch_seconds: u64,
    expected: AdapterOAuthError,
) {
    assert!(matches!(
        attempt.complete(&callback(query), now_epoch_seconds, &authority(definition)),
        Err(error) if error == expected
    ));
}

#[test]
fn authorization_url_is_pkce_bound_and_debug_redacts_transient_values() {
    let definition = definition();
    let attempt = start(&definition);
    let url = Url::parse(attempt.authorization_url()).expect("url");
    let query = url
        .query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("client-synthetic")
    );
    assert_eq!(query.get("scope").map(String::as_str), Some("read write"));
    assert_eq!(query.get("prompt").map(String::as_str), Some("consent"));
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert!(query.get("state").is_some_and(|value| value.len() >= 43));
    assert!(
        query
            .get("code_challenge")
            .is_some_and(|value| value.len() >= 43)
    );
    let debug = format!("{attempt:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains(attempt.attempt_id()));
    assert!(!debug.contains(query.get("state").expect("state")));
    assert!(!debug.contains(query.get("code_challenge").expect("challenge")));
}

#[test]
fn callback_requires_exact_state_and_revision_binding() {
    let definition = definition();
    let attempt = start(&definition);
    let state = extract_state(&attempt);
    let mut current = authority(&definition);
    current.policy_revision = 2;
    assert!(matches!(
        attempt.complete(
            &callback(&format!("code=code&state={state}")),
            101,
            &current,
        ),
        Err(AdapterOAuthError::Superseded)
    ));
}

#[test]
fn callback_success_rejects_duplicates_denials_and_expiry() {
    let definition = definition();
    let attempt = start(&definition);
    let state = extract_state(&attempt);
    let code = attempt
        .complete(
            &callback(&format!("code=code-marker&state={state}")),
            101,
            &authority(&definition),
        )
        .expect("callback");
    assert_eq!(code.attempt_id(), "07070707070707070707070707070707");
    let code_debug = format!("{code:?}");
    assert!(!code_debug.contains("code-marker"));
    assert!(!code_debug.contains(code.attempt_id()));

    let duplicate = start(&definition);
    let duplicate_state = extract_state(&duplicate);
    assert_error(
        duplicate,
        &definition,
        &format!("code=a&code=b&state={duplicate_state}"),
        101,
        AdapterOAuthError::InvalidInput,
    );

    let duplicate_unknown = start(&definition);
    let duplicate_unknown_state = extract_state(&duplicate_unknown);
    assert_error(
        duplicate_unknown,
        &definition,
        &format!("code=a&foo=x&foo=y&state={duplicate_unknown_state}"),
        101,
        AdapterOAuthError::InvalidInput,
    );

    let userinfo = start(&definition);
    let userinfo_state = extract_state(&userinfo);
    assert!(matches!(
            userinfo.complete(
                &format!(
                    "http://user:password@127.0.0.1:43123/adapter/oauth/callback?code=a&state={userinfo_state}"
                ),
                101,
                &authority(&definition),
            ),
            Err(AdapterOAuthError::CallbackMismatch)
        ));

    let denied = start(&definition);
    let denied_state = extract_state(&denied);
    assert_error(
        denied,
        &definition,
        &format!("error=access_denied&state={denied_state}"),
        101,
        AdapterOAuthError::ProviderDenied,
    );

    let expired = start(&definition);
    let expired_state = extract_state(&expired);
    assert_error(
        expired,
        &definition,
        &format!("code=a&state={expired_state}"),
        400,
        AdapterOAuthError::Expired,
    );
}

#[test]
fn callback_modes_and_unreviewed_definitions_fail_closed() {
    let mut definition = definition();
    let authority = authority(&definition);
    let error = start_with(
        &definition,
        authority.clone(),
        Oauth2CallbackMode::Hosted,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        1,
    )
    .expect_err("hosted callback cannot be loopback HTTP");
    assert_eq!(error, AdapterOAuthError::InvalidInput);
    let error = start_with(
        &definition,
        authority.clone(),
        Oauth2CallbackMode::Hosted,
        "https://setup.example.test:0/adapter/oauth/callback",
        1,
    )
    .expect_err("port zero is not a callback authority");
    assert_eq!(error, AdapterOAuthError::InvalidInput);
    definition.reviewed = false;
    assert!(matches!(
        start_with(
            &definition,
            authority,
            Oauth2CallbackMode::Loopback,
            "http://127.0.0.1:43123/adapter/oauth/callback",
            1,
        ),
        Err(AdapterOAuthError::Unsupported)
    ));
}
