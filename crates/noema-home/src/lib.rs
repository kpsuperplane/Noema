//! Noema home layout, safe paths, initialization, and developer diagnostics.

mod diagnostics;
mod initialization;
mod paths;
mod private_files;
mod safe_path;

pub use diagnostics::{SystemErrorEvent, SystemErrorLogger};
pub use initialization::{NoemaHomeError, init_noema_home};
pub use paths::{NOEMA_HOME_ENV, NoemaPathError, NoemaPaths};
pub use private_files::{atomic_write_private, ensure_private_dir};
pub use safe_path::sanitize_path_segment;
