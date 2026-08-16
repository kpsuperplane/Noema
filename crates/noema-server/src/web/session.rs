//! Private in-memory browser sessions and bounded recovery setup grants.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use tower_sessions::{Session, cookie::Key};

use super::session_store::BoundedSessionStore;

const AUTHENTICATED_KEY: &str = "authenticated";
const BROWSER_BINDING_KEY: &str = "browser_binding";
const PASSKEY_ID_KEY: &str = "passkey_id";
const RECENT_PASSKEY_AT_KEY: &str = "recent_passkey_at";
pub(super) const NATIVE_OAUTH_RESUME_KEY: &str = "native_oauth_resume";
const RECENT_PASSKEY_TTL: Duration = Duration::from_secs(5 * 60);
const SETUP_TTL: Duration = Duration::from_secs(5 * 60);
const SETUP_REGISTRATION_STARTS: u8 = 8;

/// Process-local browser session security state.
#[derive(Clone)]
pub(crate) struct SessionSecurity {
    key: Key,
    store: BoundedSessionStore,
    setup_grants: Arc<Mutex<HashMap<String, SetupGrant>>>,
}

struct SetupGrant {
    expires_at: Instant,
    remaining_starts: u8,
}

impl SessionSecurity {
    pub(crate) fn generate() -> Result<Self, ring::error::Unspecified> {
        let mut bytes = [0_u8; 32];
        SystemRandom::new().fill(&mut bytes)?;
        Ok(Self {
            key: Key::generate(),
            store: BoundedSessionStore::default(),
            setup_grants: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    #[cfg(test)]
    pub(super) fn for_tests(_unused: &str) -> Self {
        Self {
            key: Key::generate(),
            store: BoundedSessionStore::default(),
            setup_grants: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(super) fn key(&self) -> Key {
        self.key.clone()
    }

    pub(super) fn store(&self) -> BoundedSessionStore {
        self.store.clone()
    }

    pub(crate) fn subscribe_revocations(
        &self,
    ) -> tokio::sync::broadcast::Receiver<tower_sessions::session::Id> {
        self.store.subscribe_revocations()
    }

    pub(crate) async fn run_expiry_cleanup(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            self.store.delete_expired();
        }
    }

    pub(super) fn revoke_all(&self) {
        self.store.revoke_all();
    }

    pub(super) fn revoke_passkey(&self, credential_id: &str) {
        self.store.revoke_matching(PASSKEY_ID_KEY, credential_id);
    }

    pub(super) async fn authorize_setup(&self, session: &Session) -> Result<(), ()> {
        session.clear().await;
        let binding = browser_binding(session).await.ok_or(())?;
        session.cycle_id().await.map_err(|_| ())?;
        {
            let mut grants = self.setup_grants.lock().map_err(|_| ())?;
            grants.clear();
            grants.insert(
                binding,
                SetupGrant {
                    expires_at: Instant::now() + SETUP_TTL,
                    remaining_starts: SETUP_REGISTRATION_STARTS,
                },
            );
        }
        Ok(())
    }

    pub(super) async fn is_setup_authorized(&self, session: &Session) -> bool {
        let Some(binding) = browser_binding(session).await else {
            return false;
        };
        let Ok(mut grants) = self.setup_grants.lock() else {
            return false;
        };
        grants.retain(|_, grant| grant.expires_at > Instant::now());
        grants.contains_key(&binding)
    }

    pub(super) async fn claim_setup_registration(&self, session: &Session) -> bool {
        let Some(binding) = browser_binding(session).await else {
            return false;
        };
        let Ok(mut grants) = self.setup_grants.lock() else {
            return false;
        };
        grants.retain(|_, grant| grant.expires_at > Instant::now());
        let Some(grant) = grants.get_mut(&binding) else {
            return false;
        };
        if grant.remaining_starts == 0 {
            return false;
        }
        grant.remaining_starts -= 1;
        true
    }

    pub(super) async fn consume_setup(&self, session: &Session) {
        let Some(binding) = browser_binding(session).await else {
            return;
        };
        if let Ok(mut grants) = self.setup_grants.lock() {
            grants.remove(&binding);
        }
    }
}

pub(super) async fn authenticate(
    session: &Session,
    credential_id: &str,
) -> Result<(), tower_sessions::session::Error> {
    let native_oauth_resume = session.get::<String>(NATIVE_OAUTH_RESUME_KEY).await?;
    session.clear().await;
    session.insert(AUTHENTICATED_KEY, true).await?;
    session.insert(PASSKEY_ID_KEY, credential_id).await?;
    session
        .insert(RECENT_PASSKEY_AT_KEY, unix_timestamp())
        .await?;
    if let Some(query) = native_oauth_resume {
        session.insert(NATIVE_OAUTH_RESUME_KEY, query).await?;
    }
    session.cycle_id().await
}

pub(super) async fn is_authenticated(session: &Session) -> bool {
    session
        .get::<bool>(AUTHENTICATED_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

pub(super) async fn has_recent_passkey(session: &Session) -> bool {
    let Some(authenticated_at) = session
        .get::<u64>(RECENT_PASSKEY_AT_KEY)
        .await
        .ok()
        .flatten()
    else {
        return false;
    };
    unix_timestamp().saturating_sub(authenticated_at) <= RECENT_PASSKEY_TTL.as_secs()
}

pub(super) async fn authenticating_passkey(session: &Session) -> Option<String> {
    session.get::<String>(PASSKEY_ID_KEY).await.ok().flatten()
}

pub(super) async fn browser_binding(session: &Session) -> Option<String> {
    if let Ok(Some(binding)) = session.get::<String>(BROWSER_BINDING_KEY).await {
        return Some(binding);
    }
    let mut bytes = [0_u8; 32];
    SystemRandom::new().fill(&mut bytes).ok()?;
    let binding = URL_SAFE_NO_PAD.encode(bytes);
    session.insert(BROWSER_BINDING_KEY, &binding).await.ok()?;
    Some(binding)
}

pub(super) async fn logout(session: &Session) -> Result<(), tower_sessions::session::Error> {
    session.flush().await
}

pub(super) async fn request_principal(
    session: &Session,
    authentication_required: bool,
) -> Option<noema_api::RequestPrincipal> {
    (!authentication_required || is_authenticated(session).await)
        .then(noema_api::RequestPrincipal::local)
}

pub(super) fn session_hash(session: &Session) -> Option<[u8; 32]> {
    session.id().map(session_id_hash)
}

pub(super) async fn ensure_session_hash(session: &Session) -> Option<[u8; 32]> {
    if session.id().is_none() {
        browser_binding(session).await?;
        session.save().await.ok()?;
    }
    session_hash(session)
}

pub(crate) fn session_id_hash(session_id: tower_sessions::session::Id) -> [u8; 32] {
    let session_id = session_id.to_string();
    let candidate = digest::digest(&digest::SHA256, session_id.as_bytes());
    candidate
        .as_ref()
        .try_into()
        .expect("SHA-256 always returns 32 bytes")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
