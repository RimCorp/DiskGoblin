//! Raw traversal events for downstream aggregation, without retaining every file.

pub mod parallel;
pub mod sequential;

use crate::{RawMetadata, ScanErrorRecord};
use std::path::PathBuf;

/// An entry in depth-first, lexicographical traversal order. IDs are scan-local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitedEntry {
    pub id: usize,
    pub parent_id: Option<usize>,
    pub path: PathBuf,
    pub depth: usize,
    pub metadata: RawMetadata,
}

/// Directory completion, including errors belonging directly to this directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisitedDir {
    pub id: usize,
    pub direct_error_count: u64,
    /// False for unreadable directories and directories excluded by mount policy.
    pub fully_read: bool,
}

/// Entries precede their descendants; directory completion follows descendants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraversalEvent {
    Entry(VisitedEntry),
    Error {
        directory_id: usize,
        record: ScanErrorRecord,
    },
    DirectoryFinished(VisitedDir),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TraversalStats {
    pub file_count: u64,
    /// Includes the root directory and directories skipped at mount boundaries.
    pub dir_count: u64,
    pub symlink_count: u64,
    pub error_count: u64,
}
