//! Narrow support shared by concrete provider adapters.

mod diagnostics;
mod strict_schema;

pub use diagnostics::StructuredResponseDiagnosticContext;
pub(crate) use strict_schema::lower_strict_schema;
