mod output;
#[cfg(feature = "adapters")]
mod request;
mod sse;
#[cfg(feature = "adapters")]
mod transport;
mod usage;

pub(crate) use crate::response_support::tool_names::OpenAiToolNameMap;
#[cfg(feature = "adapters")]
pub(crate) use output::ChatCompletionResponse;
#[cfg(feature = "adapters")]
pub(crate) use request::{ChatCompletionRequest, ChatMessage, ChatTool};
pub(crate) use sse::ChatSseAccumulator;
#[cfg(feature = "adapters")]
pub(crate) use transport::{ChatDiagnosticContext, ChatTransport};
pub(crate) use usage::ChatUsage;

#[cfg(all(test, feature = "adapters"))]
mod tests;
