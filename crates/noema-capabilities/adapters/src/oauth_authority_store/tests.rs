use super::*;
use crate::{
    CredentialField, CredentialInput, CredentialSetup, Oauth2CallbackMode,
    Oauth2ClientAuthentication, Oauth2CredentialSetup, OauthApplicationStatus,
    OauthScopeResponsePolicy, write_new_file,
};
use tempfile::TempDir;

fn store() -> (TempDir, OauthAuthorityStore) {
    let temporary = TempDir::new().expect("temporary home");
    let paths = NoemaPaths::from_noema_home(temporary.path()).expect("paths");
    (temporary, OauthAuthorityStore::new(paths))
}

fn profile() -> OauthProfileV1 {
    OauthProfileV1 {
        schema_version: 1,
        profile_id: "google".into(),
        display_name: "Google".into(),
        authorization_endpoint: "https://accounts.google.com/o/oauth2/v2/auth".into(),
        token_endpoint: "https://oauth2.googleapis.com/token".into(),
        client_authentication: Oauth2ClientAuthentication::None,
        setups: vec![Oauth2CredentialSetup {
            callback_mode: Oauth2CallbackMode::Hosted,
            setup: CredentialSetup {
                credential_type: "OAuth client document".into(),
                setup_url: "https://console.cloud.google.com/apis/credentials".into(),
                instructions: vec!["Create one Web OAuth client.".into()],
                input: CredentialInput::Fields {
                    fields: vec![CredentialField {
                        id: "client_id".into(),
                        label: "Client ID".into(),
                    }],
                },
            },
        }],
        authorization_parameters: Default::default(),
        account_selection_parameters: [("prompt".into(), "select_account".into())]
            .into_iter()
            .collect(),
        grant_audience: "google-apis".into(),
        omitted_scope_policy: OauthScopeResponsePolicy::RequireScope,
        preserve_refresh_token_on_expansion: true,
    }
}

fn application(profile_digest: &str) -> (OauthApplicationV1, OauthApplicationCredentialV1) {
    let generation_id = "b".repeat(32);
    (
        OauthApplicationV1 {
            schema_version: 1,
            application_id: "a".repeat(32),
            profile_digest: profile_digest.into(),
            callback_mode: Oauth2CallbackMode::Hosted,
            client_id: "public-client-id".into(),
            project_label: Some("Personal APIs".into()),
            credential_generation: generation_id.clone(),
            revision: 1,
            status: OauthApplicationStatus::Active,
        },
        OauthApplicationCredentialV1 {
            schema_version: 1,
            generation_id,
            client_secret: None,
        },
    )
}

fn account(id: char, profile_digest: &str) -> ExternalAccountV1 {
    ExternalAccountV1 {
        schema_version: 1,
        account_id: id.to_string().repeat(32),
        profile_digest: profile_digest.into(),
        provider_subject: format!("subject-{id}"),
        account_label: Some(format!("account-{id}@example.com")),
        revision: 1,
    }
}

fn grant(
    id: char,
    application_id: &str,
    account_id: &str,
    generation: char,
) -> (AuthorizationGrantV1, OauthGrantTokenV1) {
    let generation_id = generation.to_string().repeat(32);
    (
        AuthorizationGrantV1 {
            schema_version: 1,
            grant_id: id.to_string().repeat(32),
            application_id: application_id.into(),
            account_id: Some(account_id.into()),
            audience: "google-apis".into(),
            desired_scopes: vec!["gmail.readonly".into()],
            granted_scopes: vec!["gmail.readonly".into()],
            authority_revision: 1,
            token_generation: Some(generation_id.clone()),
            token_revision: 1,
            status: AuthorizationGrantStatus::Active,
        },
        OauthGrantTokenV1 {
            schema_version: 1,
            generation_id,
            access_token: format!("access-{id}"),
            refresh_token: Some(format!("refresh-{id}")),
            expires_at_epoch_seconds: Some(2_000_000_000),
        },
    )
}

#[test]
fn one_application_keeps_two_accounts_and_grants_separate() {
    let (_temporary, store) = store();
    let profile = store.install_profile(&profile()).expect("profile");
    assert_eq!(
        store
            .install_profile(&profile.profile)
            .expect("same profile"),
        profile
    );
    let (application, credential) = application(&profile.profile_digest);
    store
        .install_application(&application, &credential)
        .expect("application");
    store
        .install_application(&application, &credential)
        .expect("same application");
    for (account_id, grant_id, token_id) in [('c', 'e', '1'), ('d', 'f', '2')] {
        let account = account(account_id, &profile.profile_digest);
        store.install_account(&account).expect("account");
        let (grant, token) = grant(
            grant_id,
            &application.application_id,
            &account.account_id,
            token_id,
        );
        let debug = format!("{token:?}");
        assert!(!debug.contains(&token.access_token));
        assert!(!debug.contains(token.refresh_token.as_deref().expect("refresh token")));
        store.install_grant(&grant, Some(&token)).expect("grant");
    }
    let snapshot = store.snapshot().expect("snapshot");
    assert_eq!(snapshot.applications.len(), 1);
    assert_eq!(snapshot.accounts.len(), 2);
    assert_eq!(snapshot.grants.len(), 2);
}

#[test]
fn failed_expansion_preserves_access_and_refresh_keeps_authority_revision() {
    let (_temporary, store) = store();
    let profile = store.install_profile(&profile()).expect("profile");
    let (application, credential) = application(&profile.profile_digest);
    store
        .install_application(&application, &credential)
        .expect("application");
    let account = account('c', &profile.profile_digest);
    store.install_account(&account).expect("account");
    let (grant, token) = grant('e', &application.application_id, &account.account_id, '1');
    store.install_grant(&grant, Some(&token)).expect("grant");

    let mut invalid_expansion = grant.clone();
    invalid_expansion.desired_scopes.push("gmail.send".into());
    invalid_expansion.authority_revision += 1;
    invalid_expansion.token_revision += 1;
    invalid_expansion.token_generation = Some("2".repeat(32));
    let mut invalid_token = token.clone();
    invalid_token.generation_id = "2".repeat(32);
    invalid_token.access_token = "new-access".into();
    assert!(
        store
            .promote_grant(&invalid_expansion, &invalid_expansion, &invalid_token)
            .is_err()
    );
    assert_eq!(
        store
            .load_grant_authority(&grant.grant_id)
            .expect("unchanged")
            .0,
        grant
    );

    let mut refreshed = grant.clone();
    refreshed.token_generation = Some("3".repeat(32));
    refreshed.token_revision += 1;
    let mut refreshed_token = token;
    refreshed_token.generation_id = "3".repeat(32);
    refreshed_token.access_token = "refreshed-access".into();
    let refreshed = store
        .refresh_grant(&grant, &refreshed, &refreshed_token)
        .expect("refresh");
    assert_eq!(refreshed.authority_revision, grant.authority_revision);
}

#[test]
fn unsafe_or_extra_entries_block_the_complete_snapshot() {
    let (temporary, store) = store();
    let profile = store.install_profile(&profile()).expect("profile");
    let profile_dir = temporary
        .path()
        .join("adapters/oauth-profiles")
        .join(profile.profile_digest);
    write_new_file(&profile_dir.join("unexpected.json"), b"{}").expect("extra file");
    assert!(matches!(
        store.snapshot(),
        Err(OauthAuthorityStoreError::Integrity("object_entries"))
    ));
}
