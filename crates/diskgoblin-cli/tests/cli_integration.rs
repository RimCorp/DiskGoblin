//! End-to-end black-box tests for the compiled `diskgoblin` binary (DG-010).
//!
//! Every scenario runs the real binary in a subprocess against synthetic
//! [`FixtureBuilder`] trees and asserts Milestone M1 behaviour: stdout/stderr
//! separation, JSON schema fidelity, flag semantics, and the `0`/`1`/`2` exit
//! codes.

use assert_cmd::Command;
#[cfg(unix)]
use diskgoblin_core::test_fixtures::is_root_user;
use diskgoblin_core::test_fixtures::{FixtureBuilder, TestFixture};
use diskgoblin_core::{DirNode, ScanErrorKind, ScanReport, SizeMetric, REPORT_SCHEMA_VERSION};
use std::path::Path;
use std::process::Output;

/// Apparent byte size of `alpha/data.bin`.
const DATA_BIN_BYTES: u64 = 5_000;
/// Apparent byte size of `alpha/nested/deep.txt`.
const DEEP_TXT_BYTES: u64 = 100;
/// Apparent byte size of `beta/shared.bin`, counted once despite its hard link.
const SHARED_BIN_BYTES: u64 = 2_048;
/// Directory entries in the baseline fixture, excluding the root itself.
const BASELINE_DIRS: u64 = 4;
/// File entries in the baseline fixture, including the deduplicated hard link.
const BASELINE_FILES: u64 = 4;

/// Symlinks in the baseline fixture (`link-to-alpha`, `dangling`).
#[cfg(unix)]
const BASELINE_SYMLINKS: u64 = 2;
/// Symlinks are a Unix-only fixture entry.
#[cfg(not(unix))]
const BASELINE_SYMLINKS: u64 = 0;

/// Apparent bytes contributed by symlink inodes: targets `alpha` (5) + `missing` (7).
#[cfg(unix)]
const SYMLINK_TARGET_BYTES: u64 = 12;
/// Symlinks contribute no bytes where they cannot be created.
#[cfg(not(unix))]
const SYMLINK_TARGET_BYTES: u64 = 0;

/// Total apparent bytes of the baseline fixture, with the hard link counted once.
const BASELINE_APPARENT_BYTES: u64 =
    DATA_BIN_BYTES + DEEP_TXT_BYTES + SHARED_BIN_BYTES + SYMLINK_TARGET_BYTES;

/// Builds a subprocess invocation of the compiled `diskgoblin` binary.
///
/// `Command::cargo_bin` is deprecated in newer `assert_cmd` 2.x releases in
/// favour of the `cargo_bin_cmd!` macro, which older minors do not provide, so
/// the deprecation is allowed here to keep `-D warnings` green on both.
#[allow(deprecated)]
fn diskgoblin() -> Command {
    Command::cargo_bin("diskgoblin").expect("the diskgoblin binary should be built for tests")
}

/// A nested tree with hard links, symlinks, and an empty directory.
///
/// Traversal visits `beta/` in filename order, so `shared-link.bin` is counted
/// first and `shared.bin` becomes the deduplicated member of the inode pair.
fn baseline_fixture() -> TestFixture {
    let builder = FixtureBuilder::new()
        .add_file("alpha/data.bin", DATA_BIN_BYTES)
        .add_file("alpha/nested/deep.txt", DEEP_TXT_BYTES)
        .add_file("beta/shared.bin", SHARED_BIN_BYTES)
        .add_hard_link("beta/shared.bin", "beta/shared-link.bin")
        .add_dir("empty");
    #[cfg(unix)]
    let builder = builder
        .add_symlink("alpha", "link-to-alpha")
        .add_symlink("missing", "dangling");
    builder.build().expect("the baseline fixture should build")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be valid UTF-8")
}

