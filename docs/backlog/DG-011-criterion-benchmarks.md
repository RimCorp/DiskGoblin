# DG-011: Create Criterion benchmark suite with synthetic directory topologies

- **Milestone:** M2 — Performance
- **Type:** research
- **Priority:** P1
- **Complexity Estimate:** M (45–75 minutes)
- **Risk:** Low

## Objective
Establish a reproducible Criterion benchmark suite (`crates/diskgoblin-core/benches/scanner_bench.rs`) that measures scan latency and throughput (entries/sec) across three realistic synthetic filesystem topologies before introducing parallel traversal in `DG-012`.

## Context
Per [`PLANNING_CHARTER.md` §3 & §5](../../PLANNING_CHARTER.md) and [`ADR-005`](../decisions/005-traversal-and-concurrency.md), performance optimizations must be justified by empirical benchmarks on realistic fixtures rather than assumptions.

## Scope
- `crates/diskgoblin-core/Cargo.toml`: Add `[dev-dependencies]` `criterion = { version = "0.5", features = ["html_reports"] }` and configure `[[bench]] name = "scanner_bench" harness = false`.
- `crates/diskgoblin-core/benches/scanner_bench.rs`:
  - Build fixtures once per benchmark group outside the timed closure (`iter_batched` or pre-built `TestFixture` per group):
    1. `wide_tree`: 50 directories × 200 files = 10,000 files.
    2. `deep_tree`: depth 20 nested directories with 2,500 files and symlinks.
    3. `hardlink_cache_tree`: 5,000 files where 60% are hard links (`nlink > 1`).
  - Measure throughput (`Throughput::Elements(total_entries)`).
- Verify `cargo bench --workspace --no-run` compiles cleanly in CI.

## Out of scope
- Implementing parallel traversal (`DG-012`).

## Dependencies
- `DG-010` (Completed `M1` sequential baseline).

## Implementation guidance
- Do **not** include `FixtureBuilder` file creation inside the timed `b.iter(|| ...)` loop—only benchmark `scan_directory(&options)`.
- Keep fixture sizes around `2,500–10,000` files so a full `cargo bench` run completes in under 60 seconds while still executing enough `lstat`/`readdir` syscalls to measure kernel I/O and aggregation overhead accurately.

## Acceptance criteria
- [ ] `cargo bench -p diskgoblin-core --no-run` compiles with zero warnings.
- [ ] Running `cargo bench -p diskgoblin-core -- --quick` executes all three topology benchmarks (`wide_tree`, `deep_tree`, `hardlink_cache_tree`) and reports throughput in elements/second.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo bench -p diskgoblin-core -- --quick
```

## Definition of done
- Benchmark suite compiles and runs cleanly; baseline sequential numbers recorded in PR report.
- PR opened linking `DG-011` with Agent Implementation Report.
