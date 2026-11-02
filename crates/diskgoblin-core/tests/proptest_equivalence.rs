//! Property-based equivalence and invariant tests for the DiskGoblin scanner.
//!
//! Every generated case describes a synthetic tree (nested directories, files of
//! arbitrary lengths, hard links to those files, and Unix symlinks), materializes
//! it through [`FixtureBuilder`], and then scans it twice: sequentially
//! (`threads = 1`, the specification oracle) and in parallel (`threads = 4`).
//!
//! Four invariants are checked on every tree:
//!
//! 1. **Sequential == Parallel Equivalence:** both reports are identical in
//!    `root`, `top_directories`, `top_files`, and `errors` once `duration_ms`
//!    (wall-clock noise) is normalized.
//! 2. **Conservation of Bytes:** the root's `apparent_size` equals the exact sum
//!    of unique file bytes plus symlink bytes (a symlink's apparent size is the
//!    byte length of its target string on Unix; hard links never double count).
//! 3. **Monotonic Ancestor Invariant:** every directory's `apparent_size` and
//!    `allocated_size` are at least the sum of its child directories' sizes.
//! 4. **Top-N Bound & Ordering:** both rankings stay within `top_n`, respect
//!    `min_size_bytes`, and are strictly ordered by `(metric_size DESC, path ASC)`.
//!
//! Sizes are deliberately modest so the whole suite materializes 32 real
//! temporary trees in well under five seconds.

use diskgoblin_core::{
    scan_directory,
    test_fixtures::{FixtureBuilder, TestFixture},
    DirNode, DirSummary, FileEntry, ScanOptions, ScanReport, SizeMetric,
};
use proptest::{prelude::*, test_runner::TestCaseResult};
use std::path::{Path, PathBuf};

/// Thread count for the parallel half of every equivalence check.
const PARALLEL_THREADS: usize = 4;

/// Upper bound on a single generated file's length. Small files keep fixture
/// creation fast while still exercising real allocation and hard-link metadata.
const MAX_FILE_BYTES: u64 = 16_384;

/// Relative symlink targets. Relative targets never escape the fixture root,
/// and their stored length is exactly what the scanner counts as apparent size.
const SYMLINK_TARGETS: [&str; 4] = ["..", ".", "missing", "f00"];

/// A declarative description of one synthetic directory tree.
///
/// Directory `i` lives under `dirs[i]`; `None` means "directly under the scan
/// root". Because every parent index is strictly less than its child index, the
/// descriptor can never describe a cycle or a directory that is its own ancestor.
#[derive(Debug, Clone)]
struct TreeSpec {
    dirs: Vec<Option<usize>>,
    files: Vec<FileSpec>,
    hard_links: Vec<HardLinkSpec>,
    symlinks: Vec<SymlinkSpec>,
}

#[derive(Debug, Clone)]
struct FileSpec {
    dir: Option<usize>,
    size_bytes: u64,
}

#[derive(Debug, Clone)]
struct HardLinkSpec {
    /// Index into [`TreeSpec::files`] of the file this link points at.
    file: usize,
    dir: Option<usize>,
}

#[derive(Debug, Clone)]
struct SymlinkSpec {
    target: &'static str,
    dir: Option<usize>,
}

/// Generates a random tree descriptor with at most 12 directories, 24 files,
/// 6 hard links, and 4 symlinks.
fn tree_spec() -> impl Strategy<Value = TreeSpec> {
    // One selector byte per directory decides its parent: any pick below the
    // directory's own index is an already-built ancestor, anything at or above
    // it places the directory directly under the scan root. Nesting therefore
    // stays bounded and acyclic by construction.
    prop::collection::vec(any::<u8>(), 1..=12).prop_flat_map(|parent_selectors| {
        let dir_count = parent_selectors.len();
        let container = 0u16..=dir_count as u16;
        (
            prop::collection::vec((0u64..=MAX_FILE_BYTES, container.clone()), 0..=24),
            prop::collection::vec((any::<u32>(), container.clone()), 0..=6),
            prop::collection::vec((0u16..SYMLINK_TARGETS.len() as u16, container), 0..=4),
        )
            .prop_map(move |(files, links, symlinks)| {
                let dirs = parent_selectors
                    .iter()
                    .enumerate()
                    .map(|(index, selector)| {
                        // `% (index + 2)` bounds the pick to `0..=index + 1`.
                        let choice = usize::from(*selector) % (index + 2);
                        if choice >= index {
                            None
                        } else {
                            Some(choice)
                        }
                    })
                    .collect::<Vec<_>>();

                let files = files
                    .into_iter()
                    .map(|(size_bytes, container)| FileSpec {
                        dir: resolve_container(container),
                        size_bytes,
                    })
                    .collect::<Vec<_>>();

                // Hard links can only point at a file that the strategy actually
                // generated, so an empty file list yields no hard links.
                let hard_links = if files.is_empty() {
                    Vec::new()
                } else {
                    links
                        .into_iter()
                        .map(|(selector, container)| HardLinkSpec {
                            file: selector as usize % files.len(),
                            dir: resolve_container(container),
                        })
                        .collect::<Vec<_>>()
                };

                let symlinks = symlinks
                    .into_iter()
                    .map(|(selector, container)| SymlinkSpec {
                        target: SYMLINK_TARGETS[usize::from(selector)],
                        dir: resolve_container(container),
                    })
                    .collect::<Vec<_>>();

                TreeSpec {
                    dirs,
                    files,
                    hard_links,
                    symlinks,
                }
            })
    })
}

