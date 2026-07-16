use serde_json::json;

use super::{ServiceFixture, exa_account};
use crate::{ProviderAccountOperationError, ProviderAccountOperations, ProviderAccountStatus};

#[tokio::test]
async fn provider_reported_auth_failure_marks_the_exact_credential_revision_unauthenticated() {
    let mut account = exa_account("team", false);
    account.metadata = json!({
        "secretConfigured": true,
        "credentialRevision": 7,
        "base_url": "https://example.test",
    });
    let fixture = ServiceFixture::with_account(account.clone());

    let updated = fixture
        .service
        .record_auth_failure(&account.provider_account_id, 7)
        .await
        .expect("record auth failure");

    assert_eq!(updated.status, ProviderAccountStatus::Unauthenticated);
    assert_eq!(updated.last_error_code.as_deref(), Some("auth_failed"));
    assert_eq!(
        updated.last_error_message.as_deref(),
        Some("Provider rejected the configured credentials")
    );
    assert_eq!(updated.metadata, account.metadata);
}

#[tokio::test]
async fn stale_provider_auth_failure_cannot_clobber_replacement_credentials() {
    let mut account = exa_account("team", false);
    account.metadata["credentialRevision"] = json!(8);
    let fixture = ServiceFixture::with_account(account.clone());

    let error = fixture
        .service
        .record_auth_failure(&account.provider_account_id, 7)
        .await
        .expect_err("stale auth failure");

    assert_eq!(error, ProviderAccountOperationError::Conflict);
    let durable = fixture
        .persistence
        .account(&account.provider_account_id)
        .expect("durable account");
    assert_eq!(durable.status, ProviderAccountStatus::Authenticated);
    assert_eq!(durable.metadata["credentialRevision"], json!(8));
}
