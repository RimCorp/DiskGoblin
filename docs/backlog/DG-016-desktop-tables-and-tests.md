# DG-016: Add desktop top-items table, error drawer, and Vitest UI component tests

- **Milestone:** M3 — Desktop MVP
- **Type:** feature
- **Priority:** P2
- **Complexity Estimate:** M (60–90 minutes)
- **Risk:** Low

## Objective
Complete the Milestone `M3` Desktop MVP by adding the Top Directories / Top Files tabbed view, the partial scan error drawer (`ScanErrorRecord` viewer), live scan progress banner with Cancel button, and component tests in `apps/diskgoblin-desktop`.

## Context
Completes Milestone `M3` (Desktop MVP). Synchronizes the `d3-hierarchy` treemap from `DG-015` with tabular views of largest files/directories and exposes any permission/IO errors encountered during scanning.

## Scope
- `apps/diskgoblin-desktop/src/components/ScanToolbar.tsx`: Path input, folder picker button, metric toggle (`Allocated` vs. `Apparent`), Scan / Cancel buttons, and live `ScanProgress` status bar.
- `apps/diskgoblin-desktop/src/components/TopItemsPanel.tsx`: Tabbed table showing `top_directories` (clicking a row focuses that directory in the treemap if present) and `top_files`.
- `apps/diskgoblin-desktop/src/components/ErrorDrawer.tsx`: Expandable badge/drawer displaying `report.errors` (`path`, `kind`, `message`) when `errors.length > 0`.
- `apps/diskgoblin-desktop/src/components/__tests__/`: Vitest component/logic tests using mock `ScanReport` fixtures.

## Out of scope
- File deletion or system mutation actions (strictly non-goal per `PLANNING_CHARTER.md` §2).

## Dependencies
- `DG-015` (Desktop treemap UI).

## Implementation guidance
- Provide a clean mock IPC adapter in `src/lib/ipc.ts` that falls back to a deterministic fixture when running inside Vitest or browser preview outside Tauri runtime.

## Acceptance criteria
- [ ] Users can trigger a scan, view live progress (`files_seen`, `dirs_seen`, `bytes_accumulated`), and cancel an active scan from `<ScanToolbar />`.
- [ ] Users can toggle between `Allocated` and `Apparent` size metrics in the UI.
- [ ] `<TopItemsPanel />` renders `top_directories` and `top_files` with formatted sizes and relative percentage bars.
- [ ] `<ErrorDrawer />` displays the count of unreadable paths and expands to show individual `ScanErrorRecord` items.
- [ ] All Vitest unit/component tests and production TypeScript build pass.

## Verification
```bash
cd apps/diskgoblin-desktop && npm test -- --run && npm run build
cargo test --workspace --all-targets
```

## Definition of done
- Desktop frontend and Rust backend compile and pass all tests; Milestone `M3` (Desktop MVP) is complete.
- PR opened linking `DG-016` with Agent Implementation Report.
