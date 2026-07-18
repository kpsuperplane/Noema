//! Shared transaction-local provider selection readers for Work commands.

// Keep this namespace until the provider-selection reader is moved beside the
// Work command writer.  The old task CRUD/event modules were V2-only and are
// intentionally gone; Work commands use this one focused helper instead.
pub(crate) mod provider_selection;
