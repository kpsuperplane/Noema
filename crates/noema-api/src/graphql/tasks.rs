//! GraphQL projections and semantic commands for the Tasks surface.
//!
//! This module deliberately mirrors the current task store and domain boundary:
//! task stage is the only task-level state, while runs, gates, reviews, and
//! attention are independent projections. Resolvers never write rows directly.

mod inputs;
mod projections;
mod resolvers;

pub(in crate::graphql) use inputs::*;
pub(in crate::graphql) use projections::*;
pub(in crate::graphql) use resolvers::*;
