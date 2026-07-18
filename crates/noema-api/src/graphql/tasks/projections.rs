//! GraphQL projections for the Work read and command surfaces.
//!
//! The concrete types are grouped by the stable API domains so that the
//! schema façade stays navigable while all projections remain in one module
//! namespace for async-graphql registration.

mod connections;
mod detail;
mod evidence;
mod foundation;
mod mapping;
mod settings;
mod task;
mod vocabulary;

pub(in crate::graphql) use connections::*;
pub(in crate::graphql) use evidence::*;
pub(in crate::graphql) use foundation::*;
pub(in crate::graphql) use mapping::*;
pub(in crate::graphql) use settings::*;
pub(in crate::graphql) use task::*;
pub(in crate::graphql) use vocabulary::*;
