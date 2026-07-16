//! Noema home layout, safe paths, initialization, and developer diagnostics.

mod diagnostics;
mod initialization;
mod paths;
mod safe_path;

pub use diagnostics::{SystemErrorEvent, SystemErrorLogger, SystemErrorWriteError};
pub use initialization::{
    NoemaHomeError, NoemaHomeInitOptions, NoemaHomeInitResult, init_noema_home,
};
pub use paths::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
pub use safe_path::sanitize_path_segment;
