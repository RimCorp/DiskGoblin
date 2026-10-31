# DG-003: Build deterministic synthetic filesystem fixture generator for tests

- **Milestone:** M0 — Foundation
- **Type:** feature
- **Priority:** P0
- **Complexity Estimate:** M (45–75 minutes)
- **Risk:** Low

## Objective
Create a declarative, reusable synthetic filesystem fixture builder (`FixtureBuilder`) in `diskgoblin-core` so all unit, integration, and benchmark tests can construct deterministic temporary directory trees (including nested dirs, sparse/sized files, symlinks, hard links, and restricted-permission directories) with automatic cleanup on drop.

## Context
Per [`docs/testing-strategy.md`](../testing-strategy.md), DiskGoblin tests must never rely on the developer's host filesystem. Moreover, tests that create `chmod 000` directories on Unix will fail to clean up their `TempDir` on drop unless permissions are automatically restored during teardown.

## Scope
- `crates/diskgoblin-core/src/test_fixtures.rs` (exposed publicly or behind a `test-utils` feature / `#[cfg(any(test, feature = "test-utils"))]`, enabled for tests and workspace crates):
  - `FixtureBuilder` and `TestFixture` struct wrapping `tempfile::TempDir`.
  - Builder methods:
    - `.add_dir(rel_path)`
    - `.add_file(rel_path, size_bytes)` (writes deterministic content of `size_bytes`)
    - `.add_hard_link(existing_rel_path, new_link_rel_path)`
    - `.add_symlink(target_path, link_rel_path)` (`#[cfg(unix)]`)
    - `.add_unreadable_dir(rel_path)` (`#[cfg(unix)]`, creates dir and sets mode `0o000`, recording the path so `Drop for TestFixture` restores `0o700` before `TempDir` drops).
- Integration test in `crates/diskgoblin-core/tests/fixture_builder_test.rs` verifying every builder capability and confirming clean teardown.

## Out of scope
- Do not implement scanning or aggregation logic (`DG-005`, `DG-006`).

## Dependencies
- `DG-001` (Workspace scaffold). Can be implemented in parallel with `DG-002`.

## Implementation guidance
- Ensure `TestFixture` implements `Drop` to iterate over any `chmod 000` paths and restore `Permissions::from_mode(0o700)` before the inner `TempDir` is dropped. Without this, `TempDir::drop` silently fails to delete unreadable directories on macOS and Linux!
- Provide a helper `is_root_user() -> bool` (`#[cfg(unix)]`) so permission-denied tests can gracefully skip assertions if run inside a root Docker container (`uid == 0`).

## Acceptance criteria
- [ ] `FixtureBuilder` can construct nested directories, files of exact byte lengths, hard links sharing an inode, symlinks (valid, ancestor-loop, and dangling), and unreadable (`0o000`) directories.
- [ ] Dropping a `TestFixture` containing an unreadable (`0o000`) directory restores permissions and deletes the temporary directory completely from disk (`!fixture_path.exists()` after drop).
- [ ] All relative paths passed to `FixtureBuilder` are validated to stay inside the `TempDir` root.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-core --test fixture_builder_test
```

## Definition of done
- All fixture tests pass and verify zero leaked temporary directories.
- PR opened linking `DG-003` with Agent Implementation Report.
