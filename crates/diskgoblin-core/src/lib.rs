//! Core library for DiskGoblin.

pub mod aggregator;
pub mod config;
pub mod error;
pub mod format;
pub mod model;
pub mod platform;
pub mod scanner;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_fixtures;

pub use config::{ScanOptions, SizeMetric};
pub use error::{FatalScanError, ScanErrorKind, ScanErrorRecord};
pub use format::{format_bytes, format_percentage, render_ascii_bar, UnitSystem};
pub use model::{DirNode, DirSummary, FileEntry, ScanReport};
pub use platform::{extract_metadata, EntryKind, FileId, RawMetadata};

/// Returns the core library's package version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Scan a directory with deterministic sequential traversal and bounded rankings.
pub fn scan_directory(options: &ScanOptions) -> Result<ScanReport, FatalScanError> {
    aggregator::scan(options)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_package_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
