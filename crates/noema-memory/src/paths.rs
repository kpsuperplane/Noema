//! Memory-owned filesystem layout derived from an explicit Noema root.

use std::path::PathBuf;

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

    /// Return the Mnemosyne managed data directory.
    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.root.join("mnemosyne/data")
    }

    /// Return the Mnemosyne runtime-state directory.
    #[must_use]
    pub fn runtime_dir(&self) -> PathBuf {
        self.root.join("mnemosyne/run")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_paths_are_derived_from_explicit_root() {
        let paths = MemoryServicePaths::from_noema_root("/tmp/noema");

        assert_eq!(paths.data_dir(), PathBuf::from("/tmp/noema/mnemosyne/data"));
        assert_eq!(
            paths.runtime_dir(),
            PathBuf::from("/tmp/noema/mnemosyne/run")
        );
    }
}
