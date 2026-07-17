//! Durable one-off task records and state transitions.

#![allow(clippy::missing_errors_doc)]

mod events;
mod lifecycle;
pub(crate) mod provider_selection;
mod reviews;
mod submissions;

#[cfg(test)]
mod tests;
