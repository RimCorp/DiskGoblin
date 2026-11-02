//! Streaming bottom-up rollups and bounded, deterministic top-N selection.
use crate::{
    scanner::{parallel, sequential, TraversalEvent},
    CancelToken, DirNode, DirSummary, EntryKind, FatalScanError, FileEntry, ProgressSink,
    ScanOptions, ScanProgress, ScanReport, ThrottledProgressSink,
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

/// Aggregate the selected scanner without retaining individual files beyond top-N.
///
/// Directory inode bytes are excluded; symlinks contribute their own inode sizes.
/// Directory counts exclude the directory itself. Pruning affects only the result
/// tree: rankings and totals include every scanned depth.
pub(crate) fn scan(
    options: &ScanOptions,
    cancel: &CancelToken,
    sink: &dyn ProgressSink,
) -> Result<ScanReport, FatalScanError> {
    let started = Instant::now();
    let mut stack: Vec<DirNode> = Vec::new();
    let mut root = empty_node(options.root_path.clone(), 0);
    let mut hardlinks = HashSet::new();
    let mut files = BinaryHeap::new();
    let mut directories = BinaryHeap::new();
    let mut errors = Vec::new();
    let progress_sink = ThrottledProgressSink::new(sink);
    let mut progress = ScanProgress::default();
    let mut entries_seen = 0usize;
    // Auto mode can delegate to sequential traversal on a single-worker host.
    let replay_progress = options.threads == 1
        || (options.threads == 0
            && std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                <= 1);
    let visit = |event| {
        let transition = matches!(
            &event,
            TraversalEvent::DirectoryFinished(_)
                | TraversalEvent::Entry(crate::scanner::VisitedEntry {
                    metadata: crate::RawMetadata {
                        kind: EntryKind::Directory,
                        ..
                    },
                    ..
                })
        );
        match &event {
            TraversalEvent::Entry(entry) => {
                entries_seen = entries_seen.wrapping_add(1);
                match entry.metadata.kind {
                    EntryKind::File => progress.files_seen += 1,
                    EntryKind::Directory if entry.parent_id.is_some() => progress.dirs_seen += 1,
                    EntryKind::Symlink => progress.symlinks_seen += 1,
                    _ => {}
                }
            }
            TraversalEvent::Error { .. } => progress.errors_seen += 1,
            _ => {}
        }
        match event {
            TraversalEvent::Entry(entry) => {
                let raw = entry.metadata;
                if raw.kind == EntryKind::Directory {
                    stack.push(empty_node(entry.path, entry.depth));
                } else if let Some(parent) = stack.last_mut() {
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
                        progress.bytes_accumulated += options
                            .size_metric
                            .select(raw.apparent_size, raw.allocated_size);
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
        }
        if replay_progress && (transition || entries_seen & 255 == 0) {
            progress.elapsed_ms = elapsed_ms(started);
            progress_sink.on_progress(progress);
        }
    };
    if options.threads == 1 {
        sequential::scan_with_cancel(options, cancel, visit)?;
    } else {
        parallel::scan_with_progress(options, cancel, &progress_sink, visit)?;
    }
    let report = ScanReport {
        schema_version: 1,
        root_path: options.root_path.clone(),
        size_metric: options.size_metric,
        duration_ms: elapsed_ms(started),
        root,
        top_directories: sorted(directories),
        top_files: sorted(files),
        errors,
    };
    progress.elapsed_ms = report.duration_ms;
    progress_sink.flush(progress);
    if cancel.is_cancelled() {
        return Err(FatalScanError::Cancelled);
    }
    Ok(report)
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
