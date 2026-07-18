//! Resolver orchestration for the Work GraphQL surface.
//!
//! Reads, semantic mutations, and shared authorization/error helpers live in
//! separate modules so command and query paths remain independently auditable.

mod mutations;
mod reads;
mod settings;
mod support;

pub(in crate::graphql) use mutations::*;
pub(in crate::graphql) use reads::*;
pub(in crate::graphql) use settings::*;
pub(in crate::graphql) use support::*;
