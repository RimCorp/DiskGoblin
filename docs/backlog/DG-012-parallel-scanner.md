# DG-012: Implement parallel traversal engine and evaluate SSD thread scaling

- **Milestone:** M2 — Performance
- **Type:** feature
- **Priority:** P1
- **Complexity Estimate:** L (75–120 minutes)
- **Risk:** High

## Objective
Implement the bounded multi-threaded directory scanner (`scanner::parallel`) in `diskgoblin-core`, guarantee deterministic output identical to the sequential scanner (including canonical hard-link attribution), benchmark thread scaling (`threads ∈ {1, 2, 4, 8}`) on SSD, and document empirical findings.

## Context
Per [`PLANNING_CHARTER.md` §3](../../PLANNING_CHARTER.md) and [`ADR-005`](../decisions/005-traversal-and-concurrency.md), we must investigate whether parallel filesystem traversal improves performance on SSDs or introduces excessive I/O contention, and ensure parallel execution never compromises deterministic results.

## Scope
- `crates/diskgoblin-core/Cargo.toml`: Add `rayon = "1.10"` (or `jwalk = "0.8"`—prefer a bounded `rayon::ThreadPool` or `jwalk::WalkDirGeneric` with explicit thread pool control).
- `crates/diskgoblin-core/src/scanner/parallel.rs`:
  - Execute directory traversal across a bounded thread pool (`num_threads = if options.threads == 0 { num_cpus.clamp(1, 8) } else { options.threads }`).
  - Route `options.threads == 1` directly to `scanner::sequential`.
  - Respect `CancelToken` across all worker threads and update atomic progress counters for `ProgressSink`.
  - **Deterministic Hard-Link & Rollup Rule:** Collect visited directory contents and resolve hard-link deduplication (`hard_link_count > 1`) in deterministic lexicographical path order before final bottom-up directory rollups so `scan(threads=1)` and `scan(threads=N)` produce identical `ScanReport` outputs.
- Extend `crates/diskgoblin-core/benches/scanner_bench.rs` to benchmark `threads ∈ {1, 2, 4, 8}`.
- Record empirical SSD benchmark results and thread-scaling analysis in `docs/experiments/ssd-parallel-traversal-study.md`.

## Out of scope
- Changing public `ScanReport` JSON schema.

## Dependencies
- `DG-011` (Criterion benchmark harness).

## Implementation guidance
- Why deterministic post-order rollup matters: If thread A visits `dir_b/link` before thread B visits `dir_a/link`, a naive concurrent `DashSet<FileId>` would attribute the file's bytes to `dir_b` instead of `dir_a`, causing `dir_a` and `dir_b` sizes to differ from `--threads 1`. By sorting discovered directory nodes or attributing multi-link `FileId`s to their lexicographically smallest path prior to bottom-up summation, parallel traversal remains 100% deterministic!
- Only change the CLI default `--threads` from `1` to `0` (auto bounded pool) if the benchmark study confirms `≥ 1.5x` speedup on macOS SSD without regression.

## Acceptance criteria
- [ ] `scan_directory` with `threads = 4` produces the exact same `ScanReport` (root summary, directory tree, top directories, top files, and sorted errors, excluding `duration_ms`) as `threads = 1` on fixtures containing nested dirs, symlinks, hard links, and permission errors.
- [ ] `CancelToken::cancel()` halts all parallel worker threads within `< 100 ms`.
- [ ] `docs/experiments/ssd-parallel-traversal-study.md` documents Criterion benchmark results across `threads = 1, 2, 4, 8` and explains observed SSD scaling vs. VFS contention.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo bench -p diskgoblin-core -- --quick
```

## Definition of done
- All tests and benchmarks pass; empirical study committed to `docs/experiments/ssd-parallel-traversal-study.md`.
- PR opened linking `DG-012` with Agent Implementation Report.
