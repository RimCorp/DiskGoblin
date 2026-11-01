//! Streaming bottom-up rollups and bounded, deterministic top-N selection.
use crate::{
    scanner::{sequential, TraversalEvent},
    DirNode, DirSummary, EntryKind, FatalScanError, FileEntry, ScanOptions, ScanReport,
};
use std::{
    cmp::{Ordering, Reverse},
    collections::{BinaryHeap, HashSet},
    path::PathBuf,
    time::Instant,
};

// The greatest key is the worst retained candidate: smallest size, largest path.
struct Ranked<T> {
    key: (Reverse<u64>, PathBuf),
    value: T,
}

impl<T> PartialEq for Ranked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl<T> Eq for Ranked<T> {}
impl<T> PartialOrd for Ranked<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Ranked<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

fn offer<T>(
    heap: &mut BinaryHeap<Ranked<T>>,
    value: T,
    size: u64,
    path: PathBuf,
    options: &ScanOptions,
) {
    if options.top_n == 0 || size < options.min_size_bytes {
        return;
    }
    let candidate = Ranked {
        key: (Reverse(size), path),
        value,
    };
    if heap.len() < options.top_n {
        heap.push(candidate);
    } else if let Some(mut worst) = heap.peek_mut() {
        if candidate < *worst {
            *worst = candidate;
        }
    }
}

fn sorted<T>(heap: BinaryHeap<Ranked<T>>) -> Vec<T> {
    heap.into_sorted_vec()
        .into_iter()
        .map(|v| v.value)
        .collect()
}

fn empty_node(path: PathBuf, depth: usize) -> DirNode {
    DirNode {
        summary: DirSummary {
            path,
            depth,
            apparent_size: 0,
            allocated_size: 0,
            file_count: 0,
            dir_count: 0,
            symlink_count: 0,
            error_count: 0,
        },
        children: Vec::new(),
    }
}

/// Aggregate the sequential scanner without retaining individual files beyond top-N.
///
/// Directory inode bytes are excluded; symlinks contribute their own inode sizes.
/// Directory counts exclude the directory itself. Pruning affects only the result
/// tree: rankings and totals include every scanned depth.
pub(crate) fn scan(options: &ScanOptions) -> Result<ScanReport, FatalScanError> {
    let started = Instant::now();
    let mut stack: Vec<DirNode> = Vec::new();
    let mut root = empty_node(options.root_path.clone(), 0);
    let mut hardlinks = HashSet::new();
    let mut files = BinaryHeap::new();
    let mut directories = BinaryHeap::new();
    let mut errors = Vec::new();
    sequential::scan(options, |event| match event {
        TraversalEvent::Entry(entry) => {
            let raw = entry.metadata;
            if raw.kind == EntryKind::Directory {
                stack.push(empty_node(entry.path, entry.depth));
                return;
            }
            if let Some(parent) = stack.last_mut() {
                let deduplicated = raw.kind == EntryKind::File
                    && raw.hard_link_count > 1
                    && raw.file_id.is_some_and(|id| !hardlinks.insert(id));
                match raw.kind {
                    EntryKind::File => {
                        parent.summary.file_count += 1;
                        let file = FileEntry {
                            path: entry.path,
                            apparent_size: raw.apparent_size,
                            allocated_size: raw.allocated_size,
                            is_deduplicated_hardlink: deduplicated,
                        };
                        if !file.is_deduplicated_hardlink {
                            offer(
                                &mut files,
                                file.clone(),
                                options
                                    .size_metric
                                    .select(raw.apparent_size, raw.allocated_size),
                                file.path,
                                options,
                            );
                        }
                    }
                    EntryKind::Symlink => parent.summary.symlink_count += 1,
                    EntryKind::Other | EntryKind::Directory => {}
                }
                if !deduplicated {
                    parent.summary.apparent_size += raw.apparent_size;
                    parent.summary.allocated_size += raw.allocated_size;
                }
            }
        }
        // Completion supplies the authoritative direct count, avoiding double counting.
        TraversalEvent::Error { record, .. } => errors.push(record),
        TraversalEvent::DirectoryFinished(dir) => {
            if let Some(mut node) = stack.pop() {
                node.summary.error_count += dir.direct_error_count;
                node.children.sort_by(|a, b| {
                    let size = |node: &DirNode| {
                        options
                            .size_metric
                            .select(node.summary.apparent_size, node.summary.allocated_size)
                    };
                    size(b)
                        .cmp(&size(a))
                        .then_with(|| a.summary.path.cmp(&b.summary.path))
                });
                if let Some(parent) = stack.last_mut() {
                    let child = &node.summary;
                    offer(
                        &mut directories,
                        child.clone(),
                        options
                            .size_metric
                            .select(child.apparent_size, child.allocated_size),
                        child.path.clone(),
                        options,
                    );
                    parent.summary.apparent_size += child.apparent_size;
                    parent.summary.allocated_size += child.allocated_size;
                    parent.summary.file_count += child.file_count;
                    parent.summary.dir_count += child.dir_count + 1;
                    parent.summary.symlink_count += child.symlink_count;
                    parent.summary.error_count += child.error_count;
                    if options.max_depth.is_none_or(|limit| child.depth <= limit) {
                        parent.children.push(node);
                    }
                } else {
                    root = node;
                }
            }
        }
    })?;
    Ok(ScanReport {
        schema_version: 1,
        root_path: options.root_path.clone(),
        size_metric: options.size_metric,
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        root,
        top_directories: sorted(directories),
        top_files: sorted(files),
        errors,
    })
}
