//! Serializable scan results and directory trees.
use crate::{config::SizeMetric, error::ScanErrorRecord};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub apparent_size: u64,
    pub allocated_size: u64,
    pub is_deduplicated_hardlink: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DirSummary {
    pub path: PathBuf,
    pub depth: usize,
    pub apparent_size: u64,
    pub allocated_size: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub symlink_count: u64,
    pub error_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DirNode {
    pub summary: DirSummary,
    pub children: Vec<DirNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScanReport {
    pub schema_version: u32,
    pub root_path: PathBuf,
    pub size_metric: SizeMetric,
    pub duration_ms: u64,
    pub root: DirNode,
    pub top_directories: Vec<DirSummary>,
    pub top_files: Vec<FileEntry>,
    pub errors: Vec<ScanErrorRecord>,
}
