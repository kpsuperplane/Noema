//! Memory-owned filesystem layout derived from an explicit Noema root.

use std::path::{Path, PathBuf};

/// Resolved local paths owned by the memory subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryServicePaths {
    root: PathBuf,
}

impl MemoryServicePaths {
    /// Derive memory paths from an explicit Noema root directory.
    #[must_use]
    pub fn from_noema_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Return the Mnemosyne-owned state directory.
    #[must_use]
    pub fn mnemosyne_dir(&self) -> PathBuf {
        self.root.join("mnemosyne")
    }

    /// Return the Mnemosyne managed data directory.
    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.mnemosyne_dir().join("data")
    }

    /// Return the Mnemosyne runtime-state directory.
    #[must_use]
    pub fn runtime_dir(&self) -> PathBuf {
        self.mnemosyne_dir().join("run")
    }

    /// Return the explicit Noema root used for derivation.
    #[must_use]
    pub fn noema_root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn memory_paths_are_derived_from_explicit_root() {
        let paths = MemoryServicePaths::from_noema_root("/tmp/noema");

        assert_eq!(paths.noema_root(), Path::new("/tmp/noema"));
        assert_eq!(paths.mnemosyne_dir(), PathBuf::from("/tmp/noema/mnemosyne"));
        assert_eq!(paths.data_dir(), PathBuf::from("/tmp/noema/mnemosyne/data"));
        assert_eq!(
            paths.runtime_dir(),
            PathBuf::from("/tmp/noema/mnemosyne/run")
        );
    }
}
