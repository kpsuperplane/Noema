//! Store-local object reference aliases.
//!
//! The embedded store currently uses the public memory persistence reference
//! types while the SurrealDB repositories are being split out. Keeping these
//! aliases local to `store` gives future graph-memory code a stable import path
//! without introducing a second object-reference model.

pub use crate::memory_persistence::{ActorRef, ObjectRef};
