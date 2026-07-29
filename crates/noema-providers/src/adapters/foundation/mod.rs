//! Apple Foundation Models provider and Swift bridge.

mod adapter;
mod availability;
mod bridge;
mod lowering;

pub use adapter::FoundationLocalProvider;
pub(crate) use bridge::FoundationBridgeError;

#[cfg(test)]
mod tests;
