# DiskGoblin — Testing & Verification Strategy

## 1. Core Testing Principles

1. **Zero Host Filesystem Dependence:** Tests must never scan `/Users`, `/tmp` broadly, or the repository's own `target/` directory. Every filesystem test constructs a dedicated, deterministic temporary directory via `tempfile::TempDir` that is automatically cleaned up on drop.
2. **Deterministic Assertions:** All directory rollups, top-N lists, and error records must be deterministically ordered (sorting by size descending, then relative path lexicographically ascending) so assertions never flake due to OS `readdir` ordering or thread scheduling.
3. **Strict Equivalence Oracle (`M2`):** The single-threaded sequential scanner (`M1`, `--threads 1`) serves as the formal specification oracle for the multi-threaded scanner (`M2`, `--threads N`).

---

## 2. Test Pyramid & Layers

### Layer 1: Pure Unit Tests (`crates/diskgoblin-core/src/**`)
Fast (`< 50 ms` total), in-memory tests that do not touch the OS filesystem:
- **Byte Formatting (`format.rs`):** Boundary values (`0`, `1023`, `1024`, `1024^2 - 1`, `u64::MAX`) under both binary IEC (`KiB`, `MiB`, `GiB`, `TiB`) and decimal SI (`KB`, `MB`, `GB`, `TB`).
- **Tree Aggregation & Top-K Selection (`aggregator.rs`):** Feed synthetic `RawEntry` streams into the aggregator and verify:
  - Bottom-up `allocated_size` and `apparent_size` sums across multi-level directories.
  - Tie-breaking order when multiple files or directories share the exact same byte size.
  - Depth pruning (`max_depth = 0, 1, 2, 3`) preserves root and ancestor totals while bounding `DirNode::children` depth.
  - Hard-link deduplication (`FileId` seen set) counts size once and marks subsequent links with `is_deduplicated_hardlink = true`.
- **Cancellation & Progress Throttling (`cancel.rs`):** Verify `CancelToken` visibility across threads and verify that `ThrottledProgressSink` suppresses callbacks occurring within `< 50 ms` except for the final flush.

### Layer 2: Synthetic Filesystem Integration Tests (`crates/diskgoblin-core/tests/`)
Uses the declarative `FixtureBuilder` (`DG-003`) backed by `tempfile::TempDir`:
- **Regular Hierarchy:** Nested directories (`a/b/c`) with known file sizes (`1024 B`, `65536 B`, `0 B`).
- **Symlink Safety (`#[cfg(unix)]`):**
  - Symlink pointing to an ancestor directory (`a/b/loop -> ../..`) — verify scan terminates, counts `symlink_count == 1`, and does not recurse.
  - Symlink pointing to a large file outside the scan root — verify target file bytes are not added to directory totals.
  - Broken symlink (`dangling -> ./nonexistent`) — verify `symlink_metadata` succeeds and counts as a symlink without recording an `IoError`.
- **Hard-Link Deduplication (`#[cfg(unix)]`):**
  - Create a `1 MiB` file at `dir1/original.bin` and hard-link it (`std::fs::hard_link`) at `dir1/link2.bin` and `dir2/link3.bin`.
  - Verify root `apparent_size` equals `1 MiB` (not `3 MiB`), `file_count == 3`, and exactly two `FileEntry` records have `is_deduplicated_hardlink == true`.
- **Permission Errors (`#[cfg(unix)]`):**
  - Create a subdirectory `restricted/` containing files, then `chmod 000 restricted`.
  - Include a guard: skip permission assertions if running as `uid == 0` (root in container).
  - Verify scan succeeds (`Ok(ScanReport)`), records one `ScanErrorRecord` with `ScanErrorKind::PermissionDenied`, increments `root.summary.error_count == 1`, and restores `chmod 700 restricted` before `TempDir` drop so cleanup succeeds.
- **Mount Boundaries (`--one-file-system`):**
  - Inject a mock/trait or device-ID filter test verifying that directories reporting `device_id != root_device_id` are not traversed when `one_file_system == true`.

### Layer 3: Property-Based Tests (`proptest`, `DG-013`)
- Generate arbitrary directory trees (random depth `1..6`, branching factor `0..8`, file sizes `0..10_000_000`, optional hard links and symlinks).
- **Invariants verified on every generated tree:**
  1. `root.summary.apparent_size` equals the sum of unique (deduplicated) file/symlink apparent sizes in the tree.
  2. For every directory node `D`, `D.apparent_size >= sum(child.apparent_size for child in D.children)`.
  3. `top_files.len() <= options.top_n` and `top_files` is strictly sorted by `(metric_size DESC, path ASC)`.
  4. **Sequential vs. Parallel Equivalence:** `scan_with_threads(tree, 1)` produces a `ScanReport` identical in all fields (ignoring `duration_ms`) to `scan_with_threads(tree, 4)`.

### Layer 4: CLI Binary End-to-End Tests (`crates/diskgoblin-cli/tests/cli_integration.rs`)
Invokes the compiled `diskgoblin` binary using `assert_cmd` and `predicates`:
- Verify default human output contains summary headers, top directories, and top files.
- Verify `--json` outputs valid JSON matching `schema_version: 1` on `stdout` with zero progress noise on `stdout`.
- Verify nonexistent root path exits with code `1` and helpful `stderr` message.
- Verify invalid flags exit with code `2`.

### Layer 5: Performance Benchmarks (`crates/diskgoblin-core/benches/`, `DG-011`)
Uses **Criterion** with deterministic fixture generators cached per benchmark group:
1. **Wide Topology:** 100 directories × 500 files each (50,000 files).
2. **Deep Topology:** Depth 25 nested directories with mixed files and symlinks.
3. **Hard-Link Heavy Topology:** Simulating a `pnpm` / `uv` cache with 10,000 files where 70% are hard links (`nlink > 1`).
4. **Thread Scaling Matrix:** Measure throughput across `threads ∈ {1, 2, 4, 8}` and export JSON benchmark results.

---

## 3. Platform & CI Verification Matrix

GitHub Actions (`.github/workflows/ci.yml`) runs on every pull request and push to `main`:

| Check | Command | OS Matrix | Gate Requirement |
|---|---|---|---|
| **Formatting** | `cargo fmt --all -- --check` | `ubuntu-latest` | Must pass (0 diff) |
| **Linting** | `cargo clippy --workspace --all-targets -- -D warnings` | `macos-latest`, `ubuntu-latest` | Must pass (0 warnings) |
| **Unit & Integration Tests** | `cargo test --workspace --all-targets` | `macos-latest`, `ubuntu-latest` | 100% pass |
| **Doc Tests** | `cargo test --workspace --doc` | `ubuntu-latest` | 100% pass |
| **Benchmark Compile Check** | `cargo bench --workspace --no-run` | `ubuntu-latest` | Must compile cleanly |
