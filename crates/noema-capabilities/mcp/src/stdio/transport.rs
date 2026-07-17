//! Size-bounded JSON-lines transport with direct child ownership.

use std::{
    future::ready,
    pin::Pin,
    process::Stdio,
    task::{Context, Poll},
};

use futures_util::{Stream, StreamExt, stream::BoxStream};
use process_wrap::tokio::{ChildWrapper, CommandWrap};
use rmcp::{
    RoleClient,
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::sink_stream::SinkStreamTransport,
};
use tokio::process::ChildStdin;
use tokio_util::{
    bytes::{BufMut, BytesMut},
    codec::{Encoder, FramedRead, FramedWrite, LinesCodec},
};

use crate::limits::MAX_WIRE_FRAME_BYTES;

type StdioSink = FramedWrite<ChildStdin, BoundedJsonRpcEncoder>;
type StdioMessageStream = BoxStream<'static, RxJsonRpcMessage<RoleClient>>;

pub(super) type BoundedStdioTransport = SinkStreamTransport<StdioSink, ChildOwnedStream>;

pub(super) fn spawn(mut command: CommandWrap) -> std::io::Result<BoundedStdioTransport> {
    command
        .command_mut()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    let stdin = child
        .inner_mut()
        .stdin()
        .take()
        .ok_or_else(|| std::io::Error::other("MCP stdio stdin was unavailable"))?;
    let stdout = child
        .inner_mut()
        .stdout()
        .take()
        .ok_or_else(|| std::io::Error::other("MCP stdio stdout was unavailable"))?;
    let sink = FramedWrite::new(stdin, BoundedJsonRpcEncoder);
    let stream =
        FramedRead::new(stdout, bounded_line_codec(MAX_WIRE_FRAME_BYTES))
            .scan((), |(), item| {
                ready(item.ok().and_then(|line| {
                    serde_json::from_str::<RxJsonRpcMessage<RoleClient>>(&line).ok()
                }))
            })
            .boxed();
    Ok(SinkStreamTransport::new(
        sink,
        ChildOwnedStream::new(stream, child),
    ))
}

fn bounded_line_codec(max_length: usize) -> LinesCodec {
    LinesCodec::new_with_max_length(max_length)
}

pub(super) struct BoundedJsonRpcEncoder;

impl Encoder<TxJsonRpcMessage<RoleClient>> for BoundedJsonRpcEncoder {
    type Error = std::io::Error;

    fn encode(
        &mut self,
        message: TxJsonRpcMessage<RoleClient>,
        destination: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        let encoded = serde_json::to_vec(&message).map_err(std::io::Error::other)?;
        if encoded.len() > MAX_WIRE_FRAME_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "MCP stdio request exceeded the wire limit",
            ));
        }
        destination.reserve(encoded.len().saturating_add(1));
        destination.put_slice(&encoded);
        destination.put_u8(b'\n');
        Ok(())
    }
}

pub(super) struct ChildOwnedStream {
    stream: StdioMessageStream,
    child: Option<Box<dyn ChildWrapper>>,
}

impl ChildOwnedStream {
    fn new(stream: StdioMessageStream, child: Box<dyn ChildWrapper>) -> Self {
        Self {
            stream,
            child: Some(child),
        }
    }
}

impl Stream for ChildOwnedStream {
    type Item = RxJsonRpcMessage<RoleClient>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(context)
    }
}

impl Drop for ChildOwnedStream {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = child.start_kill();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = child.wait().await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::{bytes::BytesMut, codec::Decoder};

    use super::*;

    #[test]
    fn stdio_decoder_rejects_a_frame_before_an_unbounded_line_can_accumulate() {
        let mut codec = bounded_line_codec(8);
        let mut bytes = BytesMut::from(&b"123456789"[..]);

        assert!(codec.decode(&mut bytes).is_err());
    }
}