/// Maps a strategy selector to a container: `0` is the scan root itself,
/// `n` is directory `n - 1`.
fn resolve_container(selector: u16) -> Option<usize> {
    if selector == 0 {
        None
    } else {
        Some(usize::from(selector) - 1)
    }
}

/// Resolves every directory to its path relative to the fixture root.
fn dir_paths(spec: &TreeSpec) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::with_capacity(spec.dirs.len());
    for (index, parent) in spec.dirs.iter().enumerate() {
        let name = format!("d{index:02}");
        let path = match parent {
            Some(parent) => paths[*parent].join(name),
            None => PathBuf::from(name),
        };
        paths.push(path);
    }
    paths
}

/// Resolves a container selector against the already-built directory paths.
fn container_path(paths: &[PathBuf], dir: Option<usize>) -> &Path {
    match dir {
        Some(index) => paths[index].as_path(),
        None => Path::new(""),
    }
}

/// Materializes the descriptor on disk inside a fresh temporary directory.
fn build_fixture(spec: &TreeSpec) -> std::io::Result<TestFixture> {
    let dirs = dir_paths(spec);
    let mut builder = FixtureBuilder::new();

    for path in &dirs {
        builder = builder.add_dir(path);
    }

    let file_paths = spec
        .files
        .iter()
        .enumerate()
        .map(|(index, file)| container_path(&dirs, file.dir).join(format!("f{index:02}")))
        .collect::<Vec<_>>();

    for (path, file) in file_paths.iter().zip(&spec.files) {
        builder = builder.add_file(path, file.size_bytes);
    }

    for (index, link) in spec.hard_links.iter().enumerate() {
        builder = builder.add_hard_link(
            &file_paths[link.file],
            container_path(&dirs, link.dir).join(format!("h{index:02}")),
        );
    }

    #[cfg(unix)]
    for (index, link) in spec.symlinks.iter().enumerate() {
        builder = builder.add_symlink(
            link.target,
            container_path(&dirs, link.dir).join(format!("s{index:02}")),
        );
    }

    builder.build()
}

/// The exact number of apparent bytes a correct scan must report.
///
/// Every `add_file` creates a distinct inode, so unique file bytes are simply
/// the sum of their lengths; hard links add paths but never bytes. On Unix a
/// symlink contributes the byte length of its stored target string; elsewhere
/// symlinks are not materialized and hard links report `nlink == 1`, so each
/// link contributes its own bytes.
fn expected_apparent_bytes(spec: &TreeSpec) -> u64 {
    let unique_files = spec.files.iter().map(|file| file.size_bytes).sum::<u64>();

    #[cfg(unix)]
    {
        let symlink_bytes = spec
            .symlinks
            .iter()
            .map(|link| link.target.len() as u64)
            .sum::<u64>();
        unique_files + symlink_bytes
    }
    #[cfg(not(unix))]
    {
        let link_bytes = spec
            .hard_links
            .iter()
            .map(|link| spec.files[link.file].size_bytes)
            .sum::<u64>();
        unique_files + link_bytes
    }
}

/// Selects the ranking metric the way the scanner does.
fn metric_size(metric: SizeMetric, apparent: u64, allocated: u64) -> u64 {
    metric.select(apparent, allocated)
}

/// Invariant 3: a directory is at least as large as the sum of its children.
fn assert_monotonic_ancestors(node: &DirNode) -> TestCaseResult {
    let child_apparent = node
        .children
        .iter()
        .map(|child| child.summary.apparent_size)
        .sum::<u64>();
    let child_allocated = node
        .children
        .iter()
        .map(|child| child.summary.allocated_size)
        .sum::<u64>();

    prop_assert!(
        node.summary.apparent_size >= child_apparent,
        "directory {} reports {} apparent bytes but its children total {}",
        node.summary.path.display(),
        node.summary.apparent_size,
        child_apparent,
    );
    prop_assert!(
        node.summary.allocated_size >= child_allocated,
        "directory {} reports {} allocated bytes but its children total {}",
        node.summary.path.display(),
        node.summary.allocated_size,
        child_allocated,
    );

    for child in &node.children {
        assert_monotonic_ancestors(child)?;
    }

    Ok(())
}

