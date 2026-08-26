//! Narrow support shared by concrete provider adapters.

#[cfg(feature = "adapters")]
mod diagnostics;
#[cfg(feature = "adapters")]
pub(crate) mod http;
mod provider_schema_conversion;
pub(crate) mod sse;
#[cfg(any(feature = "adapters", feature = "local-models"))]
pub(crate) mod tool_names;

#[cfg(feature = "adapters")]
pub use diagnostics::StructuredResponseDiagnosticContext;
pub(crate) use provider_schema_conversion::convert_schema_fully;
