//! Deterministic, read-only, iterative depth-first traversal.
use super::{TraversalEvent, TraversalStats, VisitedDir, VisitedEntry};
use crate::{
    extract_metadata, CancelToken, EntryKind, FatalScanError, RawMetadata, ScanErrorRecord,
    ScanOptions,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

struct Frame {
    id: usize,
    path: PathBuf,
    depth: usize,
    entries: std::vec::IntoIter<PathBuf>,
    errors: u64,
    fully_read: bool,
}

/// Streams raw entries and errors to `visit` without retaining files in the result.
///
/// The root must itself be a directory (root symlinks are not resolved). All
/// descendants are scanned regardless of presentation options such as max_depth,
/// top_n and min_size_bytes; these belong to downstream aggregation/rendering.
/// Missing device IDs do not impose a mount boundary on unsupported platforms.
/// No parallel strategy is selected here, regardless of `options.threads`.
pub fn scan(
    options: &ScanOptions,
    visit: impl FnMut(TraversalEvent),
) -> Result<TraversalStats, FatalScanError> {
    scan_with_cancel(options, &CancelToken::new(), visit)
}

/// Traverse while checking cancellation at directory boundaries and every 256 entries.
pub fn scan_with_cancel(
    options: &ScanOptions,
    cancel: &CancelToken,
    visit: impl FnMut(TraversalEvent),
) -> Result<TraversalStats, FatalScanError> {
    scan_with_metadata(options, cancel, visit, |_, metadata| {
        Ok(extract_metadata(metadata))
    })
}

fn scan_with_metadata(
    options: &ScanOptions,
    cancel: &CancelToken,
    mut visit: impl FnMut(TraversalEvent),
    mut extract: impl FnMut(&Path, &fs::Metadata) -> io::Result<RawMetadata>,
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
    let root_device = raw.file_id.map(|id| id.device_id);
    let mut stats = TraversalStats {
        dir_count: 1,
        ..TraversalStats::default()
    };
    visit(TraversalEvent::Entry(VisitedEntry {
        id: 0,
        parent_id: None,
        path: root.clone(),
        depth: 0,
        metadata: raw,
    }));
    let mut stack = vec![frame(
        0,
        root.clone(),
        0,
        entries,
        &mut visit,
        &mut stats,
        cancel,
    )?];
    let mut entries_seen = 0usize;
    let mut next_id = 1;
    while let Some(mut current) = stack.pop() {
        if entries_seen & 255 == 0 {
            check_cancel(cancel)?;
        }
        entries_seen = entries_seen.wrapping_add(1);
        let Some(path) = current.entries.next() else {
            visit(TraversalEvent::DirectoryFinished(VisitedDir {
                id: current.id,
                direct_error_count: current.errors,
                fully_read: current.fully_read,
            }));
            continue;
        };
        let raw = match fs::symlink_metadata(&path).and_then(|metadata| extract(&path, &metadata)) {
            Ok(raw) => raw,
            Err(error) => {
                record_error(&mut current, path, error, &mut visit, &mut stats);
                stack.push(current);
                continue;
            }
        };
        let id = next_id;
        next_id += 1;
        let depth = current.depth + 1;
        visit(TraversalEvent::Entry(VisitedEntry {
            id,
            parent_id: Some(current.id),
            path: path.clone(),
            depth,
            metadata: raw,
        }));
        match raw.kind {
            EntryKind::File => stats.file_count += 1,
            EntryKind::Symlink => stats.symlink_count += 1,
            EntryKind::Directory => stats.dir_count += 1,
            EntryKind::Other => {}
        }
        stack.push(current);
        if raw.kind == EntryKind::Directory {
            check_cancel(cancel)?;
            let different_device = matches!((root_device, raw.file_id), (Some(root), Some(child)) if root != child.device_id);
            if options.one_file_system && different_device {
                visit(TraversalEvent::DirectoryFinished(VisitedDir {
                    id,
                    direct_error_count: 0,
                    fully_read: false,
                }));
                continue;
            }
            match fs::read_dir(&path) {
                Ok(entries) => stack.push(frame(
                    id, path, depth, entries, &mut visit, &mut stats, cancel,
                )?),
                Err(error) => {
                    let mut failed = Frame {
                        id,
                        path: path.clone(),
                        depth,
                        entries: Vec::new().into_iter(),
                        errors: 0,
                        fully_read: false,
                    };
                    record_error(&mut failed, path, error, &mut visit, &mut stats);
                    stack.push(failed);
                }
            }
        }
    }
    check_cancel(cancel)?;
    Ok(stats)
}

fn frame(
    id: usize,
    path: PathBuf,
    depth: usize,
    entries: fs::ReadDir,
    visit: &mut impl FnMut(TraversalEvent),
    stats: &mut TraversalStats,
    cancel: &CancelToken,
) -> Result<Frame, FatalScanError> {
    check_cancel(cancel)?;
    let mut result = Frame {
        id,
        path,
        depth,
        entries: Vec::new().into_iter(),
        errors: 0,
        fully_read: true,
    };
    let mut paths = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index & 255 == 0 {
            check_cancel(cancel)?;
        }
        match entry {
            Ok(entry) => paths.push(entry.path()),
            // read_dir iteration errors do not supply a child filename.
            Err(error) => {
                result.fully_read = false;
                let path = result.path.clone();
                record_error(&mut result, path, error, visit, stats);
            }
        }
    }
    check_cancel(cancel)?;
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    result.entries = paths.into_iter();
    check_cancel(cancel)?;
    Ok(result)
}

