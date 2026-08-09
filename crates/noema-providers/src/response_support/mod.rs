//! Narrow support shared by concrete provider adapters.

mod diagnostics;
pub(crate) mod http;
mod provider_schema_conversion;
pub(crate) mod sse;
#[cfg(feature = "adapters")]
pub(crate) mod tool_names;

pub use diagnostics::StructuredResponseDiagnosticContext;
pub(crate) use provider_schema_conversion::convert_schema_fully;
