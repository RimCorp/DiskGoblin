//! Scan configuration and size selection.
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeMetric {
    Allocated,
    Apparent,
}

impl SizeMetric {
    pub fn select(self, apparent: u64, allocated: u64) -> u64 {
        match self {
            Self::Allocated => allocated,
            Self::Apparent => apparent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOptions {
    pub root_path: PathBuf,
    pub size_metric: SizeMetric,
    pub one_file_system: bool,
    pub max_depth: Option<usize>,
    pub top_n: usize,
    pub min_size_bytes: u64,
    pub threads: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            root_path: PathBuf::from("."),
            size_metric: SizeMetric::Allocated,
            one_file_system: false,
            max_depth: None,
            top_n: 20,
            min_size_bytes: 0,
            threads: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_metric_selection() {
        let options = ScanOptions::default();
        assert_eq!(options.root_path, PathBuf::from("."));
        assert_eq!(options.size_metric, SizeMetric::Allocated);
        assert!(!options.one_file_system);
        assert_eq!(options.max_depth, None);
        assert_eq!(options.top_n, 20);
        assert_eq!(options.min_size_bytes, 0);
        assert_eq!(options.threads, 1);
        assert_eq!(SizeMetric::Apparent.select(17, 4096), 17);
        assert_eq!(SizeMetric::Allocated.select(17, 4096), 4096);
    }
}
