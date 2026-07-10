//! Private in-memory browser sessions and one-shot startup bootstrap.

use std::sync::{Arc, Mutex};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::rand::{SecureRandom, SystemRandom};
use tower_sessions::{Session, cookie::Key};

const AUTHENTICATED_KEY: &str = "authenticated";

/// Process-local session and startup capability state.
#[derive(Clone)]
pub(crate) struct SessionSecurity {
    capability: Arc<Mutex<Option<String>>>,
    key: Key,
}

impl SessionSecurity {
    pub(crate) fn generate() -> Result<Self, ring::error::Unspecified> {
        let mut bytes = [0_u8; 32];
        SystemRandom::new().fill(&mut bytes)?;
        Ok(Self {
            capability: Arc::new(Mutex::new(Some(URL_SAFE_NO_PAD.encode(bytes)))),
            key: Key::generate(),
        })
    }

    #[cfg(test)]
    pub(super) fn for_tests(capability: &str) -> Self {
        Self {
            capability: Arc::new(Mutex::new(Some(capability.to_string()))),
            key: Key::generate(),
        }
    }

    pub(super) fn key(&self) -> Key {
        self.key.clone()
    }

    pub(crate) fn bootstrap_url(&self, authority: &str) -> Option<String> {
        self.capability.lock().ok().and_then(|capability| {
            capability
                .as_ref()
                .map(|value| format!("http://{authority}/__noema/bootstrap/{value}"))
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

pub(super) async fn authenticate(session: &Session) -> Result<(), tower_sessions::session::Error> {
    session.insert(AUTHENTICATED_KEY, true).await
}

pub(super) async fn is_authenticated(session: &Session) -> bool {
    session
        .get::<bool>(AUTHENTICATED_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

pub(super) async fn request_principal(
    session: &Session,
) -> Option<crate::graphql::RequestPrincipal> {
    is_authenticated(session)
        .await
        .then(crate::graphql::RequestPrincipal::local)
}
