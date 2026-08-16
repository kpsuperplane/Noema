//! Private in-memory browser sessions and one-shot startup bootstrap.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use tower_sessions::{Session, cookie::Key};

const AUTHENTICATED_KEY: &str = "authenticated";
const SETUP_AUTHORIZED_KEY: &str = "setup_authorized";
const BROWSER_BINDING_KEY: &str = "browser_binding";
const PASSKEY_ID_KEY: &str = "passkey_id";
const RECENT_PASSKEY_AT_KEY: &str = "recent_passkey_at";
const RECENT_PASSKEY_TTL: Duration = Duration::from_secs(5 * 60);

/// Process-local session and startup capability state.
#[derive(Clone)]
pub(crate) struct SessionSecurity {
    capability: Arc<Mutex<Option<String>>>,
    key: Key,
    #[cfg(test)]
    test_bootstrap_authenticates: bool,
}

impl SessionSecurity {
    pub(crate) fn generate() -> Result<Self, ring::error::Unspecified> {
        let mut bytes = [0_u8; 32];
        SystemRandom::new().fill(&mut bytes)?;
        Ok(Self {
            capability: Arc::new(Mutex::new(Some(URL_SAFE_NO_PAD.encode(bytes)))),
            key: Key::generate(),
            #[cfg(test)]
            test_bootstrap_authenticates: false,
        })
    }

    #[cfg(test)]
    pub(super) fn for_tests(capability: &str) -> Self {
        Self {
            capability: Arc::new(Mutex::new(Some(capability.to_string()))),
            key: Key::generate(),
            test_bootstrap_authenticates: true,
        }
    }

    #[cfg(test)]
    pub(super) fn for_setup_tests(capability: &str) -> Self {
        Self {
            capability: Arc::new(Mutex::new(Some(capability.to_string()))),
            key: Key::generate(),
            test_bootstrap_authenticates: false,
        }
    }

    #[cfg(test)]
    pub(super) const fn test_bootstrap_authenticates(&self) -> bool {
        self.test_bootstrap_authenticates
    }

    pub(super) fn key(&self) -> Key {
        self.key.clone()
    }

    pub(crate) fn bootstrap_url(&self, origin: &str) -> Option<String> {
        self.capability.lock().ok().and_then(|capability| {
            capability
                .as_ref()
                .map(|value| format!("{origin}/__noema/bootstrap/{value}"))
        })
    }

    pub(super) fn consume(&self, candidate: &str) -> bool {
        let Ok(mut capability) = self.capability.lock() else {
            return false;
        };
        if capability.as_deref() != Some(candidate) {
            return false;
        }
        capability.take();
        true
    }
}

pub(super) async fn authenticate(
    session: &Session,
    credential_id: &str,
) -> Result<(), tower_sessions::session::Error> {
    session.clear().await;
    session.insert(AUTHENTICATED_KEY, true).await?;
    session.insert(PASSKEY_ID_KEY, credential_id).await?;
    session
        .insert(RECENT_PASSKEY_AT_KEY, unix_timestamp())
        .await?;
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

pub(super) async fn authorize_setup(
    session: &Session,
) -> Result<(), tower_sessions::session::Error> {
    session.clear().await;
    session.insert(SETUP_AUTHORIZED_KEY, true).await?;
    session.cycle_id().await
}

pub(super) async fn is_setup_authorized(session: &Session) -> bool {
    session
        .get::<bool>(SETUP_AUTHORIZED_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
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

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
