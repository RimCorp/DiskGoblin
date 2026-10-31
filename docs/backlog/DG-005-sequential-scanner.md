# DG-005: Implement sequential directory traversal with symlink, mount, and error handling

- **Milestone:** M1 — CLI MVP
- **Type:** feature
- **Priority:** P0
- **Complexity Estimate:** L (60–90 minutes)
- **Risk:** Medium

## Objective
Implement the deterministic, single-threaded directory traversal engine (`scanner::sequential`) in `diskgoblin-core` that safely traverses directory hierarchies using `symlink_metadata`, respects `--one-file-system` mount boundaries, and collects non-fatal `ScanErrorRecord` entries without aborting.

## Context
Per [`ADR-003`](../decisions/003-symlinks-mounts-and-errors.md) and [`ADR-005`](../decisions/005-traversal-and-concurrency.md), DiskGoblin's `M1` scanner must establish a rock-solid sequential reference baseline before any parallel optimization (`M2`). It must never follow symbolic links, must optionally stay on the root `device_id` (`one_file_system`), and must distinguish fatal root errors (`FatalScanError`) from non-fatal descendant errors (`ScanErrorRecord`).

## Scope
- `crates/diskgoblin-core/src/scanner/mod.rs` and `crates/diskgoblin-core/src/scanner/sequential.rs`:
  - Define internal/intermediate raw traversal output structures (`VisitedDir`, `VisitedEntry`, or visitor callback) feeding the aggregator (`DG-006`).
  - Validate the scan root using `std::fs::symlink_metadata(&options.root_path)`:
    - Return `Err(FatalScanError::RootNotFound)` if `ErrorKind::NotFound`.
    - Return `Err(FatalScanError::RootUnreadable)` on other root metadata or root `read_dir` errors.
  - Traverse descendant directories in deterministic lexicographical order (sorting each directory's entries by file name).
  - Never follow symbolic links (`EntryKind::Symlink` entries are recorded as leaf symlink entries and never recursed into).
  - When `options.one_file_system == true`, skip recursing into any child directory whose `file_id.device_id != root_device_id`.
  - Record any descendant `read_dir` or `symlink_metadata` error as a `ScanErrorRecord` associated with the failed path and increment the enclosing directory's direct error counter.
- Integration tests in `crates/diskgoblin-core/tests/sequential_scanner_test.rs` using `FixtureBuilder` (`DG-003`).

## Out of scope
- Do not implement multi-threaded/parallel traversal (`DG-012`).
- Full bottom-up tree rollups, hard-link deduplication, and top-N heaps belong to `DG-006` (though `DG-005` must emit all necessary `RawMetadata`, depth, parent-child relationship, and error records).

## Dependencies
- `DG-002` (Core domain types & `platform::extract_metadata`)
- `DG-003` (`FixtureBuilder` for synthetic filesystem tests)

## Implementation guidance
- Using standard library `std::fs::read_dir` + `std::fs::symlink_metadata` with an explicit stack (or post-order recursion) avoids external traversal dependencies and gives full control over sorting child entries by `OsString` for determinism, checking `device_id`, and attributing `read_dir` errors to the exact directory node.
- Be careful with deep directory trees: prefer an explicit stack (`Vec<StackFrame>`) or tested recursion so deeply nested directories cannot overflow the call stack.

## Acceptance criteria
- [ ] Scanning a nonexistent root returns `Err(FatalScanError::RootNotFound)`.
- [ ] Scanning an unreadable root (`0o000`) on Unix (when non-root) returns `Err(FatalScanError::RootUnreadable)`.
- [ ] Scanning a tree containing a symlink loop (`a/b/loop -> ../..`) and a broken symlink (`dangling -> missing`) terminates cleanly, records `symlink_count == 2`, and does not recurse into `loop`.
- [ ] Scanning a tree with an unreadable child directory (`0o000`) succeeds (`Ok`), visits all readable siblings, and records a `ScanErrorRecord` with `ScanErrorKind::PermissionDenied` for the unreadable subdirectory.
- [ ] When `one_file_system` is enabled, directories with a differing `device_id` (verified via unit test hook or mock metadata) are skipped from recursion.
- [ ] Traversal order is 100% deterministic across repeated runs.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-core --test sequential_scanner_test
```

## Definition of done
- All unit and integration tests pass on macOS and Ubuntu.
- PR opened linking `DG-005` with Agent Implementation Report.
