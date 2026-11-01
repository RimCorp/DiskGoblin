//! Core library for DiskGoblin.

pub mod config;
pub mod error;
pub mod model;
pub mod platform;

pub use config::{ScanOptions, SizeMetric};
pub use error::{FatalScanError, ScanErrorKind, ScanErrorRecord};
pub use model::{DirNode, DirSummary, FileEntry, ScanReport};
pub use platform::{extract_metadata, EntryKind, FileId, RawMetadata};

/// Returns the core library's package version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_package_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
