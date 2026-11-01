use diskgoblin_core::{
    scanner::{sequential::scan, TraversalEvent},
    test_fixtures::FixtureBuilder,
    FatalScanError, ScanOptions,
};
use std::error::Error;

#[test]
fn missing_and_file_roots_are_fatal() -> Result<(), Box<dyn Error>> {
    let fixture = FixtureBuilder::new().add_file("file", 1).build()?;
    let options = ScanOptions {
        root_path: fixture.path().join("missing"),
        ..ScanOptions::default()
    };
    assert!(matches!(
        scan(&options, |_| {}),
        Err(FatalScanError::RootNotFound { .. })
    ));
    let options = ScanOptions {
        root_path: fixture.path().join("file"),
        ..options
    };
    assert!(matches!(
        scan(&options, |_| {}),
        Err(FatalScanError::RootUnreadable { .. })
    ));
    Ok(())
}

#[test]
fn deterministic_depth_first_metadata_and_parentage() -> Result<(), Box<dyn Error>> {
    let fixture = FixtureBuilder::new()
        .add_file("z", 9)
        .add_file("a/z", 3)
        .add_file("a/a", 2)
        .add_dir("empty")
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        max_depth: Some(0),
        ..ScanOptions::default()
    };
    let mut baseline = Vec::new();
    let stats = scan(&options, |event| baseline.push(event))?;
    assert_eq!((stats.file_count, stats.dir_count), (3, 3));
    let entries: Vec<_> = baseline
        .iter()
        .filter_map(|event| {
            if let TraversalEvent::Entry(entry) = event {
                Some(entry)
            } else {
                None
            }
        })
        .collect();
    let relative: Vec<_> = entries
        .iter()
        .map(|entry| {
            entry
                .path
                .strip_prefix(fixture.path())
                .unwrap()
                .to_str()
                .unwrap()
        })
        .collect();
    assert_eq!(relative, ["", "a", "a/a", "a/z", "empty", "z"]);
    assert_eq!(entries[2].parent_id, Some(entries[1].id));
    assert_eq!(entries[2].depth, 2);
    assert_eq!(entries[2].metadata.apparent_size, 2);
    for _ in 0..5 {
        let mut events = Vec::new();
        assert_eq!(scan(&options, |event| events.push(event))?, stats);
        assert_eq!(events, baseline);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_loop_broken_link_and_symlink_root() -> Result<(), Box<dyn Error>> {
    let fixture = FixtureBuilder::new()
        .add_dir("a/b")
        .add_symlink("../..", "a/b/loop")
        .add_symlink("missing", "dangling")
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        ..ScanOptions::default()
    };
    let stats = scan(&options, |_| {})?;
    assert_eq!(stats.symlink_count, 2);
    assert_eq!(stats.dir_count, 3);
    assert_eq!(stats.error_count, 0);
    let options = ScanOptions {
        root_path: fixture.path().join("a/b/loop"),
        ..options
    };
    assert!(matches!(
        scan(&options, |_| {}),
        Err(FatalScanError::RootUnreadable { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn unreadable_root_and_child() -> Result<(), Box<dyn Error>> {
    use diskgoblin_core::{test_fixtures::is_root_user, ScanErrorKind};
    if is_root_user() {
        return Ok(());
    }
    let fixture = FixtureBuilder::new()
        .add_unreadable_dir("blocked")
        .add_file("readable/data", 7)
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().join("blocked"),
        ..ScanOptions::default()
    };
    assert!(matches!(
        scan(&options, |_| {}),
        Err(FatalScanError::RootUnreadable { .. })
    ));
    let options = ScanOptions {
        root_path: fixture.path().into(),
        ..options
    };
    let mut events = Vec::new();
    let stats = scan(&options, |event| events.push(event))?;
    assert_eq!(stats.file_count, 1);
    assert_eq!(stats.error_count, 1);
    assert!(events.iter().any(|event| matches!(event, TraversalEvent::Error { record, .. } if record.path == fixture.path().join("blocked") && record.kind == ScanErrorKind::PermissionDenied)));
    assert!(events.iter().any(|event| matches!(event, TraversalEvent::DirectoryFinished(dir) if dir.id == 1 && dir.direct_error_count == 1 && !dir.fully_read)));
    Ok(())
}

#[test]
fn deep_tree_on_small_stack() -> Result<(), Box<dyn Error>> {
    let mut path = std::path::PathBuf::new();
    for _ in 0..200 {
        path.push("d");
    }
    let fixture = FixtureBuilder::new()
        .add_file(path.join("leaf"), 1)
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        ..ScanOptions::default()
    };
    let stats = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || scan(&options, |_| {}))?
        .join()
        .map_err(|_| "scanner thread panicked")??;
    assert_eq!(stats.dir_count, 201);
    assert_eq!(stats.file_count, 1);
    Ok(())
}
