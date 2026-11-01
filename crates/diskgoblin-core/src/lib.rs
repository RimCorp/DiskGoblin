//! Core library for DiskGoblin.

pub mod aggregator;
pub mod cancel;
pub mod config;
pub mod error;
pub mod format;
pub mod model;
pub mod platform;
pub mod scanner;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_fixtures;

pub use cancel::{
    CancelToken, FnProgressSink, NoopProgressSink, ProgressSink, ScanProgress,
    ThrottledProgressSink,
};
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
    scan_directory_with_progress(options, &CancelToken::new(), &NoopProgressSink)
}

/// Scan with cooperative cancellation and progress throttled to 50 ms.
/// A final snapshot is always delivered on success, even within that interval.
pub fn scan_directory_with_progress(
    options: &ScanOptions,
    cancel: &CancelToken,
    progress: &dyn ProgressSink,
) -> Result<ScanReport, FatalScanError> {
    aggregator::scan(options, cancel, progress)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_package_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
