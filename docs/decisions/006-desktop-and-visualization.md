# ADR-006: Desktop Integration (Tauri 2) and Treemap Visualization Strategy

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Principal Architect, Product Owner

## Context

Milestone `M3` adds a desktop GUI using **Tauri 2** and **TypeScript + React + Vite**, including an interactive treemap visualization. `PLANNING_CHARTER.md` asks us to evaluate D3 (`d3-hierarchy`), ECharts, or a lightweight treemap library, and define how the CLI and desktop share `diskgoblin-core`.

### Evaluation of Visualization Options

| Option | Footprint (gzipped) | Pros | Cons |
|---|---|---|---|
| **Apache ECharts** | ~300+ KB | Out-of-the-box treemap animations, tooltips, breadcrumbs. | Heavy dependency; imperative canvas config is awkward to customize for file-path details and custom keyboard/accessibility interactions; agents frequently struggle with complex ECharts option types. |
| **`d3-hierarchy` (squarified treemap layout) + React SVG/HTML** | ~6 KB (`d3-hierarchy` only) | Pure, deterministic math function (`treemap().size([w, h])`) computing `{x0, y0, x1, y1}` rectangles; rendered directly as standard React components (`<svg>` or positioned `<div>`s); trivial to unit-test in Vitest without a DOM canvas; zero bloat. | Requires writing ~100 lines of React rendering, hover tooltip, and breadcrumb drill-down state (which is straightforward and high-clarity for coding agents). |

## Decision

1. **Shared Engine via Direct Rust Dependency:**
   - `apps/diskgoblin-desktop/src-tauri/Cargo.toml` depends directly on `diskgoblin-core = { path = "../../../crates/diskgoblin-core" }`.
   - It exposes three Tauri commands:
     - `start_scan(options: ScanOptions)` -> spawns scan on a background thread, emits `scan:progress` events via a Tauri `ProgressSink` implementation, and returns `ScanReport`.
     - `cancel_scan()` -> triggers the active `CancelToken`.
     - `pick_directory()` -> opens native folder picker dialog (`tauri-plugin-dialog`).

2. **Treemap Visualization via `d3-hierarchy` + React SVG:**
   - We select **`d3-hierarchy` (`d3.treemap` with `d3.treemapSquarify`)** rendered with declarative React SVG components rather than full ECharts.
   - To keep DOM node counts fast (< 500 rectangles per view), the treemap renders up to 2 levels below the currently zoomed directory node and groups tiny sibling entries (`< 0.5%` of current view area) into an `"Other (N items)"` bucket.

## Consequences

### Positive
- Extremely lightweight frontend bundle (< 6 KB layout engine vs. 300+ KB ECharts).
- Layout calculation is a pure TypeScript function that agents can deterministically unit-test with Vitest (`expect(computeTreemapNodes(mockDirNode, 800, 600)).toMatchSnapshot()`).
- Zero code duplication between CLI and Desktop for scanning, aggregation, or size formatting.

### Negative
- No built-in canvas zoom morphing animation out of the box (clean instant drill-down with breadcrumb navigation is preferred for responsiveness).