fn check_cancel(cancel: &CancelToken) -> Result<(), FatalScanError> {
    if cancel.is_cancelled() {
        Err(FatalScanError::Cancelled)
    } else {
        Ok(())
    }
}

fn record_error(
    frame: &mut Frame,
    path: PathBuf,
    error: io::Error,
    visit: &mut impl FnMut(TraversalEvent),
    stats: &mut TraversalStats,
) {
    frame.errors += 1;
    stats.error_count += 1;
    visit(TraversalEvent::Error {
        directory_id: frame.id,
        record: ScanErrorRecord::from_io_error(path, error),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_fixtures::FixtureBuilder, FileId, ScanErrorKind};

    #[test]
    fn mount_boundary_and_metadata_failure() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = FixtureBuilder::new()
            .add_file("mount/hidden", 4)
            .add_file("sibling", 2)
            .build()?;
        for one_file_system in [true, false] {
            let options = ScanOptions {
                root_path: fixture.path().into(),
                one_file_system,
                ..ScanOptions::default()
            };
            let mut events = Vec::new();
            let stats = scan_with_metadata(
                &options,
                &CancelToken::new(),
                |event| events.push(event),
                |path, metadata| {
                    let mut raw = extract_metadata(metadata);
                    raw.file_id = Some(FileId {
                        device_id: u64::from(path == fixture.path().join("mount")),
                        inode: 1,
                    });
                    Ok(raw)
                },
            )?;
            assert_eq!(stats.file_count, if one_file_system { 1 } else { 2 });
            assert!(events.iter().any(|event| matches!(event, TraversalEvent::DirectoryFinished(dir) if dir.id == 1 && dir.fully_read != one_file_system)));
        }
        let mut events = Vec::new();
        let options = ScanOptions {
            root_path: fixture.path().into(),
            ..ScanOptions::default()
        };
        let stats = scan_with_metadata(
            &options,
            &CancelToken::new(),
            |event| events.push(event),
            |path, metadata| {
                if path == fixture.path().join("mount") {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "disappeared"));
                }
                Ok(extract_metadata(metadata))
            },
        )?;
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.file_count, 1);
        assert!(events.iter().any(|event| matches!(event, TraversalEvent::Error { directory_id: 0, record } if record.kind == ScanErrorKind::NotFound)));
        assert!(events.iter().any(|event| matches!(event, TraversalEvent::DirectoryFinished(dir) if dir.id == 0 && dir.direct_error_count == 1)));
        Ok(())
    }
}
