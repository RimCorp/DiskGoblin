# DG-015: Implement React + `d3-hierarchy` interactive treemap and breadcrumb drill-down

- **Milestone:** M3 — Desktop MVP
- **Type:** feature
- **Priority:** P2
- **Complexity Estimate:** L (75–120 minutes)
- **Risk:** Medium

## Objective
Implement the pure TypeScript treemap layout module (`d3-hierarchy` squarified treemap) and the interactive React SVG `<TreemapView />` component with directory zoom drill-down, breadcrumb navigation, and hover tooltips in `apps/diskgoblin-desktop`.

## Context
Per [`ADR-006`](../decisions/006-desktop-and-visualization.md), we use `d3-hierarchy` (`d3.treemap` with `d3.treemapSquarify`) to compute deterministic rectangle layouts from `DirNode` trees, grouping tiny entries (`< 0.5%` of view area) into an `"Other"` bucket so the DOM stays fast (`< 500` SVG nodes).

## Scope
- `apps/diskgoblin-desktop/package.json`: Add `d3-hierarchy` and `@types/d3-hierarchy` (plus `vitest` for unit testing).
- `apps/diskgoblin-desktop/src/lib/treemapLayout.ts`:
  - Pure function `computeTreemapLayout(rootNode: DirNode, width: number, height: number, metric: SizeMetric): TreemapRect[]`.
  - Groups small sibling directories below `0.5%` threshold into a synthetic `"Other (N directories)"` tile.
- `apps/diskgoblin-desktop/src/components/TreemapView.tsx`:
  - Renders SVG rectangles colored by depth/category, displaying directory label and formatted size when rectangle dimensions permit.
  - Clicking a directory rectangle drills down to set that `DirNode` as the zoomed root.
- `apps/diskgoblin-desktop/src/components/BreadcrumbBar.tsx`:
  - Displays path segments from scan root to current zoomed `DirNode`, allowing one-click navigation back up.
- `apps/diskgoblin-desktop/src/lib/__tests__/treemapLayout.test.ts`: Vitest unit tests verifying layout math, zero-size handling, and small-node bucketing.

## Out of scope
- Top files/directories side tables and error drawer (`DG-016`).

## Dependencies
- `DG-014` (Tauri desktop scaffold & TypeScript types).

## Implementation guidance
- Ensure `computeTreemapLayout` handles `width <= 0`, `height <= 0`, or a `DirNode` with `0` total bytes gracefully by returning an empty array `[]` instead of `NaN` coordinates.

## Acceptance criteria
- [ ] `computeTreemapLayout` produces deterministic `{ id, path, name, bytes, percentage, x0, y0, x1, y1, depth }` rectangles within `[0..width, 0..height]`.
- [ ] Entries smaller than `0.5%` of the current node's total are aggregated into an `"Other"` bucket so total rendered rectangles never exceed bounds.
- [ ] Clicking a node in `<TreemapView />` zooms into that subtree and updates `<BreadcrumbBar />`.
- [ ] Vitest unit tests for `computeTreemapLayout` pass.

## Verification
```bash
cd apps/diskgoblin-desktop && npm test -- --run && npm run build
```

## Definition of done
- All Vitest layout tests and TypeScript build checks pass with zero errors.
- PR opened linking `DG-015` with Agent Implementation Report.
