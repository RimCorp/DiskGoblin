//! Bounded parallel discovery with canonical depth-first event replay.
//!
//! Workers never assign IDs or deduplicate hard links. Ordered replay on the
//! calling thread preserves the sequential oracle's attribution and rollups.
use super::{sequential, TraversalEvent, TraversalStats, VisitedDir, VisitedEntry};
use crate::{
    extract_metadata, CancelToken, EntryKind, FatalScanError, NoopProgressSink, ProgressSink,
    RawMetadata, ScanErrorRecord, ScanOptions, ScanProgress,
};
use rayon::prelude::*;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

struct Node {
    path: PathBuf,
    metadata: RawMetadata,
    children: Vec<Child>,
    errors: Vec<ScanErrorRecord>,
    fully_read: bool,
}

enum Child {
    Node(Node),
    Error(ScanErrorRecord),
}

#[derive(Default)]
struct Counters {
    files: AtomicU64,
    dirs: AtomicU64,
    symlinks: AtomicU64,
    errors: AtomicU64,
}

struct Discovery<'a, F> {
    options: &'a ScanOptions,
    cancel: &'a CancelToken,
    sink: &'a dyn ProgressSink,
    counters: Counters,
    started: Instant,
    root_device: Option<u64>,
    extract: F,
}

impl<F> Discovery<'_, F>
where
    F: Fn(&Path, &fs::Metadata) -> io::Result<RawMetadata> + Sync,
{
    fn progress(&self) {
        // Discovery cannot yet attribute hard-link bytes canonically. Bytes
        // stay zero until the aggregator flushes the completed replay totals.
        self.sink.on_progress(ScanProgress {
            files_seen: self.counters.files.load(Ordering::Relaxed),
            dirs_seen: self.counters.dirs.load(Ordering::Relaxed),
            symlinks_seen: self.counters.symlinks.load(Ordering::Relaxed),
            errors_seen: self.counters.errors.load(Ordering::Relaxed),
            bytes_accumulated: 0,
            elapsed_ms: self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        });
    }

    fn error(&self, path: PathBuf, error: io::Error) -> ScanErrorRecord {
        self.counters.errors.fetch_add(1, Ordering::Relaxed);
        ScanErrorRecord::from_io_error(path, error)
    }

    fn directory(
        &self,
        path: PathBuf,
        metadata: RawMetadata,
        entries: fs::ReadDir,
    ) -> Result<Node, FatalScanError> {
        check_cancel(self.cancel)?;
        let mut node = Node {
            path,
            metadata,
            children: Vec::new(),
            errors: Vec::new(),
            fully_read: true,
        };
        let mut paths = Vec::new();
        for entry in entries {
            check_cancel(self.cancel)?;
            match entry {
                Ok(entry) => paths.push(entry.path()),
                Err(error) => {
                    node.fully_read = false;
                    node.errors.push(self.error(node.path.clone(), error));
                }
            }
        }
        check_cancel(self.cancel)?;
        paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        check_cancel(self.cancel)?;
        // Indexed parallel collection retains the sorted sibling order.
        node.children = paths
            .into_par_iter()
            .map(|path| self.child(path))
            .collect::<Result<Vec<_>, _>>()?;
        check_cancel(self.cancel)?;
        Ok(node)
    }

    fn child(&self, path: PathBuf) -> Result<Child, FatalScanError> {
        check_cancel(self.cancel)?;
        let raw = match fs::symlink_metadata(&path).and_then(|m| (self.extract)(&path, &m)) {
            Ok(raw) => raw,
            Err(error) => return Ok(Child::Error(self.error(path, error))),
        };
        check_cancel(self.cancel)?;
        let counter = match raw.kind {
            EntryKind::File => Some(&self.counters.files),
            EntryKind::Directory => Some(&self.counters.dirs),
            EntryKind::Symlink => Some(&self.counters.symlinks),
            EntryKind::Other => None,
        };
        if let Some(counter) = counter {
            let previous = counter.fetch_add(1, Ordering::Relaxed);
            if previous & 255 == 0 {
                self.progress();
            }
        }
        check_cancel(self.cancel)?;
        let different_device = matches!((self.root_device, raw.file_id),
            (Some(root), Some(child)) if root != child.device_id);
        if raw.kind == EntryKind::Directory && !(self.options.one_file_system && different_device) {
            match fs::read_dir(&path) {
                Ok(entries) => return self.directory(path, raw, entries).map(Child::Node),
                Err(error) => {
                    let record = self.error(path.clone(), error);
                    return Ok(Child::Node(Node {
                        path,
                        metadata: raw,
                        children: Vec::new(),
                        errors: vec![record],
                        fully_read: false,
                    }));
                }
            }
        }
        Ok(Child::Node(Node {
            path,
            metadata: raw,
            children: Vec::new(),
            errors: Vec::new(),
            fully_read: false,
        }))
    }
}

