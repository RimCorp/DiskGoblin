//! Integration tests for the deterministic synthetic fixture builder (DG-003).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use diskgoblin_core::test_fixtures::FixtureBuilder;

#[cfg(unix)]
use diskgoblin_core::test_fixtures::is_root_user;

#[test]
fn builds_nested_directories_and_exact_file_sizes() {
    let fixture = FixtureBuilder::new()
        .add_dir("a/b/c")
        .add_file("a/b/c/one.bin", 1024)
        .add_file("a/b/c/two.bin", 65_536)
        .add_file("a/empty.bin", 0)
        .add_file("root.bin", 7)
        .build()
        .expect("fixture should build");

    let root = fixture.path();
    assert!(root.is_dir());
    assert!(root.join("a").is_dir());
    assert!(root.join("a/b").is_dir());
    assert!(root.join("a/b/c").is_dir());

    assert_eq!(file_len(&root.join("a/b/c/one.bin")), 1024);
    assert_eq!(file_len(&root.join("a/b/c/two.bin")), 65_536);
    assert_eq!(file_len(&root.join("a/empty.bin")), 0);
    assert_eq!(file_len(&root.join("root.bin")), 7);
}

#[test]
fn file_contents_are_deterministic() {
    let fixture = FixtureBuilder::new()
        .add_file("small.bin", 1024)
        .add_file("large.bin", 65_536)
        .build()
        .expect("fixture should build");

    for (name, expected_len) in [("small.bin", 1024), ("large.bin", 65_536)] {
        let bytes = fs::read(fixture.path().join(name)).expect("fixture file should be readable");
        assert_eq!(bytes.len(), expected_len);
        assert!(
            bytes
                .iter()
                .enumerate()
                .all(|(offset, byte)| *byte == (offset % 256) as u8),
            "{name} does not contain the documented deterministic pattern"
        );
    }
}

#[cfg(unix)]
#[test]
fn hard_links_share_the_same_inode() {
    use std::os::unix::fs::MetadataExt;

    let fixture = FixtureBuilder::new()
        .add_dir("dir1")
        .add_dir("dir2")
        .add_file("dir1/original.bin", 1_048_576)
        .add_hard_link("dir1/original.bin", "dir1/link2.bin")
        .add_hard_link("dir1/original.bin", "dir2/link3.bin")
        .build()
        .expect("fixture should build");

    let original = fs::symlink_metadata(fixture.path().join("dir1/original.bin"))
        .expect("original file metadata");
    let link2 =
        fs::symlink_metadata(fixture.path().join("dir1/link2.bin")).expect("second link metadata");
    let link3 =
        fs::symlink_metadata(fixture.path().join("dir2/link3.bin")).expect("third link metadata");

    assert_eq!(original.ino(), link2.ino());
    assert_eq!(original.ino(), link3.ino());
    assert_eq!(original.dev(), link3.dev());
    assert_eq!(original.len(), 1_048_576);
    assert_eq!(link3.nlink(), 3);
}

#[test]
fn hard_link_without_existing_source_fails_to_build() {
    let result = FixtureBuilder::new()
        .add_hard_link("missing.bin", "link.bin")
        .build();

    assert!(result.is_err(), "hard link source must already exist");
}

