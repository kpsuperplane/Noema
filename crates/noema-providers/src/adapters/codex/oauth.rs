//! Codex OAuth token storage and device-code login.

mod claims;
mod client;
mod device_auth;
mod token_store;

pub(crate) use client::CodexOAuthClient;
pub(crate) use token_store::CodexTokenStore;

pub(crate) use claims::chatgpt_account_id_from_access_token;
pub(crate) use device_auth::{
    CodexDeviceAuthOutcome, CodexDeviceAuthSession, begin_codex_device_auth,
};

#[cfg(test)]
use claims::{now_unix_seconds, token_needs_refresh};
#[cfg(test)]
use client::{DeviceAuthorizationResponse, DeviceCodeResponse, TokenResponse};

#[cfg(test)]
mod tests;
