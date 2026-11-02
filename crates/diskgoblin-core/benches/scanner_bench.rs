//! Criterion benchmarks for the directory scanner thread scaling (`DG-012`).
//!
//! Three synthetic topologies are materialized once per group (outside the
//! timed closure) with [`FixtureBuilder`], then only [`scan_directory`] runs
//! inside `b.iter`. Each group reports throughput in entries per second, where
//! an entry is a file, directory, or symlink visited by traversal.
//!
//! Each group compares the sequential baseline against bounded pools with
//! 2, 4, and 8 workers. The CLI default is gated on 1.5x across the suite
//! (see `ADR-005`).

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use diskgoblin_core::{
    scan_directory,
    test_fixtures::{FixtureBuilder, TestFixture},
    ScanOptions,
};

/// Deterministic byte length of every fixture file (keeps fixtures cheap).
const FILE_SIZE_BYTES: u64 = 256;

/// `wide_tree`: 50 directories, each holding 200 files (10,000 files total).
const WIDE_DIR_COUNT: usize = 50;
const WIDE_FILES_PER_DIR: usize = 200;

/// `deep_tree`: 20 nested directories holding 2,500 files (125 per level).
const DEEP_LEVEL_COUNT: usize = 20;
const DEEP_TREE_FILES: usize = 2_500;
const DEEP_FILES_PER_LEVEL: usize = DEEP_TREE_FILES / DEEP_LEVEL_COUNT;

/// `hardlink_cache_tree`: 2,000 original files plus 3,000 hard links (60 %).
const HARDLINK_BASE_FILES: usize = 2_000;
const HARDLINK_LINK_FILES: usize = 3_000;

/// Symlinks added to each level of `deep_tree` (Unix only).
#[cfg(unix)]
const DEEP_SYMLINKS_PER_LEVEL: u64 = 1;
#[cfg(not(unix))]
const DEEP_SYMLINKS_PER_LEVEL: u64 = 0;

fn options_for(fixture: &TestFixture) -> ScanOptions {
    ScanOptions {
        root_path: fixture.path().to_path_buf(),
        ..ScanOptions::default()
    }
}

/// 50 sibling directories x 200 files: exercises directory fan-out with many
/// independent, moderately sized `read_dir` batches.
fn wide_tree_fixture() -> TestFixture {
    let mut builder = FixtureBuilder::new();

    for dir_index in 0..WIDE_DIR_COUNT {
        let dir_path = format!("dir{dir_index:02}");
        builder = builder.add_dir(dir_path.clone());
        for file_index in 0..WIDE_FILES_PER_DIR {
            builder = builder.add_file(
                format!("{dir_path}/file{file_index:03}.bin"),
                FILE_SIZE_BYTES,
            );
        }
    }

    builder.build().expect("wide_tree fixture builds")
}

fn bench_wide_tree(c: &mut Criterion) {
    let fixture = wide_tree_fixture();
    let mut options = options_for(&fixture);
    let total_entries = (WIDE_DIR_COUNT * (WIDE_FILES_PER_DIR + 1)) as u64;

    let mut group = c.benchmark_group("wide_tree");
    group.throughput(Throughput::Elements(total_entries));
    for threads in [1, 2, 4, 8] {
        options.threads = threads;
        group.bench_function(BenchmarkId::new("threads", threads), |b| {
            b.iter(|| {
                black_box(scan_directory(black_box(&options)).expect("wide_tree scan succeeds"))
            })
        });
    }
    group.finish();
}

/// Returns the relative path of the `level`-th nested directory.
fn nested_dir_path(level: usize) -> String {
    (0..=level)
        .map(|index| format!("level{index:02}"))
        .collect::<Vec<_>>()
        .join("/")
}

/// 20 levels of nested directories with files at every level, plus one symlink
/// per level: exercises deep recursion, path growth, and `lstat`-only handling
/// of links that point back at an ancestor.
fn deep_tree_fixture() -> (TestFixture, u64) {
    let mut builder = FixtureBuilder::new();

    for level in 0..DEEP_LEVEL_COUNT {
        let dir_path = nested_dir_path(level);
        builder = builder.add_dir(dir_path.clone());
        for file_index in 0..DEEP_FILES_PER_LEVEL {
            builder = builder.add_file(
                format!("{dir_path}/file{file_index:03}.bin"),
                FILE_SIZE_BYTES,
            );
        }

        #[cfg(unix)]
        {
            builder = builder.add_symlink("..", format!("{dir_path}/up_link"));
        }
    }

    let fixture = builder.build().expect("deep_tree fixture builds");
    let file_entries = DEEP_TREE_FILES as u64;
    let dir_entries = DEEP_LEVEL_COUNT as u64;
    let total_entries = file_entries + dir_entries + dir_entries * DEEP_SYMLINKS_PER_LEVEL;

    (fixture, total_entries)
}

fn bench_deep_tree(c: &mut Criterion) {
    let (fixture, total_entries) = deep_tree_fixture();
    let mut options = options_for(&fixture);

    let mut group = c.benchmark_group("deep_tree");
    group.throughput(Throughput::Elements(total_entries));
    for threads in [1, 2, 4, 8] {
        options.threads = threads;
        group.bench_function(BenchmarkId::new("threads", threads), |b| {
            b.iter(|| {
                black_box(scan_directory(black_box(&options)).expect("deep_tree scan succeeds"))
            })
        });
    }
    group.finish();
}

/// 2,000 original files plus 3,000 hard links onto them: exercises the
/// `(dev, ino)` deduplication `HashSet` that must stay warm across 5,000
/// entries, 60 % of which are duplicates.
fn hardlink_cache_tree_fixture() -> TestFixture {
    let mut builder = FixtureBuilder::new();

    for base_index in 0..HARDLINK_BASE_FILES {
        builder = builder.add_file(
            format!("originals/file{base_index:04}.bin"),
            FILE_SIZE_BYTES,
        );
    }

    for link_index in 0..HARDLINK_LINK_FILES {
        // Each link points at one of the originals; no modulo-zero branching.
        let base_index = link_index % HARDLINK_BASE_FILES;
        builder = builder.add_hard_link(
            format!("originals/file{base_index:04}.bin"),
            format!("links/link{link_index:04}.bin"),
        );
    }

    builder.build().expect("hardlink_cache_tree fixture builds")
}

fn bench_hardlink_cache_tree(c: &mut Criterion) {
    let fixture = hardlink_cache_tree_fixture();
    let mut options = options_for(&fixture);
    let total_entries = (HARDLINK_BASE_FILES + HARDLINK_LINK_FILES) as u64;

    let mut group = c.benchmark_group("hardlink_cache_tree");
    group.throughput(Throughput::Elements(total_entries));
    for threads in [1, 2, 4, 8] {
        options.threads = threads;
        group.bench_function(BenchmarkId::new("threads", threads), |b| {
            b.iter(|| {
                black_box(
                    scan_directory(black_box(&options)).expect("hardlink_cache_tree scan succeeds"),
                )
            })
        });
    }
    group.finish();
}

criterion_group!(
    scanner_benches,
    bench_wide_tree,
    bench_deep_tree,
    bench_hardlink_cache_tree
);
criterion_main!(scanner_benches);