/// Parses `stdout` as the single JSON document the CLI is allowed to emit.
///
/// Asserts that the only bytes outside the document are the CLI's trailing
/// newline, so progress or diagnostics can never leak into the data stream.
fn parse_json_report(output: &Output) -> ScanReport {
    let stdout = &output.stdout;
    assert_eq!(
        stdout.iter().filter(|byte| **byte == b'\n').count(),
        1,
        "compact JSON output should carry exactly one line feed: {}",
        String::from_utf8_lossy(stdout)
    );
    let document = &stdout[..stdout.len() - 1];
    let report: ScanReport =
        serde_json::from_slice(document).expect("stdout should be a valid ScanReport document");
    assert_eq!(
        serde_json::to_vec(&report).expect("paths should re-serialize"),
        document,
        "stdout must hold the JSON document and nothing else"
    );
    report
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn top_file_names(report: &ScanReport, root: &Path) -> Vec<String> {
    report
        .top_files
        .iter()
        .map(|file| relative(root, &file.path))
        .collect()
}

fn top_directory_names(report: &ScanReport, root: &Path) -> Vec<String> {
    report
        .top_directories
        .iter()
        .map(|dir| relative(root, &dir.path))
        .collect()
}

fn collect_directories(node: &DirNode, root: &Path, out: &mut Vec<(usize, String)>) {
    out.push((node.summary.depth, relative(root, &node.summary.path)));
    for child in &node.children {
        collect_directories(child, root, out);
    }
}

#[test]
fn default_human_output_lists_nested_fixture_contents() {
    let fixture = baseline_fixture();
    let assert = diskgoblin().arg(fixture.path()).assert().code(0);
    let output = assert.get_output();
    let stdout = stdout_of(output);
    assert!(
        stderr_of_is_empty(output),
        "non-interactive runs should keep stderr quiet: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        stdout.starts_with(&format!("Root: {}", fixture.path().display())),
        "missing root header in:\n{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "Files: {BASELINE_FILES} | Dirs: {BASELINE_DIRS} | Symlinks: {BASELINE_SYMLINKS} | Duration: "
        )),
        "missing summary counters in:\n{stdout}"
    );
    assert!(stdout.contains("Errors: 0 inaccessible paths"));
    assert!(stdout.contains("Top Directories (Allocated)"));
    assert!(stdout.contains("Top Files (Allocated)"));

    for directory in ["alpha", "alpha/nested", "beta", "empty"] {
        let expected = fixture.path().join(directory).display().to_string();
        assert!(
            stdout.contains(&expected),
            "directory {directory} missing from:\n{stdout}"
        );
    }
    for file in [
        "alpha/data.bin",
        "alpha/nested/deep.txt",
        "beta/shared-link.bin",
    ] {
        let expected = fixture.path().join(file).display().to_string();
        assert!(
            stdout.contains(&expected),
            "file {file} missing from:\n{stdout}"
        );
    }

    // The hard link sharing `shared.bin`'s inode is never ranked twice.
    let deduplicated = fixture.path().join("beta/shared.bin").display().to_string();
    assert!(!stdout.contains(&deduplicated));
}

