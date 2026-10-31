# DG-002: Implement core domain models, error types, and platform metadata adapter

- **Milestone:** M0 — Foundation
- **Type:** feature
- **Priority:** P0
- **Complexity Estimate:** M (45–75 minutes)
- **Risk:** Low

## Objective
Define the authoritative domain types, configuration structs, error hierarchy, and cross-platform metadata extraction adapter in `diskgoblin-core` so multiple downstream tasks can develop against stable Rust interfaces in parallel.

## Context
Per [`docs/architecture.md` §4](../architecture.md) and [`ADR-002`](../decisions/002-size-metrics-and-hardlinks.md) / [`ADR-003`](../decisions/003-symlinks-mounts-and-errors.md), `diskgoblin-core` requires explicit data structures for scan options, dual size metrics (`Allocated` vs. `Apparent`), fatal vs. non-fatal scan errors, and Unix inode/block metadata (`st_blocks`, `st_dev`, `st_ino`, `st_nlink`).

## Scope
- `crates/diskgoblin-core/Cargo.toml`: Add approved dependencies `thiserror = "2"` and `serde = { version = "1", features = ["derive"] }` (and `tempfile = "3"` under `[dev-dependencies]`).
- `crates/diskgoblin-core/src/config.rs`: `SizeMetric`, `ScanOptions` (with sensible `Default` implementation).
- `crates/diskgoblin-core/src/error.rs`: `FatalScanError`, `ScanErrorKind`, `ScanErrorRecord`, and helper converting `std::io::Error` + `PathBuf` into `ScanErrorRecord`.
- `crates/diskgoblin-core/src/model.rs`: `FileEntry`, `DirSummary`, `DirNode`, `ScanReport` (with `SizeMetric::select(apparent, allocated) -> u64` helper).
- `crates/diskgoblin-core/src/platform.rs`: `FileId`, `EntryKind`, `RawMetadata`, and `extract_metadata(&std::fs::Metadata) -> RawMetadata` using `std::os::unix::fs::MetadataExt` on `#[cfg(unix)]` (`blocks() * 512`, `dev()`, `ino()`, `nlink()`) and safe fallback on `#[cfg(not(unix))]`.
- `crates/diskgoblin-core/src/lib.rs`: Re-export modules and public types.

## Out of scope
- Do not implement recursive directory traversal (`DG-005`) or tree aggregation (`DG-006`).
- Do not add `serde_json` serialization formatting (`DG-008`) or CLI dependencies.

## Dependencies
- `DG-001` (Workspace scaffold).

## Implementation guidance
- Follow the exact struct and enum field definitions in [`docs/architecture.md` §4](../architecture.md).
- On Unix (`#[cfg(unix)]`), compute `allocated_size = metadata.blocks().saturating_mul(512)`.
- Classify `std::io::ErrorKind::PermissionDenied` as `ScanErrorKind::PermissionDenied`, `std::io::ErrorKind::NotFound` as `ScanErrorKind::NotFound`, and all other `ErrorKind` variants as `ScanErrorKind::IoError`.
- Provide unit tests in `platform.rs` and `error.rs` using `tempfile::TempDir` to inspect real metadata of a regular file, a directory, a symlink (`std::os::unix::fs::symlink`), and a hard link (`std::fs::hard_link`).

## Acceptance criteria
- [ ] `ScanOptions`, `SizeMetric`, `FatalScanError`, `ScanErrorKind`, `ScanErrorRecord`, `FileEntry`, `DirSummary`, `DirNode`, `ScanReport`, `FileId`, `EntryKind`, and `RawMetadata` are exported by `diskgoblin_core`.
- [ ] On Unix, `extract_metadata` returns `EntryKind::Symlink` when given `std::fs::symlink_metadata` for a symlink, populates `FileId { device_id, inode }`, sets `hard_link_count = 2` for a pair of hard-linked files with identical `FileId`, and computes `allocated_size = blocks * 512`.
- [ ] `ScanErrorRecord::from_io_error(path, err)` accurately maps `PermissionDenied`, `NotFound`, and generic `IoError`.
- [ ] All public types derive `Debug`, `Clone`, `PartialEq`, `Eq` (and `Serialize`, `Deserialize` where specified in `docs/architecture.md`).

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-core
```

## Definition of done
- All unit tests pass on macOS and Linux.
- Zero Clippy warnings.
- PR opened linking `DG-002` with Agent Implementation Report.
