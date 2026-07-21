//! Narrow support shared by concrete provider adapters.

mod diagnostics;
mod schema;
mod stream;
mod strict_schema;

#[cfg(all(test, feature = "adapters"))]
pub use diagnostics::SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE;
pub use diagnostics::StructuredResponseDiagnosticContext;
pub use schema::noema_response_text_format;
pub(crate) use schema::noema_response_text_format_with_strict;
pub use stream::NoemaAssistantTextDeltaExtractor;
pub(crate) use strict_schema::lower_strict_schema;
pub use strict_schema::{decode_recursive_json, encode_recursive_json, recursive_json_schema};
