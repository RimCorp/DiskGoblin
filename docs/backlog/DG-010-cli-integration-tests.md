# DG-010: Add end-to-end CLI integration test suite and exit-code verification

- **Milestone:** M1 — CLI MVP
- **Type:** chore
- **Priority:** P0
- **Complexity Estimate:** M (45–75 minutes)
- **Risk:** Low

## Objective
Create an end-to-end black-box integration test suite (`crates/diskgoblin-cli/tests/cli_integration.rs`) that executes the compiled `diskgoblin` binary against synthetic `FixtureBuilder` trees and verifies all Milestone `M1` MVP capabilities, CLI flags, stdout/stderr separation, and exit codes.

## Context
Completes Milestone `M1` (CLI MVP). Before moving to performance optimization (`M2`) or desktop UI (`M3`), we require an automated end-to-end test suite proving the `diskgoblin` binary satisfies all 9 MVP capabilities in [`PLANNING_CHARTER.md` §2](../../PLANNING_CHARTER.md).

## Scope
- `crates/diskgoblin-cli/Cargo.toml`: Add `[dev-dependencies]` `assert_cmd = "2"`, `predicates = "3"`, `serde_json = "1"`, and `tempfile = "3"` (plus `diskgoblin-core` with `test-utils` feature if gated).
- `crates/diskgoblin-cli/tests/cli_integration.rs` covering:
  1. Default human output on a known nested fixture (`exit code 0`, expected directory and file names in `stdout`).
  2. `--json` output parsed via `serde_json::from_slice` into `ScanReport`, verifying exact file/dir counts, hard-link deduplication, and symlink counts.
  3. `--apparent-size`, `--si`, `--top 2`, `--max-depth 1`, and `--min-size` filtering.
  4. Partial failure handling on an unreadable child directory (`exit code 0`, error count reported, `--show-errors` lists the restricted path).
  5. Fatal error on nonexistent root path (`exit code 1`, diagnostic message on `stderr`, empty `stdout`).
  6. Invalid CLI flag (`exit code 2`).

## Out of scope
- Modifying core scanning algorithms (unless fixing a bug uncovered by integration tests).

## Dependencies
- `DG-009` (`diskgoblin-cli` implementation).

## Implementation guidance
- Use `assert_cmd::Command::cargo_bin("diskgoblin")` to invoke the binary in subprocess mode.
- Guard the `chmod 000` unreadable child directory test with a non-root check (`uid != 0`) so container runs do not fail.

## Acceptance criteria
- [ ] All 6 end-to-end test scenarios pass on both macOS and Ubuntu.
- [ ] `diskgoblin <fixture> --json` produces 0 bytes on `stdout` outside the valid JSON document.
- [ ] Milestone `M1` exit codes (`0`, `1`, `2`) are explicitly asserted.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

## Definition of done
- Full workspace test suite passes on CI (`macos-latest` and `ubuntu-latest`).
- Milestone `M1` (CLI MVP) is complete.
- PR opened linking `DG-010` with Agent Implementation Report.
