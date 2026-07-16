//! Codex provider adapter, OAuth, and model-catalog support.

pub(crate) mod catalog;
pub(crate) mod oauth;
mod responses;

pub(crate) use responses::CodexResponsesProvider;
