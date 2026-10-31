# DG-009: Implement `diskgoblin-cli` argument parser, terminal table renderer, and SIGINT handler

- **Milestone:** M1 — CLI MVP
- **Type:** feature
- **Priority:** P0
- **Complexity Estimate:** M (60–90 minutes)
- **Risk:** Low

## Objective
Implement the complete user-facing command-line application in `crates/diskgoblin-cli`, including `clap` argument parsing, live `stderr` TTY progress updates, `SIGINT` (Ctrl-C) cooperative cancellation, human-readable summary/top-N tables, and `--json` output mode.

## Context
Per [`docs/product-spec.md` §6](../product-spec.md) and [`docs/architecture.md` §2–§3](../architecture.md), `diskgoblin-cli` is the primary deliverable of Milestone `M1`. It must strictly separate `stdout` (final report or JSON) from `stderr` (live progress indicator and fatal error messages) and return standard POSIX exit codes (`0`, `1`, `2`, `130`).

## Scope
- `crates/diskgoblin-cli/Cargo.toml`: Add approved dependencies `clap = { version = "4", features = ["derive"] }`, `ctrlc = "3"`, and `anyhow = "1"`.
- `crates/diskgoblin-cli/src/args.rs`: Clap `#[derive(Parser)]` matching the exact flags in [`docs/product-spec.md` §6](../product-spec.md):
  - `[PATH]` (default `.`)
  - `--apparent-size`
  - `-x, --one-file-system`
  - `-d, --max-depth <DEPTH>` (default `3`)
  - `-n, --top <N>` (default `20`)
  - `--min-size <BYTES>` (default `0`)
  - `--si`
  - `--json`
  - `--show-errors`
  - `-q, --quiet`
  - `-j, --threads <N>` (default `1` in M1)
- `crates/diskgoblin-cli/src/progress.rs`: `StderrProgressSink` that checks `std::io::IsTerminal::is_terminal(&std::io::stderr())` and renders a single-line carriage-return (`\r\x1b[2K`) status update when not `--quiet`, clearing the line when finished.
- `crates/diskgoblin-cli/src/render.rs`: Pure function `render_human_report<W: Write>(report: &ScanReport, opts: &RenderOptions, writer: W) -> io::Result<()>` rendering:
  1. Root summary header (path, allocated size, apparent size, files, dirs, symlinks, duration, error count).
  2. Top Directories table (Rank, Size, `%`, Bar, Error indicator `[!]` if `error_count > 0`, Path).
  3. Top Files table (Rank, Size, `%`, Path).
  4. Error summary footer (and individual error list when `--show-errors` is passed).
- `crates/diskgoblin-cli/src/main.rs`: Wire `ctrlc::set_handler` to `CancelToken::cancel()`, invoke `scan_directory_with_progress`, render output to `stdout`, and map exit codes (`0` success, `1` fatal scan error, `130` `FatalScanError::Cancelled`).

## Out of scope
- Parallel worker thread execution (`--threads` flag is parsed and stored in `ScanOptions::threads`, which uses the sequential scanner until `DG-012`).

## Dependencies
- `DG-004` (Byte formatting & ASCII bars)
- `DG-007` (`CancelToken` & `ProgressSink`)
- `DG-008` (JSON serialization)

## Implementation guidance
- Keep `render_human_report` taking a generic `W: std::io::Write` so unit tests inside `diskgoblin-cli` can render into a `Vec<u8>` buffer and assert exact text formatting without spawning a subprocess.
- Never write progress updates to `stdout` so `diskgoblin <path> --json | jq .` is always valid JSON even in an interactive terminal.

## Acceptance criteria
- [ ] `diskgoblin --help` and `diskgoblin --version` work and document all specified flags.
- [ ] Human report clearly displays root totals, Top Directories with percentage bars, Top Files, and error counts.
- [ ] `--json` writes valid JSON to `stdout` and nothing else to `stdout`.
- [ ] `--si` switches human output from `KiB`/`MiB`/`GiB` to `KB`/`MB`/`GB`.
- [ ] `--show-errors` prints the list of inaccessible paths and error kinds when `report.errors` is non-empty.
- [ ] `FatalScanError::Cancelled` exits with code `130`; `RootNotFound` / `RootUnreadable` exits with code `1`.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-cli
```

## Definition of done
- Unit tests in `args.rs` and `render.rs` pass.
- PR opened linking `DG-009` with Agent Implementation Report.