#[cfg(unix)]
#[test]
fn symlinks_cover_valid_ancestor_loop_and_dangling_targets() {
    let fixture = FixtureBuilder::new()
        .add_dir("a/b")
        .add_file("a/b/data.bin", 16)
        .add_symlink("b/data.bin", "a/valid.bin")
        .add_symlink("../..", "a/b/loop")
        .add_symlink("nonexistent", "a/dangling")
        .build()
        .expect("fixture should build");

    let root = fixture.path();

    let valid = root.join("a/valid.bin");
    assert!(fs::symlink_metadata(&valid)
        .expect("valid symlink metadata")
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_link(&valid).expect("valid symlink target"),
        PathBuf::from("b/data.bin")
    );
    assert_eq!(file_len(&valid), 16);

    let ancestor_loop = root.join("a/b/loop");
    assert_eq!(
        fs::read_link(&ancestor_loop).expect("loop symlink target"),
        PathBuf::from("../..")
    );
    assert!(fs::metadata(&ancestor_loop)
        .expect("loop target should resolve")
        .is_dir());

    let dangling = root.join("a/dangling");
    assert_eq!(
        fs::read_link(&dangling).expect("dangling symlink target"),
        PathBuf::from("nonexistent")
    );
    assert!(fs::symlink_metadata(&dangling).is_ok());
    assert!(fs::metadata(&dangling).is_err());
}

#[cfg(unix)]
#[test]
fn unreadable_directory_is_restricted() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = FixtureBuilder::new()
        .add_dir("restricted")
        .add_file("restricted/secret.bin", 8)
        .add_unreadable_dir("restricted")
        .build()
        .expect("fixture should build");

    let restricted = fixture.path().join("restricted");
    let mode = fs::metadata(&restricted)
        .expect("restricted directory metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o000);

    if !is_root_user() {
        let error = fs::read_dir(&restricted).expect_err("unreadable directory must deny reads");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}

#[test]
fn dropping_fixture_removes_the_temporary_tree() {
    let fixture = FixtureBuilder::new()
        .add_dir("a/b")
        .add_file("a/b/data.bin", 128)
        .build()
        .expect("fixture should build");

    let root = fixture.path().to_path_buf();
    assert!(root.exists());

    drop(fixture);

    assert!(!root.exists(), "fixture root should be deleted on drop");
}

#[cfg(unix)]
#[test]
fn dropping_fixture_removes_unreadable_directories() {
    let fixture = FixtureBuilder::new()
        .add_dir("locked")
        .add_file("locked/inside.bin", 4)
        .add_unreadable_dir("locked")
        .build()
        .expect("fixture should build");

    let root = fixture.path().to_path_buf();
    assert!(root.join("locked").exists());

    drop(fixture);

    assert!(
        !root.exists(),
        "fixture root should be deleted after permissions are restored on drop"
    );
}

#[test]
fn rejects_paths_that_escape_the_fixture_root() {
    for bad_path in [
        "../escape",
        "a/../../escape",
        "/absolute/path",
        "a/../b",
        "./relative",
    ] {
        let error = FixtureBuilder::new()
            .add_dir(bad_path)
            .build()
            .expect_err("escaping path should be rejected");
        assert_eq!(
            error.kind(),
            io::ErrorKind::InvalidInput,
            "path should be rejected: {bad_path}"
        );
    }

    let error = FixtureBuilder::new()
        .add_file("", 1)
        .build()
        .expect_err("empty path should be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn rejects_escaping_hard_link_paths() {
    let error = FixtureBuilder::new()
        .add_file("original.bin", 4)
        .add_hard_link("original.bin", "../link.bin")
        .build()
        .expect_err("escaping hard link path should be rejected");

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[cfg(unix)]
#[test]
fn symlink_targets_are_not_confined_but_link_paths_are() {
    let target = PathBuf::from("/tmp/diskgoblin-nonexistent-target");
    let fixture = FixtureBuilder::new()
        .add_symlink(&target, "outside-target-link")
        .build()
        .expect("fixture should build");

    let link = fixture.path().join("outside-target-link");
    assert_eq!(fs::read_link(&link).expect("symlink target"), target);

    let error = FixtureBuilder::new()
        .add_symlink("target", "../escape")
        .build()
        .expect_err("escaping symlink path should be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

    let error = FixtureBuilder::new()
        .add_symlink("", "empty-target-link")
        .build()
        .expect_err("empty symlink target should be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path)
        .unwrap_or_else(|error| panic!("failed to read metadata for {}: {error}", path.display()))
        .len()
}
