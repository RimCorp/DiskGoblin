# DG-006: Implement bottom-up directory aggregation, hard-link deduplication, and top-N selection

- **Milestone:** M1 — CLI MVP
- **Type:** feature
- **Priority:** P0
- **Complexity Estimate:** L (60–90 minutes)
- **Risk:** Medium

## Objective
Implement the tree aggregation and ranking engine (`aggregator.rs`) in `diskgoblin-core` that turns raw traversal entries from `DG-005` into a complete `ScanReport`, performing bottom-up directory rollups, `(dev, ino)` hard-link deduplication, `max_depth` tree pruning, and bounded-memory `BinaryHeap` selection for `top_directories` and `top_files`.

## Context
Per [`docs/architecture.md` §4–§6](../architecture.md), [`ADR-002`](../decisions/002-size-metrics-and-hardlinks.md), and [`ADR-004`](../decisions/004-result-model-and-streaming.md):
1. A directory's `allocated_size`, `apparent_size`, `file_count`, `dir_count`, `symlink_count`, and `error_count` must equal the sum of its direct contents plus all recursive subdirectories.
2. Hard-linked files (`hard_link_count > 1` sharing `FileId { device_id, inode }`) must only contribute their byte size on first encounter; subsequent links in the same scan are marked `is_deduplicated_hardlink: true` and contribute `0` bytes to directory totals.
3. Individual files must not all be retained in memory (`O(K)` top-N heap only).

## Scope
- `crates/diskgoblin-core/src/aggregator.rs`:
  - Hard-link tracker (`HashSet<FileId>`) inserting only when `raw_metadata.hard_link_count > 1`.
  - Bottom-up rollup computing `DirSummary` for every visited directory.
  - Pruning `DirNode::children` beyond `options.max_depth` while still rolling up sizes from all depths into ancestor `DirSummary` totals.
  - Bounded top-N selection (`options.top_n` and `options.min_size_bytes`) for `top_files: Vec<FileEntry>` and `top_directories: Vec<DirSummary>` (excluding the root directory itself from `top_directories`, or ordering all directories by selected `SizeMetric` descending, tie-breaking by `path` ascending).
- Wire `scan_directory(options: &ScanOptions) -> Result<ScanReport, FatalScanError>` in `crates/diskgoblin-core/src/lib.rs` (or `scanner/mod.rs`).
- Integration tests in `crates/diskgoblin-core/tests/aggregation_test.rs`.

## Out of scope
- Cooperative cancellation and progress callbacks (`DG-007` will extend the signature/wiring).
- Parallel scanning (`DG-012`).

## Dependencies
- `DG-005` (Sequential scanner).

## Implementation guidance
- **Deterministic Tie-Breaking:** When sorting `top_files`, `top_directories`, and `DirNode::children`, sort by:
  1. Primary metric size (`options.size_metric`: `allocated_size` or `apparent_size`) **descending**.
  2. `path` **ascending** (lexicographical tie-breaker).
- **Hard-Link Rule:** When a file with `hard_link_count > 1` is seen a second time:
  - Increment the parent directory's `file_count` by 1.
  - Add `0` to `allocated_size` and `0` to `apparent_size` of the parent directory.
  - Do not allow deduplicated hard-link aliases to crowd out unique files in `top_files` (or if included when `is_deduplicated_hardlink` is tracked, its effective ranked size is the file's size only on its primary path; prefer only ranking the primary non-deduplicated entry in `top_files` and exposing `is_deduplicated_hardlink` if inspected directly, or test both behaviors clearly—Specifically: count & rank the primary encounter in `top_files` with `is_deduplicated_hardlink: false`, and if `options` or unit tests inspect hardlink deduplication, ensure directory totals never double-count).

## Acceptance criteria
- [ ] Root `DirSummary` totals accurately equal the sum of all descendant files, directories, symlinks, and errors.
- [ ] In a fixture with three hard links to the same `1 MiB` file across two subdirectories, `root.summary.apparent_size` includes the `1 MiB` file **once** (not `3 MiB`), while `root.summary.file_count` is `3`.
- [ ] Setting `max_depth = Some(1)` limits `root.children` to depth 1 (`child.children.is_empty()`), while `root.summary` and depth-1 `child.summary` still include all bytes from depths `2..N`.
- [ ] `top_files` and `top_directories` contain at most `options.top_n` items, exclude entries smaller than `options.min_size_bytes`, and sort deterministically by `(metric_size DESC, path ASC)`.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-core --test aggregation_test
```

## Definition of done
- All unit and integration tests pass.
- PR opened linking `DG-006` with Agent Implementation Report.
