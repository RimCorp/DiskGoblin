# ADR-004: Hybrid Streaming Progress and Bounded Result Tree

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Principal Architect, Product Owner

## Context

`PLANNING_CHARTER.md` asks: *"Should scanning stream events or produce a complete result tree?"*

- A **purely event-streaming API** (yielding every visited file and directory as an iterator) shifts the burden of bottom-up directory size aggregation, hard-link deduplication, and top-N sorting onto every caller (`diskgoblin-cli` and `diskgoblin-desktop`). Because directory totals cannot be known until all descendants have been visited, callers cannot display sorted top-level directories until the scan finishes anyway.
- Conversely, a **purely batch API** with no streaming feedback appears frozen for several seconds when scanning a 500,000-file disk and provides no hook for live UI updates or responsive cancellation.
- Furthermore, retaining a complete tree of *every file* in memory (`O(F)`) consumes hundreds of megabytes on million-file volumes.

## Decision

We adopt a **Hybrid Progress-Stream + Bounded-Tree Result** architecture:

1. **Streaming Progress & Cooperative Cancellation:**
   `scan_directory(options, &cancel_token, &progress_sink)` accepts a `CancelToken` and a `ProgressSink` callback/trait.
   - As the scanner traverses entries, it periodically emits lightweight `ScanProgress` snapshots (`files_seen`, `dirs_seen`, `bytes_accumulated`, `elapsed_ms`) throttled to at most once every `50 ms` (20 Hz).
   - Both the CLI (via `stderr` status line) and the Tauri Desktop app (via Tauri window events) consume `ProgressSink` for live feedback.

2. **Bounded-Memory Aggregation (`O(D_depth + K)`):**
   - **Files are never stored in full in the tree:** Each regular file immediately adds its sizes to its parent directory accumulator and is offered to a bounded min-heap of capacity `K = options.top_n` (`BinaryHeap` of largest files).
   - **Directories are rolled up bottom-up:** All directories are accumulated to compute accurate ancestor sizes and select `top_directories` (bounded to `K = options.top_n`), while the hierarchical `DirNode` tree returned in `ScanReport::root` is pruned to `options.max_depth` (default `3` for CLI, configurable up to e.g. `5` or on-demand drill-down for Desktop).

## Consequences

### Positive
- Callers (`diskgoblin-cli` and `diskgoblin-desktop`) share 100% of aggregation, deduplication, and sorting logic inside `diskgoblin-core`.
- Memory usage stays small and predictable even on multi-million-file filesystems.
- Live progress updates remain smooth and non-blocking in both terminal and GUI environments.

### Negative
- If a Desktop UI user drills down deeper than `max_depth` in the treemap (`M3`), the desktop backend either requests a deeper initial `max_depth` (e.g., `6`) or runs a fast targeted sub-scan on the clicked directory.