/// Scan with a bounded pool, replaying events on the calling thread.
pub fn scan_with_cancel(
    options: &ScanOptions,
    cancel: &CancelToken,
    visit: impl FnMut(TraversalEvent),
) -> Result<TraversalStats, FatalScanError> {
    scan_with_progress(options, cancel, &NoopProgressSink, visit)
}

pub(crate) fn scan_with_progress(
    options: &ScanOptions,
    cancel: &CancelToken,
    sink: &dyn ProgressSink,
    visit: impl FnMut(TraversalEvent),
) -> Result<TraversalStats, FatalScanError> {
    let num_threads = if options.threads == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .clamp(1, 8)
    } else {
        options.threads
    };
    if num_threads <= 1 {
        return sequential::scan_with_cancel(options, cancel, visit);
    }
    scan_with_metadata(options, cancel, sink, visit, num_threads, |_, m| {
        Ok(extract_metadata(m))
    })
}

fn scan_with_metadata(
    options: &ScanOptions,
    cancel: &CancelToken,
    sink: &dyn ProgressSink,
    mut visit: impl FnMut(TraversalEvent),
    num_threads: usize,
    extract: impl Fn(&Path, &fs::Metadata) -> io::Result<RawMetadata> + Sync,
) -> Result<TraversalStats, FatalScanError> {
    check_cancel(cancel)?;
    let root = &options.root_path;
    let unreadable = |source| FatalScanError::RootUnreadable {
        path: root.clone(),
        source,
    };
    let metadata = fs::symlink_metadata(root).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            FatalScanError::RootNotFound { path: root.clone() }
        } else {
            unreadable(source)
        }
    })?;
    let raw = extract(root, &metadata).map_err(unreadable)?;
    if raw.kind != EntryKind::Directory {
        return Err(unreadable(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scan root must be a directory",
        )));
    }
    let entries = fs::read_dir(root).map_err(unreadable)?;
    let discovery = Discovery {
        options,
        cancel,
        sink,
        counters: Counters::default(),
        started: Instant::now(),
        root_device: raw.file_id.map(|id| id.device_id),
        extract,
    };
    discovery.progress();
    check_cancel(cancel)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .map_err(|error| unreadable(io::Error::other(error)))?;
    let tree = pool.install(|| discovery.directory(root.clone(), raw, entries))?;
    check_cancel(cancel)?;
    let mut stats = TraversalStats::default();
    let mut next_id = 0;
    replay(tree, None, 0, cancel, &mut next_id, &mut stats, &mut visit)?;
    check_cancel(cancel)?;
    Ok(stats)
}

fn replay(
    node: Node,
    parent_id: Option<usize>,
    depth: usize,
    cancel: &CancelToken,
    next_id: &mut usize,
    stats: &mut TraversalStats,
    visit: &mut impl FnMut(TraversalEvent),
) -> Result<(), FatalScanError> {
    check_cancel(cancel)?;
    let id = *next_id;
    *next_id += 1;
    let kind = node.metadata.kind;
    match kind {
        EntryKind::File => stats.file_count += 1,
        EntryKind::Directory => stats.dir_count += 1,
        EntryKind::Symlink => stats.symlink_count += 1,
        EntryKind::Other => {}
    }
    visit(TraversalEvent::Entry(VisitedEntry {
        id,
        parent_id,
        path: node.path,
        depth,
        metadata: node.metadata,
    }));
    let mut errors = node.errors.len() as u64;
    for record in node.errors {
        check_cancel(cancel)?;
        stats.error_count += 1;
        visit(TraversalEvent::Error {
            directory_id: id,
            record,
        });
    }
    for child in node.children {
        check_cancel(cancel)?;
        match child {
            Child::Node(child) => {
                replay(child, Some(id), depth + 1, cancel, next_id, stats, visit)?
            }
            Child::Error(record) => {
                errors += 1;
                stats.error_count += 1;
                visit(TraversalEvent::Error {
                    directory_id: id,
                    record,
                });
            }
        }
    }
    if kind == EntryKind::Directory {
        visit(TraversalEvent::DirectoryFinished(VisitedDir {
            id,
            direct_error_count: errors,
            fully_read: node.fully_read,
        }));
    }
    check_cancel(cancel)
}

