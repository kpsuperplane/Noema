//! Native desktop OAuth authorization through the system browser.

use std::{collections::HashMap, net::IpAddr, str::FromStr, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl,
    TokenResponse as _, TokenUrl, basic::BasicClient,
};
use ring::rand::{SecureRandom as _, SystemRandom};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
};

use crate::desktop_profile::{RemoteMetadata, RemoteProfile};

const CALLBACK_PATH: &str = "/oauth/callback";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_CALLBACK_BYTES: usize = 8 * 1024;

pub(crate) struct PendingConnection {
    origin: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionStage {
    pub(crate) origin: String,
}

impl PendingConnection {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let origin = if raw.starts_with("https://") {
            trusted_origin(raw)?
        } else {
            let parsed = url::Url::parse(raw)
                .map_err(|_| "Enter a valid Noema server address.".to_string())?;
            if parsed.scheme() != "noema"
                || parsed.host_str() != Some("connect")
                || !matches!(parsed.path(), "" | "/")
                || parsed.fragment().is_some()
            {
                return Err("Enter a valid Noema server address.".to_string());
            }
            let values = parsed.query_pairs().collect::<Vec<_>>();
            if values.len() != 1 || values[0].0 != "origin" {
                return Err("The Noema connection link is invalid.".to_string());
            }
            trusted_origin(&values[0].1)?
        };
        Ok(Self { origin })
    }

    pub(crate) fn stage(&self) -> ConnectionStage {
        ConnectionStage {
            origin: self.origin.clone(),
        }
    }

    pub(crate) async fn authorize(self) -> Result<RemoteProfile, String> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| "Noema could not start its private sign-in callback.".to_string())?;
        let port = listener
            .local_addr()
            .map_err(|_| "Noema could not read its private sign-in callback.".to_string())?
            .port();
        let redirect = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
        let client_id = format!("noema-desktop:{}", random_text(24)?);
        let client = BasicClient::new(ClientId::new(client_id.clone()))
            .set_auth_uri(
                AuthUrl::new(format!("{}/oauth/authorize", self.origin))
                    .map_err(|_| "Noema could not prepare the sign-in address.".to_string())?,
            )
            .set_token_uri(
                TokenUrl::new(format!("{}/oauth/token", self.origin))
                    .map_err(|_| "Noema could not prepare the token address.".to_string())?,
            )
            .set_redirect_uri(
                RedirectUrl::new(redirect.clone())
                    .map_err(|_| "Noema could not prepare the callback address.".to_string())?,
            );
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (authorization_url, expected_state) = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(challenge)
            .url();
        open::that(authorization_url.as_str())
            .map_err(|_| "Noema could not open the system browser.".to_string())?;
        let (code, returned_state) =
            tokio::time::timeout(CALLBACK_TIMEOUT, receive_callback(&listener, &redirect))
                .await
                .map_err(|_| "Noema sign-in expired. Try again.".to_string())??;
        if returned_state.secret() != expected_state.secret() {
            return Err("Noema rejected an invalid sign-in response.".to_string());
        }
        let http = oauth2::reqwest::Client::builder()
            .redirect(oauth2::reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "Noema could not prepare the token request.".to_string())?;
        let tokens = client
            .exchange_code(code)
            .set_pkce_verifier(verifier)
            .request_async(&http)
            .await
            .map_err(|_| "The server rejected the desktop authorization.".to_string())?;
        let refresh_token = tokens
            .refresh_token()
            .ok_or_else(|| "The server did not return a refresh credential.".to_string())?
            .secret()
            .to_string();
        Ok(RemoteProfile {
            metadata: RemoteMetadata {
                origin: self.origin,
                client_id,
            },
            refresh_token,
            access_token: Some(tokens.access_token().secret().to_string()),
        })
    }
}