/// Invariant 4 for file rankings: bounded, filtered, and strictly ordered.
fn assert_ranked_files(files: &[FileEntry], options: &ScanOptions) -> TestCaseResult {
    prop_assert!(
        files.len() <= options.top_n,
        "top_files holds {} entries but top_n is {}",
        files.len(),
        options.top_n,
    );

    for file in files {
        let size = metric_size(options.size_metric, file.apparent_size, file.allocated_size);
        prop_assert!(
            size >= options.min_size_bytes,
            "top file {} reports {} bytes below min_size_bytes {}",
            file.path.display(),
            size,
            options.min_size_bytes,
        );
        prop_assert!(
            !file.is_deduplicated_hardlink,
            "top file {} is a deduplicated hard link",
            file.path.display(),
        );
    }

    for pair in files.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let left_size = metric_size(options.size_metric, left.apparent_size, left.allocated_size);
        let right_size = metric_size(
            options.size_metric,
            right.apparent_size,
            right.allocated_size,
        );
        prop_assert!(
            left_size > right_size || (left_size == right_size && left.path < right.path),
            "top_files is not strictly ordered: {} ({} bytes) precedes {} ({} bytes)",
            left.path.display(),
            left_size,
            right.path.display(),
            right_size,
        );
    }

    Ok(())
}

/// Invariant 4 for directory rankings: bounded, filtered, and strictly ordered.
fn assert_ranked_directories(dirs: &[DirSummary], options: &ScanOptions) -> TestCaseResult {
    prop_assert!(
        dirs.len() <= options.top_n,
        "top_directories holds {} entries but top_n is {}",
        dirs.len(),
        options.top_n,
    );

    for dir in dirs {
        let size = metric_size(options.size_metric, dir.apparent_size, dir.allocated_size);
        prop_assert!(
            size >= options.min_size_bytes,
            "top directory {} reports {} bytes below min_size_bytes {}",
            dir.path.display(),
            size,
            options.min_size_bytes,
        );
    }

    for pair in dirs.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let left_size = metric_size(options.size_metric, left.apparent_size, left.allocated_size);
        let right_size = metric_size(
            options.size_metric,
            right.apparent_size,
            right.allocated_size,
        );
        prop_assert!(
            left_size > right_size || (left_size == right_size && left.path < right.path),
            "top_directories is not strictly ordered: {} ({} bytes) precedes {} ({} bytes)",
            left.path.display(),
            left_size,
            right.path.display(),
            right_size,
        );
    }

    Ok(())
}

/// Invariant 1: both engines agree on everything except elapsed wall time.
fn assert_reports_equivalent(sequential: ScanReport, parallel: ScanReport) -> TestCaseResult {
    let normalized_sequential = ScanReport {
        duration_ms: 0,
        ..sequential
    };
    let normalized_parallel = ScanReport {
        duration_ms: 0,
        ..parallel
    };

    prop_assert_eq!(&normalized_sequential.root, &normalized_parallel.root);
    prop_assert_eq!(
        &normalized_sequential.top_directories,
        &normalized_parallel.top_directories
    );
    prop_assert_eq!(
        &normalized_sequential.top_files,
        &normalized_parallel.top_files
    );
    prop_assert_eq!(&normalized_sequential.errors, &normalized_parallel.errors);
    prop_assert_eq!(
        &normalized_sequential.root_path,
        &normalized_parallel.root_path
    );
    prop_assert_eq!(
        normalized_sequential.size_metric,
        normalized_parallel.size_metric
    );

    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// The full invariant battery over one generated tree.
    #[test]
    fn generated_trees_preserve_scan_invariants(
        spec in tree_spec(),
        top_n in 0u16..=6,
        min_size_bytes in 0u64..=2_048,
    ) {
        let fixture = build_fixture(&spec).expect("synthetic fixture materializes");
        let options = ScanOptions {
            root_path: fixture.path().to_path_buf(),
            size_metric: SizeMetric::Apparent,
            top_n: usize::from(top_n),
            min_size_bytes,
            ..ScanOptions::default()
        };

        let sequential = scan_directory(&options).expect("sequential scan succeeds");
        let parallel = scan_directory(&ScanOptions {
            threads: PARALLEL_THREADS,
            ..options.clone()
        })
        .expect("parallel scan succeeds");

        // Invariant 2: conservation of bytes across the unique inodes on disk.
        prop_assert_eq!(
            sequential.root.summary.apparent_size,
            expected_apparent_bytes(&spec),
            "root apparent size disagrees with the generated tree {:?}",
            spec,
        );

        // Invariant 3: ancestors never shrink below their children.
        assert_monotonic_ancestors(&sequential.root)?;

        // Invariant 4: both rankings are bounded, filtered, and ordered.
        assert_ranked_files(&sequential.top_files, &options)?;
        assert_ranked_directories(&sequential.top_directories, &options)?;

        // Invariant 1: the sequential oracle and the parallel engine agree.
        assert_reports_equivalent(sequential, parallel)?;
    }
}
