# ADR-005: Sequential Baseline vs. Parallel Filesystem Traversal on SSDs

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Principal Architect, Product Owner

## Context

`PLANNING_CHARTER.md` specifically instructs us to:
> *"Investigate whether parallel filesystem traversal improves performance on SSDs and whether it introduces excessive I/O contention."*
> *"Correctness before optimization. Establish a trustworthy sequential baseline before introducing concurrency."*

### Technical Analysis of Parallel `readdir` / `lstat` on Modern SSDs & OS Kernels

1. **Why Sequential Traversal Leaves SSD Bandwidth Idle:**
   In a sequential scanner (`walkdir` or single-threaded `std::fs::read_dir`), the thread spends most of its time blocked inside kernel syscalls (`getdirentries64` / `getdents64` and `fstatat64` / `lstat`). Modern NVMe SSDs support deep hardware command queues (up to 64K queues) and high IOPS, which a single synchronous syscall thread cannot saturate—especially on cold or partially warm caches.
2. **Where Parallel Contention Occurs on macOS (APFS) and Linux:**
   - **APFS VFS / Directory Locks:** On macOS APFS, moderate thread counts (`4–8` threads on Apple Silicon) yield 2.5x–4x speedups over single-threaded `walkdir` (as demonstrated by tools like `fd`, `ripgrep`, and `dust` using `jwalk` or `rayon`). However, spawning unbounded threads (e.g., >16 threads hammering `lstat` simultaneously) increases kernel VFS lock contention and context-switching overhead, causing throughput to plateau or degrade.
   - **Determinism Hazard:** Parallel directory visits happen in non-deterministic thread scheduling order. If hard-link deduplication (`(dev, ino)`) or equal-size tie-breaking depends on visit order, parallel runs can produce slightly different per-directory totals when two directories share hard links.

## Decision

1. **Milestone `M1` Uses a Deterministic Sequential Scanner (`threads = 1`):**
   - In `M1`, `diskgoblin-core` implements `scanner::sequential`, sorting each directory's entries by filename before recursion.
   - This provides a 100% deterministic reference oracle for unit tests, property tests, and hard-link deduplication.

2. **Milestone `M2` Introduces a Bounded Parallel Scanner Verified Against `M1`:**
   - In `M2`, we implement `scanner::parallel` (evaluating `jwalk` vs. a custom bounded `rayon` work-stealing directory queue).
   - **Contention Mitigation:** Cap default worker threads to `min(num_cpus, 8)` rather than unbounded threads, configurable via `--threads <N>` (`-j 1` always forces the sequential engine).
   - **Hard-Link Determinism in Parallel Mode:** When a file has `nlink > 1`, parallel workers record its `(FileId, PathBuf, RawMetadata)` into a shared collection; at the end of traversal (before final directory rollups), hard-linked groups are canonically attributed to the lexicographically smallest `PathBuf`, guaranteeing **byte-for-byte identical `ScanReport` output** between `--threads 1` and `--threads N`.
   - **Empirical Gate:** We will benchmark `threads ∈ {1, 2, 4, 8, 16}` using Criterion across three synthetic fixtures (wide tree, deep tree, high-file-count tree) and record the empirical curves in `docs/experiments/`. Parallel mode becomes the default only if it achieves `≥ 1.5x` speedup on macOS SSD without violating equivalence tests.

## Consequences

### Positive
- Zero concurrency bugs in `M1` while agents build out aggregation, CLI rendering, and JSON output.
- Mathematical guarantee that parallel mode (`M2`) produces the exact same `ScanReport` as sequential mode (`M1`), enabling a simple property test: `assert_eq!(scan(threads=1), scan(threads=4))`.
- Empirical benchmark evidence on SSD thread scaling feeds directly into the `M2` performance report.

### Negative
- Requires maintaining both `sequential.rs` and `parallel.rs` behind the `scanner` module (kept small because both feed the same aggregation structures).
