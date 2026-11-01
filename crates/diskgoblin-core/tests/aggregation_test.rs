use diskgoblin_core::{
    extract_metadata, scan_directory, test_fixtures::FixtureBuilder, FatalScanError, ScanOptions,
    SizeMetric,
};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn rollups_pruning_and_rankings() -> Result {
    let fixture = FixtureBuilder::new()
        .add_file("a/deep/deeper/data", 100)
        .add_file("b/data", 100)
        .add_file("small", 2)
        .add_dir("empty")
        .build()?;
    let mut options = ScanOptions {
        root_path: fixture.path().into(),
        size_metric: SizeMetric::Apparent,
        top_n: 2,
        min_size_bytes: 100,
        ..ScanOptions::default()
    };
    let full = scan_directory(&options)?;
    assert_eq!(full.root.summary.apparent_size, 202);
    assert_eq!(full.root.summary.file_count, 3);
    assert_eq!(full.root.summary.dir_count, 5);
    assert_eq!(full.root.summary.error_count, 0);
    let expected_allocated = ["a/deep/deeper/data", "b/data", "small"]
        .into_iter()
        .map(|p| {
            std::fs::symlink_metadata(fixture.path().join(p))
                .map(|m| extract_metadata(&m).allocated_size)
        })
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .sum::<u64>();
    assert_eq!(full.root.summary.allocated_size, expected_allocated);
    assert_eq!(
        full.top_files
            .iter()
            .map(|f| f.path.strip_prefix(fixture.path()).unwrap().to_path_buf())
            .collect::<Vec<_>>(),
        vec![
            std::path::PathBuf::from("a/deep/deeper/data"),
            "b/data".into()
        ]
    );
    assert_eq!(full.top_directories.len(), 2);
    assert_eq!(full.top_directories[0].path, fixture.path().join("a"));
    assert_eq!(full.top_directories[1].path, fixture.path().join("a/deep"));
    assert_eq!(
        full.root
            .children
            .iter()
            .map(|n| n.summary.path.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "b", "empty"]
    );
    for depth in [0, 1] {
        options.max_depth = Some(depth);
        let pruned = scan_directory(&options)?;
        assert_eq!(pruned.root.summary, full.root.summary);
        assert_eq!(pruned.top_directories, full.top_directories);
        if depth == 0 {
            assert!(pruned.root.children.is_empty());
        } else {
            assert!(pruned.root.children.iter().all(|c| c.children.is_empty()));
            assert_eq!(
                pruned.root.children[0].summary,
                full.root.children[0].summary
            );
        }
    }
    options.top_n = 0;
    let report = scan_directory(&options)?;
    assert!(report.top_files.is_empty() && report.top_directories.is_empty());
    options.top_n = 20;
    options.min_size_bytes = 101;
    let report = scan_directory(&options)?;
    assert!(report.top_files.is_empty() && report.top_directories.is_empty());
    Ok(())
}

#[cfg(unix)]
#[test]
fn hardlinks_count_once_but_all_paths_count() -> Result {
    let fixture = FixtureBuilder::new()
        .add_file("a/primary", 1024 * 1024)
        .add_hard_link("a/primary", "a/second")
        .add_hard_link("a/primary", "b/third")
        .add_file("b/unique", 8)
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        size_metric: SizeMetric::Apparent,
        ..ScanOptions::default()
    };
    let report = scan_directory(&options)?;
    assert_eq!(report.root.summary.apparent_size, 1024 * 1024 + 8);
    assert_eq!(report.root.summary.file_count, 4);
    assert_eq!(report.root.children[0].summary.file_count, 2);
    assert_eq!(report.root.children[1].summary.apparent_size, 8);
    assert_eq!(report.top_files.len(), 2);
    assert_eq!(report.top_files[0].path, fixture.path().join("a/primary"));
    assert!(report.top_files.iter().all(|f| !f.is_deduplicated_hardlink));
    assert_eq!(scan_directory(&options)?.top_files, report.top_files);
    let allocated = ["a/primary", "b/unique"]
        .into_iter()
        .map(|p| {
            std::fs::symlink_metadata(fixture.path().join(p))
                .map(|m| extract_metadata(&m).allocated_size)
        })
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .sum::<u64>();
    assert_eq!(report.root.summary.allocated_size, allocated);
    Ok(())
}

#[test]
fn empty_root_and_fatal_errors() -> Result {
    let fixture = FixtureBuilder::new().build()?;
    let mut options = ScanOptions {
        root_path: fixture.path().into(),
        ..ScanOptions::default()
    };
    let report = scan_directory(&options)?;
    assert_eq!(report.root.summary.apparent_size, 0);
    assert_eq!(report.root.summary.dir_count, 0);
    assert!(report.top_directories.is_empty() && report.top_files.is_empty());
    options.root_path.push("missing");
    assert!(matches!(
        scan_directory(&options),
        Err(FatalScanError::RootNotFound { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_inode_sizes_and_partial_errors() -> Result {
    let fixture = FixtureBuilder::new()
        .add_file("a/data", 10)
        .add_symlink("..", "a/loop")
        .add_symlink("missing", "dangling")
        .add_unreadable_dir("a/blocked")
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        size_metric: SizeMetric::Apparent,
        ..ScanOptions::default()
    };
    let report = scan_directory(&options)?;
    let links = ["a/loop", "dangling"]
        .into_iter()
        .map(|p| std::fs::symlink_metadata(fixture.path().join(p)).map(|m| m.len()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .sum::<u64>();
    assert_eq!(report.root.summary.apparent_size, 10 + links);
    assert_eq!(report.root.summary.symlink_count, 2);
    assert_eq!(report.root.summary.file_count, 1);
    if !diskgoblin_core::test_fixtures::is_root_user() {
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.root.summary.error_count, 1);
        assert_eq!(report.root.children[0].summary.error_count, 1);
        assert_eq!(report.root.children[0].children[0].summary.error_count, 1);
    }
    Ok(())
}

#[test]
fn allocated_rankings_use_metadata_and_resolve_ties() -> Result {
    let fixture = FixtureBuilder::new()
        .add_file("z/data", 100)
        .add_file("a/data", 100)
        .add_file("m/data", 200)
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        top_n: 2,
        ..ScanOptions::default()
    };
    let report = scan_directory(&options)?;
    let mut expected = ["z/data", "a/data", "m/data"]
        .into_iter()
        .map(|p| {
            let path = fixture.path().join(p);
            let raw = extract_metadata(&std::fs::symlink_metadata(&path)?);
            Ok((raw.allocated_size, path))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    expected.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    assert_eq!(
        report
            .top_files
            .iter()
            .map(|f| (f.allocated_size, f.path.clone()))
            .collect::<Vec<_>>(),
        expected[..2]
    );
    assert_eq!(
        report
            .top_directories
            .iter()
            .map(|d| d.path.clone())
            .collect::<Vec<_>>(),
        expected[..2]
            .iter()
            .map(|(_, p)| p.parent().unwrap().to_path_buf())
            .collect::<Vec<_>>()
    );
    Ok(())
}
