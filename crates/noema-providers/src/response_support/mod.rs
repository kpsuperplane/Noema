//! Narrow support shared by concrete provider adapters.

mod diagnostics;
pub(crate) mod http;
pub(crate) mod sse;
mod strict_schema;
#[cfg(feature = "adapters")]
pub(crate) mod tool_names;

pub use diagnostics::StructuredResponseDiagnosticContext;
pub(crate) use strict_schema::lower_strict_schema;
