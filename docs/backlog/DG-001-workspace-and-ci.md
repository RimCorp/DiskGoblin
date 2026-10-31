# DG-001: Initialize Cargo workspace, crates, lint rules, and GitHub Actions CI

- **Milestone:** M0 — Foundation
- **Type:** chore
- **Priority:** P0
- **Complexity Estimate:** S (30–45 minutes)
- **Risk:** Low

## Objective
Initialize the foundational two-crate Rust Cargo workspace (`diskgoblin-core` and `diskgoblin-cli`), configure workspace-wide formatting and Clippy lints, and establish a GitHub Actions CI workflow running on both macOS and Ubuntu.

## Context
Per [`docs/architecture.md`](../architecture.md) and [`ADR-001`](../decisions/001-workspace-structure.md), DiskGoblin starts with a clean separation between its pure library engine (`crates/diskgoblin-core`) and its thin binary wrapper (`crates/diskgoblin-cli`). All downstream issues (`DG-002` through `DG-018`) depend on this workspace structure and CI quality gate.

## Scope
- Root `Cargo.toml` defining `[workspace]` (`resolver = "2"`, members `["crates/diskgoblin-core", "crates/diskgoblin-cli"]`), `[workspace.package]` (edition `2021`, license `MIT`), `[workspace.dependencies]`, and `[workspace.lints]`.
- `crates/diskgoblin-core/Cargo.toml` and minimal stub `crates/diskgoblin-core/src/lib.rs` with a passing smoke unit test.
- `crates/diskgoblin-cli/Cargo.toml` (producing binary named `diskgoblin`, depending on `diskgoblin-core`) and minimal stub `crates/diskgoblin-cli/src/main.rs`.
- `rustfmt.toml` (standard edition 2021 defaults).
- `.github/workflows/ci.yml` running `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace --all-targets` on `ubuntu-latest` and `macos-latest`.

## Out of scope
- Do not scaffold `diskgoblin-desktop` or any Node/Tauri files (deferred to `M3` / `DG-014`).
- Do not implement filesystem scanning, domain models, or CLI argument parsing in this issue.
- Do not add unused third-party dependencies to `Cargo.toml` yet beyond what is needed to compile the workspace stubs.

## Dependencies
- None (First issue on critical path).

## Implementation guidance
- In root `Cargo.toml`, configure `[workspace.lints.rust]` with `unsafe_code = "forbid"` (or allow only where explicitly needed later; none of our planned crates require `unsafe`, as `std::os::unix::fs::MetadataExt` is completely safe Rust) and `[workspace.lints.clippy]` with `all = "warn"`.
- Ensure both `crates/diskgoblin-core/Cargo.toml` and `crates/diskgoblin-cli/Cargo.toml` inherit `[lints] workspace = true`.
- In `crates/diskgoblin-core/src/lib.rs`, export a simple `pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }` and a `#![cfg(test)]` module testing it so `cargo test` executes at least one unit test.

## Acceptance criteria
- [ ] Root `Cargo.toml` configures a Cargo workspace with members `crates/diskgoblin-core` and `crates/diskgoblin-cli`.
- [ ] `crates/diskgoblin-cli` compiles a binary named `diskgoblin` and links `diskgoblin-core`.
- [ ] `unsafe_code = "forbid"` is set in workspace lints.
- [ ] `.github/workflows/ci.yml` triggers on `push` to `main` and `pull_request`, running fmt, clippy (`-D warnings`), and test jobs across `ubuntu-latest` and `macos-latest`.

## Verification
Run the following commands and verify exit code `0`:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p diskgoblin-cli
```

## Definition of done
- All verification commands pass locally with zero warnings.
- `Cargo.lock` is committed for reproducible builds.
- PR opened linking `DG-001` with the structured Agent Implementation Report from `AGENTS.md`.
