//! Apple Foundation Models provider and Swift bridge.

mod adapter;
mod availability;
mod bridge;
mod lowering;

pub use adapter::{
    FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS, FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS,
    FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS, FOUNDATION_LOCAL_PROVIDER,
    FoundationLocalProvider,
};
pub use bridge::FoundationBridgeError;

#[cfg(test)]
mod tests;