#[test]
fn json_output_is_a_single_parseable_document() {
    let fixture = baseline_fixture();
    let assert = diskgoblin()
        .arg(fixture.path())
        .arg("--json")
        .assert()
        .code(0);
    let output = assert.get_output();
    assert!(
        stderr_of_is_empty(output),
        "JSON runs should keep stderr quiet: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report = parse_json_report(output);
    assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
    assert_eq!(report.size_metric, SizeMetric::Allocated);
    assert_eq!(report.root_path.as_path(), fixture.path());

    let summary = &report.root.summary;
    assert_eq!(summary.path.as_path(), fixture.path());
    assert_eq!(summary.depth, 0);
    assert_eq!(summary.file_count, BASELINE_FILES);
    assert_eq!(summary.dir_count, BASELINE_DIRS);
    assert_eq!(summary.symlink_count, BASELINE_SYMLINKS);
    assert_eq!(summary.error_count, 0);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    // Apparent bytes count the hard-linked inode exactly once.
    assert_eq!(summary.apparent_size, BASELINE_APPARENT_BYTES);

    let mut directories = top_directory_names(&report, fixture.path());
    directories.sort();
    assert_eq!(directories, ["alpha", "alpha/nested", "beta", "empty"]);

    let mut files = top_file_names(&report, fixture.path());
    files.sort();
    assert_eq!(
        files,
        [
            "alpha/data.bin",
            "alpha/nested/deep.txt",
            "beta/shared-link.bin"
        ]
    );
    assert!(
        report
            .top_files
            .iter()
            .all(|file| !file.is_deduplicated_hardlink),
        "deduplicated hard links should not enter the rankings"
    );
}

#[test]
fn apparent_size_selects_logical_bytes_and_ordering() {
    let fixture = baseline_fixture();
    let assert = diskgoblin()
        .arg(fixture.path())
        .args(["--apparent-size", "--json"])
        .assert()
        .code(0);
    let report = parse_json_report(assert.get_output());
    assert_eq!(report.size_metric, SizeMetric::Apparent);
    assert_eq!(report.root.summary.apparent_size, BASELINE_APPARENT_BYTES);
    assert_eq!(
        top_file_names(&report, fixture.path()),
        [
            "alpha/data.bin",
            "beta/shared-link.bin",
            "alpha/nested/deep.txt"
        ]
    );

    let assert = diskgoblin()
        .arg(fixture.path())
        .arg("--apparent-size")
        .assert()
        .code(0);
    let stdout = stdout_of(assert.get_output());
    assert!(stdout.contains("Top Directories (Apparent)"), "{stdout}");
    assert!(stdout.contains("Top Files (Apparent)"), "{stdout}");
    assert!(stdout.contains("Apparent: 7.0 KiB"), "{stdout}");
}

#[test]
fn si_units_render_decimal_labels() {
    let fixture = baseline_fixture();
    let assert = diskgoblin()
        .arg(fixture.path())
        .args(["--apparent-size", "--si"])
        .assert()
        .code(0);
    let stdout = stdout_of(assert.get_output());
    assert!(stdout.contains("Apparent: 7.2 KB"), "{stdout}");
    assert!(
        !stdout.contains("KiB"),
        "SI runs must not print IEC units:\n{stdout}"
    );
}

#[test]
fn top_limit_bounds_both_rankings() {
    let fixture = baseline_fixture();
    let assert = diskgoblin()
        .arg(fixture.path())
        .args(["--apparent-size", "--top", "2", "--json"])
        .assert()
        .code(0);
    let report = parse_json_report(assert.get_output());
    assert_eq!(report.top_directories.len(), 2);
    assert_eq!(report.top_files.len(), 2);
    assert_eq!(
        top_directory_names(&report, fixture.path()),
        ["alpha", "beta"]
    );
    assert_eq!(
        top_file_names(&report, fixture.path()),
        ["alpha/data.bin", "beta/shared-link.bin"]
    );

    let assert = diskgoblin()
        .arg(fixture.path())
        .args(["--apparent-size", "--top", "2"])
        .assert()
        .code(0);
    let stdout = stdout_of(assert.get_output());
    for ranked in ["beta", "alpha/data.bin", "beta/shared-link.bin"] {
        let expected = fixture.path().join(ranked).display().to_string();
        assert!(stdout.contains(&expected), "{ranked} missing:\n{stdout}");
    }
    for omitted in ["empty", "alpha/nested/deep.txt"] {
        let unexpected = fixture.path().join(omitted).display().to_string();
        assert!(
            !stdout.contains(&unexpected),
            "{omitted} should be cut:\n{stdout}"
        );
    }
}

#[test]
fn max_depth_prunes_result_tree_but_keeps_totals() {
    let fixture = baseline_fixture();
    let assert = diskgoblin()
        .arg(fixture.path())
        .args(["--apparent-size", "--max-depth", "1", "--json"])
        .assert()
        .code(0);
    let report = parse_json_report(assert.get_output());

    let mut nodes = Vec::new();
    collect_directories(&report.root, fixture.path(), &mut nodes);
    assert!(
        nodes.iter().all(|(depth, _)| *depth <= 1),
        "pruned tree kept deeper nodes: {nodes:?}"
    );
    let mut names: Vec<&str> = nodes.iter().map(|(_, name)| name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["", "alpha", "beta", "empty"]);

    // Totals and rankings still cover every scanned depth.
    assert_eq!(report.root.summary.file_count, BASELINE_FILES);
    assert_eq!(report.root.summary.dir_count, BASELINE_DIRS);
    assert_eq!(report.root.summary.apparent_size, BASELINE_APPARENT_BYTES);
    assert_eq!(report.top_directories.len(), BASELINE_DIRS as usize);
}

#[test]
fn min_size_filters_small_entries_from_rankings() {
    let fixture = baseline_fixture();
    let assert = diskgoblin()
        .arg(fixture.path())
        .args(["--apparent-size", "--min-size", "1000", "--json"])
        .assert()
        .code(0);
    let report = parse_json_report(assert.get_output());
    assert_eq!(
        top_directory_names(&report, fixture.path()),
        ["alpha", "beta"]
    );
    assert_eq!(
        top_file_names(&report, fixture.path()),
        ["alpha/data.bin", "beta/shared-link.bin"]
    );
    // Filtering only affects rankings, never the reported totals.
    assert_eq!(report.root.summary.apparent_size, BASELINE_APPARENT_BYTES);
}

#[cfg(unix)]
#[test]
fn unreadable_child_directory_is_reported_without_failing_the_scan() {
    if is_root_user() {
        // Root bypasses mode bits, so restricted fixtures prove nothing.
        return;
    }
    // `add_unreadable_dir` registers the path, and `TestFixture::drop` restores
    // mode 0o700 before the temporary tree is removed.
    let fixture = FixtureBuilder::new()
        .add_file("readable/data.bin", 64)
        .add_unreadable_dir("restricted")
        .build()
        .expect("the restricted fixture should build");
    let restricted = fixture.path().join("restricted");
    let restricted_text = restricted.display().to_string();

    let assert = diskgoblin().arg(fixture.path()).assert().code(0);
    let stdout = stdout_of(assert.get_output());
    assert!(stdout.contains("Errors: 1 inaccessible paths"), "{stdout}");
    assert!(
        !stdout.contains(&format!("[!] {restricted_text}: ")),
        "per-path diagnostics should stay hidden without --show-errors:\n{stdout}"
    );

    let assert = diskgoblin()
        .arg(fixture.path())
        .arg("--show-errors")
        .assert()
        .code(0);
    let stdout = stdout_of(assert.get_output());
    assert!(stdout.contains("Errors: 1 inaccessible paths"), "{stdout}");
    assert!(
        stdout.contains(&format!("[!] {restricted_text}: PermissionDenied: ")),
        "--show-errors should name the restricted path:\n{stdout}"
    );

    let assert = diskgoblin()
        .arg(fixture.path())
        .arg("--json")
        .assert()
        .code(0);
    let report = parse_json_report(assert.get_output());
    assert_eq!(report.root.summary.error_count, 1);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].path, restricted);
    assert_eq!(report.errors[0].kind, ScanErrorKind::PermissionDenied);
    // The readable sibling was still scanned.
    assert_eq!(report.root.summary.file_count, 1);
}

#[test]
fn nonexistent_root_exits_one_with_diagnostic_on_stderr() {
    let fixture = FixtureBuilder::new().build().expect("fixture should build");
    let missing = fixture.path().join("does-not-exist");

    let assert = diskgoblin()
        .arg(&missing)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(format!(
            "root path does not exist: {}",
            missing.display()
        )));
    let output = assert.get_output();
    assert!(
        output.stdout.is_empty(),
        "fatal errors must not write to stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn invalid_flag_exits_two_with_usage_error() {
    let assert = diskgoblin()
        .arg("--definitely-not-a-flag")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--definitely-not-a-flag"));
    let output = assert.get_output();
    assert!(
        output.stdout.is_empty(),
        "usage errors must not write to stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn stderr_of_is_empty(output: &Output) -> bool {
    output.stderr.is_empty()
}
