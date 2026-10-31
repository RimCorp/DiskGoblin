# DG-014: Scaffold Tauri 2 desktop app (`diskgoblin-desktop`) and Rust IPC commands

- **Milestone:** M3 — Desktop MVP
- **Type:** feature
- **Priority:** P2
- **Complexity Estimate:** L (60–100 minutes)
- **Risk:** Medium

## Objective
Initialize the Tauri 2 desktop application (`apps/diskgoblin-desktop`) linking `diskgoblin-core`, and implement the Rust backend IPC command handlers (`start_scan`, `cancel_scan`, `pick_directory`) with live `scan:progress` event emission.

## Context
Per [`docs/architecture.md` §3](../architecture.md) and [`ADR-006`](../decisions/006-desktop-and-visualization.md), Milestone `M3` extends DiskGoblin to a graphical desktop app sharing the exact same `diskgoblin-core` engine as the CLI.

## Scope
- `apps/diskgoblin-desktop/src-tauri/`:
  - `Cargo.toml` depending on `diskgoblin-core = { path = "../../../crates/diskgoblin-core" }`, `tauri = "2"`, `tauri-plugin-dialog = "2"`, `serde`, `serde_json`.
  - `src/state.rs`: Thread-safe `ActiveScanState` holding `Mutex<Option<CancelToken>>` so starting a new scan or clicking Cancel cancels any in-flight scan.
  - `src/commands.rs`:
    - `start_scan(app: AppHandle, state: State<ActiveScanState>, options: ScanOptionsDto) -> Result<ScanReport, String>` (runs scan on `tauri::async_runtime::spawn_blocking`, streams `ScanProgress` via `app.emit("scan:progress", progress)`, and returns `ScanReport`).
    - `cancel_scan(state: State<ActiveScanState>) -> Result<(), String>`.
  - Unit tests in `src-tauri` testing `ActiveScanState` lifecycle and command handler logic directly against a `FixtureBuilder` directory without requiring a live display server.
- `apps/diskgoblin-desktop/`: Minimal Vite + React + TypeScript scaffold (`package.json`, `tsconfig.json`, `vite.config.ts`, `src/types/scan.ts` matching `ScanReport` JSON schema).

## Out of scope
- Full treemap visualization (`DG-015`) and top-items/error drawer UI (`DG-016`).

## Dependencies
- `DG-010` (Completed `M1` core & CLI contracts).

## Implementation guidance
- Keep the core state management and scan invocation logic in testable Rust functions separated from `tauri::Window` so `cargo test` inside `apps/diskgoblin-desktop/src-tauri` can verify scan execution and cancellation headlessly in CI.

## Acceptance criteria
- [ ] `apps/diskgoblin-desktop/src-tauri` compiles cleanly against `diskgoblin-core`.
- [ ] `start_scan` executes on a background blocking thread, emits throttled progress updates, and returns a serialized `ScanReport`.
- [ ] `cancel_scan` triggers the active `CancelToken` and aborts an in-progress scan.
- [ ] TypeScript interfaces in `src/types/scan.ts` accurately match `diskgoblin-core`'s `ScanReport` JSON schema.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

## Definition of done
- Backend compiles and passes unit tests; frontend scaffold builds via `npm run build` (or `bun run build`).
- PR opened linking `DG-014` with Agent Implementation Report.