fn check_cancel(cancel: &CancelToken) -> Result<(), FatalScanError> {
    if cancel.is_cancelled() {
        Err(FatalScanError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        scan_directory, scan_directory_with_progress, test_fixtures::FixtureBuilder, FileId,
        FnProgressSink, ScanErrorKind, SizeMetric,
    };
    use std::sync::{atomic::AtomicUsize, Barrier, Mutex};
    use std::time::Duration;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn reports_and_events_match_sequential() -> TestResult {
        let builder = FixtureBuilder::new()
            .add_file("z-original/data", 4097)
            .add_hard_link("z-original/data", "a/first")
            .add_hard_link("z-original/data", "a-/second")
            .add_file("a/deep/other", 13)
            .add_dir("empty");
        #[cfg(unix)]
        let builder = builder
            .add_symlink("../..", "a/deep/loop")
            .add_symlink("missing", "dangling")
            .add_file("denied/hidden", 27)
            .add_unreadable_dir("denied");
        let fixture = builder.build()?;
        for size_metric in [SizeMetric::Apparent, SizeMetric::Allocated] {
            for one_file_system in [false, true] {
                for max_depth in [None, Some(0), Some(1)] {
                    let mut options = ScanOptions {
                        root_path: fixture.path().into(),
                        size_metric,
                        one_file_system,
                        max_depth,
                        top_n: 100,
                        ..ScanOptions::default()
                    };
                    let mut expected = scan_directory(&options)?;
                    expected.duration_ms = 0;
                    #[cfg(unix)]
                    if !crate::test_fixtures::is_root_user() {
                        assert_eq!(expected.errors.len(), 1);
                        assert_eq!(expected.errors[0].kind, ScanErrorKind::PermissionDenied);
                    }
                    assert!(expected
                        .top_files
                        .iter()
                        .any(|file| file.path == fixture.path().join("a/first")));
                    assert!(!expected
                        .top_files
                        .iter()
                        .any(|file| file.path == fixture.path().join("a-/second")));
                    let mut expected_events = Vec::new();
                    let expected_stats = sequential::scan(&options, |e| expected_events.push(e))?;
                    for threads in [0, 2, 4, 8] {
                        options.threads = threads;
                        let mut actual = scan_directory(&options)?;
                        actual.duration_ms = 0;
                        assert_eq!(actual, expected, "threads={threads}");
                        let mut events = Vec::new();
                        let stats =
                            scan_with_cancel(&options, &CancelToken::new(), |e| events.push(e))?;
                        assert_eq!(events, expected_events);
                        assert_eq!(stats, expected_stats);
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn injected_mount_boundaries_and_metadata_errors() -> TestResult {
        let fixture = FixtureBuilder::new()
            .add_file("mount/hidden", 4)
            .add_file("sibling", 2)
            .add_file("vanished", 1)
            .build()?;
        for one_file_system in [false, true] {
            let options = ScanOptions {
                root_path: fixture.path().into(),
                one_file_system,
                threads: 4,
                ..ScanOptions::default()
            };
            let mut oracle = None;
            for threads in [1, 2, 4, 8] {
                let mut events = Vec::new();
                let stats = scan_with_metadata(
                    &options,
                    &CancelToken::new(),
                    &NoopProgressSink,
                    |event| events.push(event),
                    threads,
                    |path, metadata| {
                        if path == fixture.path().join("vanished") {
                            return Err(io::Error::new(io::ErrorKind::NotFound, "disappeared"));
                        }
                        let mut raw = extract_metadata(metadata);
                        raw.file_id = Some(FileId {
                            device_id: u64::from(path == fixture.path().join("mount")),
                            inode: 1,
                        });
                        Ok(raw)
                    },
                )?;
                assert_eq!(stats.file_count, if one_file_system { 1 } else { 2 });
                assert_eq!(stats.error_count, 1);
                assert!(events.iter().any(|event| matches!(event,
                    TraversalEvent::DirectoryFinished(dir) if dir.id == 1 && dir.fully_read != one_file_system)));
                assert!(events.iter().any(|event| matches!(event,
                    TraversalEvent::Error { directory_id: 0, record } if record.kind == ScanErrorKind::NotFound)));
                if let Some(expected) = &oracle {
                    assert_eq!(&events, expected);
                } else {
                    oracle = Some(events);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn cancellation_stops_all_active_workers_within_100_ms() -> TestResult {
        let mut builder = FixtureBuilder::new();
        for dir in 0..32 {
            for file in 0..32 {
                builder = builder.add_file(format!("dir{dir:02}/file{file:02}"), 1);
            }
        }
        let fixture = builder.build()?;
        let options = ScanOptions {
            root_path: fixture.path().into(),
            threads: 4,
            ..ScanOptions::default()
        };
        let cancel = CancelToken::new();
        let barrier = Barrier::new(5);
        let arrived = AtomicUsize::new(0);
        let active = AtomicUsize::new(0);
        let cancelled_at = Mutex::new(None);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                // Four workers rendezvous inside metadata extraction, proving
                // cancellation is tested during discovery rather than replay.
                barrier.wait();
                *cancelled_at.lock().unwrap() = Some(Instant::now());
                cancel.cancel();
            });
            let result = scan_with_metadata(
                &options,
                &cancel,
                &NoopProgressSink,
                |_| {},
                4,
                |path, metadata| {
                    if path != fixture.path() {
                        active.fetch_add(1, Ordering::SeqCst);
                        if arrived.fetch_add(1, Ordering::SeqCst) < 4 {
                            barrier.wait();
                        }
                        while !cancel.is_cancelled() {
                            std::thread::yield_now();
                        }
                        active.fetch_sub(1, Ordering::SeqCst);
                    }
                    Ok(extract_metadata(metadata))
                },
            );
            assert!(matches!(result, Err(FatalScanError::Cancelled)));
            assert_eq!(active.load(Ordering::SeqCst), 0);
            assert!(cancelled_at.lock().unwrap().unwrap().elapsed() < Duration::from_millis(100));
        });
        Ok(())
    }

    #[test]
    fn public_progress_cancellation_and_final_totals() -> TestResult {
        let fixture = FixtureBuilder::new()
            .add_file("a/file", 31)
            .add_hard_link("a/file", "b/link")
            .build()?;
        for threads in [0, 4] {
            let options = ScanOptions {
                root_path: fixture.path().into(),
                threads,
                ..ScanOptions::default()
            };
            let cancel = CancelToken::new();
            let cancelled_at = Mutex::new(None);
            let sink = FnProgressSink::new(|p: ScanProgress| {
                if p.files_seen > 0 {
                    *cancelled_at.lock().unwrap() = Some(Instant::now());
                    cancel.cancel();
                }
            });
            assert!(matches!(
                scan_directory_with_progress(&options, &cancel, &sink),
                Err(FatalScanError::Cancelled)
            ));
            assert!(cancelled_at.lock().unwrap().unwrap().elapsed() < Duration::from_millis(100));
            let snapshots = Mutex::new(Vec::new());
            let sink = FnProgressSink::new(|p| snapshots.lock().unwrap().push(p));
            let report = scan_directory_with_progress(&options, &CancelToken::new(), &sink)?;
            let snapshots = snapshots.lock().unwrap();
            let last = snapshots.last().unwrap();
            assert_eq!(last.files_seen, report.root.summary.file_count);
            assert_eq!(last.dirs_seen, report.root.summary.dir_count);
            assert_eq!(last.errors_seen, report.root.summary.error_count);
            assert_eq!(last.bytes_accumulated, report.root.summary.allocated_size);
            assert_eq!(last.elapsed_ms, report.duration_ms);
        }
        Ok(())
    }

    #[test]
    fn root_validation_and_sequential_fallback() -> TestResult {
        let fixture = FixtureBuilder::new().add_file("file", 1).build()?;
        for threads in [0, 1, 4] {
            for root_path in [fixture.path().join("missing"), fixture.path().join("file")] {
                let options = ScanOptions {
                    root_path: root_path.clone(),
                    threads,
                    ..ScanOptions::default()
                };
                let result = scan_with_cancel(&options, &CancelToken::new(), |_| {});
                if root_path.ends_with("missing") {
                    assert!(matches!(result, Err(FatalScanError::RootNotFound { .. })));
                } else {
                    assert!(matches!(result, Err(FatalScanError::RootUnreadable { .. })));
                }
                let token = CancelToken::new();
                token.cancel();
                assert!(matches!(
                    scan_with_cancel(&options, &token, |_| panic!(
                        "cancelled scan emitted events"
                    )),
                    Err(FatalScanError::Cancelled)
                ));
            }
        }
        Ok(())
    }
}
