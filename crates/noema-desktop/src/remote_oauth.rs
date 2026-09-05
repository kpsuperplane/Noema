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
        let code = tokio::time::timeout(
            CALLBACK_TIMEOUT,
            receive_callback(&listener, &redirect, &self.origin, &expected_state),
        )
        .await
        .map_err(|_| "Noema sign-in expired. Try again.".to_string())??;
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
    origin: &str,
    expected_state: &CsrfToken,
) -> Result<AuthorizationCode, String> {
    let (mut stream, peer) = listener
        .accept()
        .await
        .map_err(|_| "Noema could not receive the sign-in response.".to_string())?;
    if !peer.ip().is_loopback() {
        return Err("Noema rejected a non-local sign-in response.".to_string());
    }
    let response = read_callback(&mut stream, redirect).await;
    let denied = matches!(&response, Ok(CallbackResponse::Denied(state)) if state.secret() == expected_state.secret());
    let result = match response {
        Ok(CallbackResponse::Authorized(code, state))
            if state.secret() == expected_state.secret() =>
        {
            Ok(code)
        }
        _ => Err("Noema did not complete this sign-in. Try again.".to_string()),
    };
    let (status, title, intro, note) = if denied {
        (
            "200 OK",
            "Connection declined",
            "No new access was granted.",
            "Return to Noema Desktop when you’re ready.",
        )
    } else if result.is_ok() {
        (
            "200 OK",
            "Continue in Noema Desktop",
            "The app will finish connecting.",
            "If the app did not open, return to it now.",
        )
    } else {
        (
            "400 Bad Request",
            "Sign-in did not finish",
            "Return to Noema Desktop to try again.",
            "Start a new connection from the app.",
        )
    };
    // The origin was validated by trusted_origin. No callback values enter this page.
    let message = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer"><title>{title} · Noema</title><link rel="stylesheet" href="{origin}/assets/supporting.css"></head>
<body data-setup="public"><main data-setup="body"><div data-setup="surface"><div data-setup="avatar" aria-hidden="true"><img src="{origin}/assets/noema-avatar.svg" alt=""></div><section data-setup="card" aria-labelledby="title"><header data-setup="header"><h1 id="title" data-setup="title">{title}</h1><p data-setup="note">{intro}</p></header><p data-setup="note">{note}</p></section></div></main></body></html>"#
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\r\n{message}",
        message.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    result
}

enum CallbackResponse {
    Authorized(AuthorizationCode, CsrfToken),
    Denied(CsrfToken),
}

async fn read_callback(stream: &mut TcpStream, redirect: &str) -> Result<CallbackResponse, String> {
    let target = read_callback_target(stream).await?;
    let base = url::Url::parse(redirect)
        .map_err(|_| "Noema received an invalid sign-in response.".to_string())?;
    let callback = base
        .join(&target)
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
    let state = single(&values, "state")?;
    if !(1..=256).contains(&state.len()) {
        return Err("Noema received an invalid sign-in response.".to_string());
    }
    if values
        .get("error")
        .and_then(|values| values.first())
        .map(String::as_str)
        == Some("access_denied")
    {
        return Ok(CallbackResponse::Denied(CsrfToken::new(state.to_string())));
    }
    let code = single(&values, "code")?;
    if !(1..=128).contains(&code.len()) {
        return Err("Noema received an invalid sign-in response.".to_string());
    }
    Ok(CallbackResponse::Authorized(
        AuthorizationCode::new(code.to_string()),
        CsrfToken::new(state.to_string()),
    ))
}

async fn read_callback_target(stream: &mut TcpStream) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|_| "Noema received an invalid sign-in response.".to_string())?;
        if read == 0 || bytes.len() + read > MAX_CALLBACK_BYTES {
            return Err("Noema received an invalid sign-in response.".to_string());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let text = std::str::from_utf8(&bytes[..end])
                .map_err(|_| "Noema received an invalid sign-in response.".to_string())?;
            let mut fields = text.lines().next().unwrap_or_default().split_whitespace();
            let method = fields.next();
            let target = fields.next();
            if method != Some("GET") || fields.next() != Some("HTTP/1.1") || fields.next().is_some()
            {
                return Err("Noema received an invalid sign-in response.".to_string());
            }
            return target
                .map(str::to_string)
                .ok_or_else(|| "Noema received an invalid sign-in response.".to_string());
        }
    }
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

    #[tokio::test]
    async fn callback_distinguishes_verified_denial_from_handoff_and_invalid_state() {
        for (query, title, accepted) in [
            (
                "code=unit-code&state=unit-state",
                "Continue in Noema Desktop",
                true,
            ),
            (
                "error=access_denied&state=unit-state",
                "Connection declined",
                false,
            ),
            (
                "error=access_denied&state=wrong-state",
                "Sign-in did not finish",
                false,
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
            let address = listener.local_addr().expect("address");
            let redirect = format!("http://{address}{CALLBACK_PATH}");
            let client = tokio::spawn(async move {
                let mut stream = TcpStream::connect(address).await.expect("connect");
                let request =
                    format!("GET {CALLBACK_PATH}?{query} HTTP/1.1\r\nHost: {address}\r\n\r\n");
                stream.write_all(request.as_bytes()).await.expect("request");
                let mut body = String::new();
                stream.read_to_string(&mut body).await.expect("response");
                body
            });
            let result = receive_callback(
                &listener,
                &redirect,
                "https://noema.example",
                &CsrfToken::new("unit-state".into()),
            )
            .await;
            assert_eq!(result.is_ok(), accepted);
            let body = client.await.expect("client");
            assert!(body.contains(title));
            assert!(!body.contains("unit-code"));
            assert!(!body.contains("Authorization complete"));
        }
    }

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
