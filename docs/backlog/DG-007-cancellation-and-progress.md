# DG-007: Implement cooperative cancellation token and throttled progress reporting

- **Milestone:** M1 — CLI MVP
- **Type:** feature
- **Priority:** P1
- **Complexity Estimate:** M (45–60 minutes)
- **Risk:** Low

## Objective
Implement `CancelToken`, `ScanProgress`, `ProgressSink`, and a rate-limited `ThrottledProgressSink` in `diskgoblin-core::cancel`, and integrate cooperative cancellation and progress callbacks into the scanner pipeline.

## Context
Per [`docs/architecture.md` §4.3](../architecture.md) and [`ADR-004`](../decisions/004-result-model-and-streaming.md), users must be able to cancel long-running scans cleanly (`< 100 ms` latency on Ctrl-C or Desktop Cancel button) and receive live progress updates throttled to at most ~20 Hz (`50 ms` intervals) without degrading scan throughput.

## Scope
- `crates/diskgoblin-core/src/cancel.rs`:
  - `CancelToken` wrapping `Arc<AtomicBool>` (`new()`, `cancel()`, `is_cancelled()`).
  - `ScanProgress` struct (`files_seen`, `dirs_seen`, `symlinks_seen`, `errors_seen`, `bytes_accumulated`, `elapsed_ms`).
  - `ProgressSink` trait (`fn on_progress(&self, progress: ScanProgress)`) + `NoopProgressSink` implementation + closure wrapper `FnProgressSink<F>`.
  - `ThrottledProgressSink<S: ProgressSink>` that forwards `on_progress` only when at least `min_interval: Duration` (default `50 ms`) has elapsed since the previous forwarded update, plus an explicit `.flush(progress)` at scan completion.
- Integrate `&CancelToken` and `&dyn ProgressSink` into `scan_directory_with_progress(options: &ScanOptions, cancel: &CancelToken, progress: &dyn ProgressSink) -> Result<ScanReport, FatalScanError>` (keeping `scan_directory(options)` as a convenience wrapper passing `CancelToken::new()` and `&NoopProgressSink`).

## Out of scope
- Do not implement OS `SIGINT` signal registration (`ctrlc` crate wiring belongs in `diskgoblin-cli` under `DG-009`).
- Do not write directly to `stderr` inside `diskgoblin-core`.

## Dependencies
- `DG-002` (Core types)
- `DG-006` (Sequential scanner & `scan_directory` entry point)

## Implementation guidance
- Check `cancel.is_cancelled()` at the start of each directory visit and every `256` entries within a directory so even huge flat directories abort promptly without checking `Instant::now()` on every single file.
- Similarly, only check `Instant::now()` for `ThrottledProgressSink` every `256` entries (or on directory transitions) to avoid `clock_gettime` syscall overhead in tight loops.

## Acceptance criteria
- [ ] Pre-cancelling a `CancelToken` before calling `scan_directory_with_progress` immediately returns `Err(FatalScanError::Cancelled)`.
- [ ] Cancelling the token from a progress callback mid-scan stops traversal promptly and returns `Err(FatalScanError::Cancelled)`.
- [ ] `ThrottledProgressSink` suppresses rapid back-to-back updates within `< min_interval` and delivers the final flushed `ScanProgress` matching the completed `ScanReport` counts.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-core cancel
```

## Definition of done
- Unit and integration tests cover pre-cancelled tokens, mid-scan cancellation, and throttling behavior.
- PR opened linking `DG-007` with Agent Implementation Report.
