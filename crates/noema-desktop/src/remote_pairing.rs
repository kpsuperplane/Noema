//! Strict remote pairing input and completion.

use std::{collections::HashMap, net::IpAddr, str::FromStr, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};

use crate::desktop_profile::{RemoteMetadata, RemoteProfile};

const MAX_RESPONSE_BYTES: usize = 16 * 1024;

pub(crate) struct PendingPairing {
    origin: String,
    pairing_id: String,
    secret: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingStage {
    pub(crate) origin: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompleteRequest<'a> {
    pairing_id: &'a str,
    secret: &'a str,
    display_name: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompleteResponse {
    client_id: String,
    token: String,
}

impl PendingPairing {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(raw.trim())
            .map_err(|_| "Paste a valid Noema pairing link.".to_string())?;
        if parsed.scheme() != "noema"
            || parsed.host_str() != Some("pair")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
            || !matches!(parsed.path(), "" | "/")
            || parsed.fragment().is_some()
        {
            return Err("Paste a valid Noema pairing link.".to_string());
        }
        let mut values: HashMap<String, Vec<String>> = HashMap::new();
        for (name, value) in parsed.query_pairs() {
            values
                .entry(name.into_owned())
                .or_default()
                .push(value.into_owned());
        }
        if values.len() != 3 || values.values().any(|entries| entries.len() != 1) {
            return Err("The pairing link has invalid fields.".to_string());
        }
        let origin = trusted_origin(single(&values, "origin")?)?;
        let pairing_id = single(&values, "pairingId")?.to_string();
        let secret = single(&values, "secret")?.to_string();
        if !(1..=128).contains(&pairing_id.len()) || !is_base64url(&pairing_id) {
            return Err("The pairing link has an invalid pairing id.".to_string());
        }
        if secret.len() != 43
            || !is_base64url(&secret)
            || URL_SAFE_NO_PAD
                .decode(&secret)
                .map_or(true, |bytes| bytes.len() != 32)
        {
            return Err("The pairing link has an invalid pairing secret.".to_string());
        }
        Ok(Self {
            origin,
            pairing_id,
            secret,
        })
    }

    pub(crate) fn stage(&self) -> PairingStage {
        PairingStage {
            origin: self.origin.clone(),
        }
    }

    pub(crate) async fn complete(self, display_name: &str) -> Result<RemoteProfile, String> {
        let display_name = display_name.trim();
        if display_name.is_empty() || display_name.len() > 128 {
            return Err("Enter a client name between 1 and 128 bytes.".to_string());
        }
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "Noema could not prepare the remote pairing request.".to_string())?;
        let endpoint = format!("{}/auth/client/pairing/complete", self.origin);
        let response = client
            .post(endpoint)
            .json(&CompleteRequest {
                pairing_id: &self.pairing_id,
                secret: &self.secret,
                display_name,
            })
            .send()
            .await
            .map_err(|_| "Noema could not reach that server.".to_string())?;
        if !response.status().is_success() {
            return Err("The server rejected this pairing link.".to_string());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| "Noema could not read the pairing response.".to_string())?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("The server returned an invalid client credential.".to_string());
        }
        let response: CompleteResponse = serde_json::from_slice(&bytes)
            .map_err(|_| "The server returned an invalid client credential.".to_string())?;
        validate_complete_response(&response)?;
        Ok(RemoteProfile {
            metadata: RemoteMetadata {
                origin: self.origin,
                client_id: response.client_id,
            },
            token: response.token,
        })
    }
}

pub(crate) fn trusted_origin(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw)
        .map_err(|_| "Pairing requires a trusted HTTPS server origin.".to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "Pairing requires a trusted HTTPS server origin.".to_string())?;
    let local = host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
        || IpAddr::from_str(host).is_ok();
    if parsed.scheme() != "https"
        || local
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("Pairing requires a trusted HTTPS server origin.".to_string());
    }
    Ok(parsed.origin().ascii_serialization())
}

fn single<'a>(values: &'a HashMap<String, Vec<String>>, key: &str) -> Result<&'a str, String> {
    values
        .get(key)
        .and_then(|entries| entries.first())
        .map(String::as_str)
        .ok_or_else(|| "The pairing link is missing a required field.".to_string())
}

fn is_base64url(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_complete_response(response: &CompleteResponse) -> Result<(), String> {
    let valid_id =
        (1..=128).contains(&response.client_id.len()) && is_base64url(&response.client_id);
    let Some(encoded) = response
        .token
        .strip_prefix(&format!("{}.", response.client_id))
    else {
        return Err("The server returned an invalid client credential.".to_string());
    };
    let valid_token = encoded.len() == 43
        && is_base64url(encoded)
        && URL_SAFE_NO_PAD
            .decode(encoded)
            .is_ok_and(|bytes| bytes.len() == 32);
    if !valid_id || !valid_token {
        return Err("The server returned an invalid client credential.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairing_uri(origin: &str) -> String {
        let id = URL_SAFE_NO_PAD.encode([4_u8; 32]);
        let secret = URL_SAFE_NO_PAD.encode([7_u8; 32]);
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("origin", origin);
        query.append_pair("pairingId", &id);
        query.append_pair("secret", &secret);
        format!("noema://pair?{}", query.finish())
    }

    #[test]
    fn pairing_parser_accepts_the_server_uri() {
        let pending = PendingPairing::parse(&pairing_uri("https://noema.example:8443"))
            .expect("valid pairing");
        assert_eq!(pending.stage().origin, "https://noema.example:8443");
    }

    #[test]
    fn pairing_parser_rejects_unsafe_variants() {
        for invalid in [
            pairing_uri("http://noema.example"),
            pairing_uri("https://localhost"),
            pairing_uri("https://127.0.0.1"),
            pairing_uri("https://noema.example/path"),
            format!("{}&secret=duplicate", pairing_uri("https://noema.example")),
            format!("{}&extra=value", pairing_uri("https://noema.example")),
            "noema://pair?origin=https%3A%2F%2Fnoema.example&pairingId=x&secret=x#fragment"
                .to_string(),
        ] {
            assert!(
                PendingPairing::parse(&invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn pairing_response_requires_one_matching_bearer_credential() {
        let client_id = URL_SAFE_NO_PAD.encode([2_u8; 32]);
        let secret = URL_SAFE_NO_PAD.encode([3_u8; 32]);
        assert!(
            validate_complete_response(&CompleteResponse {
                client_id: client_id.clone(),
                token: format!("{client_id}.{secret}"),
            })
            .is_ok()
        );
        for token in [
            secret.clone(),
            format!("other.{secret}"),
            format!("{client_id}.short"),
        ] {
            assert!(
                validate_complete_response(&CompleteResponse {
                    client_id: client_id.clone(),
                    token,
                })
                .is_err()
            );
        }
    }
}
