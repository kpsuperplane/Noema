//! Narrow support shared by concrete provider adapters.

mod diagnostics;
mod schema;
mod stream;

pub use diagnostics::{
    SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, StructuredResponseDiagnosticContext,
};
pub use schema::noema_response_text_format;
pub use stream::NoemaAssistantTextDeltaExtractor;
