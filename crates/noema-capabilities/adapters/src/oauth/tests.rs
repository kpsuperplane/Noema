use super::*;
use crate::{AdapterCompiler, AdapterManifest};
use serde_json::json;
use std::collections::BTreeMap;

fn definition() -> CompiledAdapterDefinition {
    let manifest: AdapterManifest = serde_json::from_value(json!({
        "schema_version": 8,
        "definition_id": "definition:oauth",
        "adapter_id": "oauth",
        "definition_revision": "v1",
        "reviewed": true,
        "origin": "https://api.example.test/",
        "authentication": {
            "kind": "oauth2_authorization_code_pkce",
            "scopes": ["read", "write"],
            "authorization_endpoint": "https://auth.example.test/authorize",
            "token_endpoint": "https://auth.example.test/token",
            "client_authentication": "none",
            "setups": [
                {"callback_mode": "loopback", "setup": {
                    "credential_type": "Desktop app", "setup_url": "https://developers.example.test/oauth/clients/new",
                    "instructions": ["Create a Desktop app client."],
                    "input": {"kind": "document", "media_type": "application/json", "fields": [{"id": "client_id", "label": "Client ID"}],
                        "normalize": {"language": "luau", "source": "return function(input) local d = json.decode(input.document) return { client_id = d.installed.client_id } end"}}
                }},
                {"callback_mode": "hosted", "setup": {
                    "credential_type": "Web application", "setup_url": "https://developers.example.test/oauth/clients/new",
                    "instructions": ["Create a Web application client."],
                    "input": {"kind": "document", "media_type": "application/json", "fields": [{"id": "client_id", "label": "Client ID"}],
                        "normalize": {"language": "luau", "source": "return function(input) local d = json.decode(input.document) return { client_id = d.web.client_id } end"}}
                }}
            ],
            "extra_authorization_parameters": {"prompt": "consent"}
        },
        "operations": [{
            "operation_id": "read",
            "description": "Read the current resource.",
            "method": "GET",
            "path": "/v1/read",
            "behavior": {"readOnly": {"value": true, "source": "model"}, "idempotent": {"value": true, "source": "model"}, "destructive": {"value": false, "source": "model"}, "openWorld": {"value": true, "source": "model"}},
            "retry": "never",
            "pagination": {"kind": "none"},
            "response": {"accepted_content_types": ["application/json"], "transform": {"language": "luau", "source": "return function(response) return nil end"}, "output_schema": {"type": "null"}}
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
    assert!(debug.contains(attempt.attempt_id()));
    assert!(debug.contains("https://auth.example.test/authorize"));
    assert!(debug.contains(query.get("code_challenge").expect("challenge")));
    assert!(!debug.contains(query.get("state").expect("state")));
    assert!(!debug.contains(&attempt.pkce_verifier));
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
    let code_debug = format!("{code:?}");
    assert!(!code_debug.contains("code-marker"));

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

#[test]
fn attempt_registry_is_state_indexed_one_use_and_bounded() {
    let definition = definition();
    let first = start_with(
        &definition,
        authority(&definition),
        Oauth2CallbackMode::Loopback,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        1,
    )
    .expect("first");
    let first_state = extract_state(&first);
    let second = start_with(
        &definition,
        authority(&definition),
        Oauth2CallbackMode::Loopback,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        2,
    )
    .expect("second");
    let second_state = extract_state(&second);
    let mut registry = AdapterOAuthAttemptRegistry::default();
    registry.insert(first, 101).expect("insert first");
    registry
        .insert(second, 101)
        .expect("replace same connection");
    assert!(matches!(
        registry.authority_for_callback(&callback(&format!("code=old&state={first_state}"))),
        Err(AdapterOAuthError::CallbackMismatch)
    ));
    let wrong_target = format!("http://127.0.0.1:43123/wrong?code=new&state={second_state}");
    assert!(matches!(
        registry.reserve_for_callback(&wrong_target, 101, &authority(&definition)),
        Err(AdapterOAuthError::CallbackMismatch)
    ));
    let second_callback = callback(&format!("code=new&state={second_state}"));
    let reservation = registry
        .reserve_for_callback(&second_callback, 101, &authority(&definition))
        .expect("reserve valid callback");
    let replacement_while_completing = start_with(
        &definition,
        authority(&definition),
        Oauth2CallbackMode::Loopback,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        4,
    )
    .expect("replacement while completing");
    assert_eq!(
        registry.insert(replacement_while_completing, 101),
        Err(AdapterOAuthError::Unavailable)
    );
    let reserved_key = reservation.state_key();
    reservation
        .complete(&second_callback, 101, &authority(&definition))
        .expect("complete reserved callback");
    registry.finish(reserved_key);
    assert!(registry.authority_for_callback(&second_callback).is_err());
    let expired = start_with(
        &definition,
        authority(&definition),
        Oauth2CallbackMode::Loopback,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        3,
    )
    .expect("expired");
    let expired_state = extract_state(&expired);
    registry.insert(expired, 101).expect("insert expired");
    let expired_callback = callback(&format!("code=late&state={expired_state}"));
    assert!(matches!(
        registry.reserve_for_callback(&expired_callback, 400, &authority(&definition)),
        Err(AdapterOAuthError::Expired)
    ));
    assert!(registry.authority_for_callback(&expired_callback).is_err());

    for index in 0..MAX_ACTIVE_ATTEMPTS {
        let mut candidate_authority = authority(&definition);
        candidate_authority.connection_id = format!("{index:032x}");
        let candidate = start_with(
            &definition,
            candidate_authority,
            Oauth2CallbackMode::Loopback,
            "http://127.0.0.1:43123/adapter/oauth/callback",
            index as u8,
        )
        .expect("candidate");
        registry.insert(candidate, 101).expect("within capacity");
    }
    let mut overflow_authority = authority(&definition);
    overflow_authority.connection_id = "f".repeat(32);
    let overflow = start_with(
        &definition,
        overflow_authority,
        Oauth2CallbackMode::Loopback,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        200,
    )
    .expect("overflow");
    assert_eq!(
        registry.insert(overflow, 101),
        Err(AdapterOAuthError::Unavailable)
    );
    let mut replacement_authority = authority(&definition);
    replacement_authority.connection_id = format!("{:032x}", 0);
    let replacement = start_with(
        &definition,
        replacement_authority,
        Oauth2CallbackMode::Loopback,
        "http://127.0.0.1:43123/adapter/oauth/callback",
        201,
    )
    .expect("replacement at capacity");
    registry
        .insert(replacement, 101)
        .expect("same-connection replacement does not need spare capacity");
}
