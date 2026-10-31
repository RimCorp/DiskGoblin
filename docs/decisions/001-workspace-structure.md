# ADR-001: Cargo Workspace Structure and Crate Boundaries

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Principal Architect, Product Owner

## Context

`PLANNING_CHARTER.md` asks whether a multi-crate Cargo workspace (`diskgoblin-core`, `diskgoblin-cli`, `diskgoblin-desktop`) is warranted immediately, or whether we should start simpler.

We have two competing forces:
1. **Avoiding premature complexity:** Scaffolding an unimplemented `diskgoblin-desktop` crate in `M0` adds Tauri/Node/WebKit system prerequisites, slows CI, and distracts coding agents working on `M0`–`M2` CLI tasks.
2. **Enforcing architectural boundaries for coding agents:** If we start with a single mixed crate containing both CLI (`clap`, terminal formatting, `stdout`/`stderr`) and core scanning logic, autonomous coding agents frequently couple presentation concerns into core domain logic, requiring an expensive refactoring before `M3` (Desktop MVP).

## Decision

We will initialize the repository in `M0` as a **two-crate Cargo workspace**:
- `crates/diskgoblin-core`: Pure library crate containing domain models, platform metadata adapters, filesystem scanners, aggregation algorithms, and human/JSON formatters. Strictly forbidden from depending on `clap` or writing directly to `stdout`/`stderr`.
- `crates/diskgoblin-cli`: Thin binary crate (`diskgoblin`) handling CLI argument parsing (`clap`), `SIGINT` cancellation wiring, `stderr` TTY progress display, and printing reports to `stdout`.

The desktop application (`apps/diskgoblin-desktop` with `src-tauri`) will **not** be scaffolded in `M0`; it will be added as a workspace member at the start of Milestone `M3`.

## Consequences

### Positive
- Enforces compile-time separation between engine logic and CLI presentation from the very first commit.
- Allows autonomous agents to run fast, isolated unit tests via `cargo test -p diskgoblin-core` without compiling CLI dependencies.
- Keeps `M0`–`M2` free of Node.js, Vite, and Tauri system dependencies.
- Adding `diskgoblin-desktop` in `M3` becomes purely additive.

### Negative
- Slightly more initial `Cargo.toml` boilerplate than a single-crate project (mitigated by workspace-level `[workspace.dependencies]` and `[workspace.lints]`).
