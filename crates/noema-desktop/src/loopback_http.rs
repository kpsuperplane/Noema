//! Bounded HTTP request-head parsing for private loopback callbacks.

use tokio::{io::AsyncReadExt as _, net::TcpStream};

pub(crate) struct RequestHead {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) host: Option<String>,
}

impl RequestHead {
    pub(crate) fn path(&self) -> &str {
        self.target
            .split_once('?')
            .map_or(self.target.as_str(), |(path, _)| path)
    }

    pub(crate) fn query(&self) -> Option<&str> {
        self.target.split_once('?').map(|(_, query)| query)
    }

    pub(crate) fn callback_url(&self, authority: &str) -> String {
        format!("http://{authority}{}", self.target)
    }
}

pub(crate) async fn read_request_head(
    stream: &mut TcpStream,
    max_bytes: usize,
) -> Result<RequestHead, String> {
    let mut bytes = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    let header_end = loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|_| "invalid request".to_string())?;
        if read == 0 {
            return Err("missing request".to_string());
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > max_bytes {
            return Err("request too large".to_string());
        }
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
    };
    let text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| "invalid request headers".to_string())?;
    parse_request_head(text)
}

pub(crate) fn parse_request_head(text: &str) -> Result<RequestHead, String> {
    let mut lines = text.lines();
    let mut fields = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?
        .split_whitespace();
    let method = fields.next().ok_or_else(|| "missing method".to_string())?;
    let target = fields.next().ok_or_else(|| "missing target".to_string())?;
    if fields.next() != Some("HTTP/1.1") || fields.next().is_some() {
        return Err("invalid request line".to_string());
    }
    let host = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim().to_string());
    Ok(RequestHead {
        method: method.to_string(),
        target: target.to_string(),
        host,
    })
}