async fn receive_callback(
    listener: &TcpListener,
    redirect: &str,
) -> Result<(AuthorizationCode, CsrfToken), String> {
    let (mut stream, peer) = listener
        .accept()
        .await
        .map_err(|_| "Noema could not receive the sign-in response.".to_string())?;
    if !peer.ip().is_loopback() {
        return Err("Noema rejected a non-local sign-in response.".to_string());
    }
    let result = read_callback(&mut stream, redirect).await;
    let (status, message) = if result.is_ok() {
        ("200 OK", "Authorization complete. Return to Noema.")
    } else {
        (
            "400 Bad Request",
            "Noema rejected this authorization response.",
        )
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{message}",
        message.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    result
}

async fn read_callback(
    stream: &mut TcpStream,
    redirect: &str,
) -> Result<(AuthorizationCode, CsrfToken), String> {
    let mut input = Vec::with_capacity(1024);
    loop {
        let mut chunk = [0_u8; 1024];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| "Noema could not read the sign-in response.".to_string())?;
        if read == 0 || input.len() + read > MAX_CALLBACK_BYTES {
            return Err("Noema received an invalid sign-in response.".to_string());
        }
        input.extend_from_slice(&chunk[..read]);
        if input.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let request = std::str::from_utf8(&input)
        .map_err(|_| "Noema received an invalid sign-in response.".to_string())?;
    let mut fields = request
        .lines()
        .next()
        .ok_or_else(|| "Noema received an invalid sign-in response.".to_string())?
        .split_whitespace();
    if fields.next() != Some("GET") {
        return Err("Noema received an invalid sign-in response.".to_string());
    }
    let target = fields
        .next()
        .ok_or_else(|| "Noema received an invalid sign-in response.".to_string())?;
    if fields.next() != Some("HTTP/1.1") || fields.next().is_some() {
        return Err("Noema received an invalid sign-in response.".to_string());
    }
    let base = url::Url::parse(redirect)
        .map_err(|_| "Noema received an invalid sign-in response.".to_string())?;
    let callback = base
        .join(target)
        .map_err(|_| "Noema received an invalid sign-in response.".to_string())?;
    if callback.origin() != base.origin()
        || callback.path() != CALLBACK_PATH
        || callback.fragment().is_some()
    {
        return Err("Noema received an invalid sign-in response.".to_string());
    }
    let mut values: HashMap<String, Vec<String>> = HashMap::new();
    for (name, value) in callback.query_pairs() {
        values
            .entry(name.into_owned())
            .or_default()
            .push(value.into_owned());
    }
    if values.len() != 2 || values.values().any(|entries| entries.len() != 1) {
        return Err("Noema received an invalid sign-in response.".to_string());
    }
    let code = single(&values, "code")?;
    let state = single(&values, "state")?;
    if !(1..=128).contains(&code.len()) || !(1..=256).contains(&state.len()) {
        return Err("Noema received an invalid sign-in response.".to_string());
    }
    Ok((
        AuthorizationCode::new(code.to_string()),
        CsrfToken::new(state.to_string()),
    ))
}

pub(crate) fn trusted_origin(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw)
        .map_err(|_| "Connection requires a trusted HTTPS server origin.".to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "Connection requires a trusted HTTPS server origin.".to_string())?;
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
        return Err("Connection requires a trusted HTTPS server origin.".to_string());
    }
    Ok(parsed.origin().ascii_serialization())
}

fn single<'a>(values: &'a HashMap<String, Vec<String>>, key: &str) -> Result<&'a str, String> {
    values
        .get(key)
        .and_then(|entries| entries.first())
        .map(String::as_str)
        .ok_or_else(|| "Noema received an incomplete sign-in response.".to_string())
}

fn random_text(bytes: usize) -> Result<String, String> {
    let mut value = vec![0_u8; bytes];
    SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| "Noema could not create a client identity.".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_parser_accepts_https_and_server_links() {
        assert_eq!(
            PendingConnection::parse("https://noema.example:8443")
                .expect("origin")
                .stage()
                .origin,
            "https://noema.example:8443"
        );
        assert_eq!(
            PendingConnection::parse("noema://connect?origin=https%3A%2F%2Fnoema.example%3A8443")
                .expect("connection link")
                .stage()
                .origin,
            "https://noema.example:8443"
        );
    }

    #[test]
    fn connection_parser_rejects_unsafe_origins_and_extra_fields() {
        for invalid in [
            "http://noema.example",
            "https://localhost",
            "https://127.0.0.1",
            "https://noema.example/path",
            "noema://connect?origin=https%3A%2F%2Fnoema.example&extra=value",
        ] {
            assert!(
                PendingConnection::parse(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }
}
