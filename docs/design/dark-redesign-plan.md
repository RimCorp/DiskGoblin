# DiskGoblin Dark Desktop Redesign — Architecture & Implementation Plan

This document fulfills the **Architect Brief (`architect-brief.md`)** and specifies the concrete implementation plan for the **Coding Brief (`coding-brief.md`)** and visual reference (`design-reference.png`).

---

## 1. Repository Findings

- **Current Stack & Architecture:**
  - `crates/diskgoblin-core`: Pure read-only Rust library (`symlink_metadata` / `lstat` traversal, `st_blocks * 512` physical allocation vs. `st_size` logical apparent size, hard-link `(dev, ino)` deduplication, cooperative `CancelToken`, 50 ms throttled `ScanProgress`, bottom-up `DirNode` aggregation, bounded `top_directories` / `top_files` heaps, and `ScanReport` schema v1).
  - `crates/diskgoblin-cli`: Thin CLI binary (`src/main.rs`, `src/args.rs`, `src/render.rs`, `src/progress.rs`) plus dependency-free localhost HTTP bridge (`src/ui_server.rs`, serving `apps/diskgoblin-desktop/dist` and `/api/config`, `/api/scan`, `/api/progress`, `/api/cancel`, `/api/pick-directory`).
  - `apps/diskgoblin-desktop`: React 18 + TypeScript + `d3-hierarchy` frontend (`src/App.tsx`, `src/lib/ipc.ts`, `src/lib/treemapLayout.ts`, `src/components/{ScanToolbar,BreadcrumbBar,TreemapView,TopItemsPanel,ErrorDrawer}.tsx`) paired with `src-tauri` (`src-tauri/src/{lib.rs,commands.rs,state.rs}`).
- **Packaging & Native Window State:**
  - `apps/diskgoblin-desktop/src-tauri/Cargo.toml` keeps `tauri` gated behind `tauri-runtime` so headless workspace tests run offline in `< 1s`.
  - To deliver an **installed native macOS desktop application (`DiskGoblin.app`)** that opens its own native window with real macOS traffic-light controls and custom icon **without launching an external browser**, we retain the Tauri 2 command/IPC boundary in `src-tauri` while adding a native macOS `WKWebView` + `AppKit` bundle builder (`apps/diskgoblin-desktop/scripts/package-macos-app.sh` / `npm run package:macos`) that compiles a native Cocoa/WebKit host binary (`swiftc -framework Cocoa -framework WebKit`), generates `AppIcon.icns`, embeds the production frontend and `diskgoblin` engine inside `DiskGoblin.app`, and opens a standalone native macOS desktop window.
- **Validation Commands:**
  - Rust formatting: `CARGO_HOME=/tmp/dg009-cargo-home cargo fmt --all -- --check`
  - Rust Clippy: `CARGO_HOME=/tmp/dg009-cargo-home cargo clippy --offline --workspace --all-targets -- -D warnings`
  - Rust tests: `CARGO_HOME=/tmp/dg009-cargo-home cargo test --offline --workspace --all-targets`
  - Desktop backend tests: `CARGO_HOME=/tmp/dg009-cargo-home cargo test --offline --manifest-path apps/diskgoblin-desktop/src-tauri/Cargo.toml`
  - Frontend tests & build: `cd apps/diskgoblin-desktop && npm test -- --run && npm run build`

---

## 2. Information Model & Explicit State Decisions

### 2.1 Five Distinct State Concepts (`App.tsx`)
1. **Scan Root (`scanRootPath` + `report.root`):**
   - The directory path submitted to `startScan`.
   - The header summary strip (`1.2 GiB scanned · 382 files · 272 folders · 0 errors`) **always** describes `report.root.summary` (the entire scan root) and is explicitly labeled as whole-scan totals.
2. **Currently Viewed Folder (`viewedFolder: DirNode` + `zoomChain: DirNode[]`):**
   - Defaults to `report.root` when a scan completes.
   - Drilling into a child directory updates `zoomChain` / `viewedFolder` in memory **without starting a new filesystem scan**.
   - Both the **BreadcrumbBar**, the **TreemapView**, and the **Largest items** list are scoped to `viewedFolder`.
3. **Selected Item (`selectedItem: SelectedItem | null`):**
   - Represents the currently highlighted child in `viewedFolder` (`kind: "directory" | "file" | "aggregate"`, `name`, `path`, `apparent_size`, `allocated_size`, `isAggregate`, `groupedCount`, `node?: DirNode`).
   - **Single-clicking** a treemap tile or a Largest items row updates `selectedItem` **without navigating**.
   - **Double-clicking** (or pressing `Enter` when focused) on a directory tile or directory row drills into that directory (`zoomTo(node)`) and resets or updates selection to the newly viewed folder's first child (or clears selection).
   - Selection is synchronized between `TreemapView` and `TopItemsPanel` by matching item `id`/`path`, and is feeding the bottom **Inspector Bar**.
4. **Measurement Mode (`metric: SizeMetric = "allocated" | "apparent"`):**
   - Toggling between `Allocated` and `Apparent` immediately recomputes treemap areas, list ordering, percentage bar widths, whole-scan summary bytes, breadcrumb sizes, and inspector size in memory using the dual `allocated_size` and `apparent_size` fields already stored on every `DirSummary` and `FileEntry` — **no rescan required**.
5. **Scan Lifecycle (`lifecycle: "idle" | "scanning" | "complete" | "cancelled" | "error"`):**
   - During a rescan (`scanning`), the previous report is cleared (or marked stale/replaced by the scanning state) so stale data is never presented as newly completed.
   - Generation tracking in `ActiveScanState` / `ui_server.rs` / `ipc.ts` ensures a late or cancelled scan never overwrites a newer scan.

### 2.2 Immediate-Children Scope, Direct Files, Zero-Size Items, and "Other" Aggregation
- **No Mixed Root/Descendant List:** Unlike the static concept art where ancestor folders (`superwhisper`, `recordings`) appeared alongside children, `TopItemsPanel` (now **Largest items**) strictly lists **immediate children of `viewedFolder`**:
  - **`Folders` filter:** Immediate child directories (`viewedFolder.children`), sorted by descending `metricSize(child, metric)` then ascending `path`. Zero-byte immediate child directories remain at the bottom of the list so they stay discoverable.
  - **`Files` filter:** Immediate child files of `viewedFolder` (all `FileEntry` items in `report.top_files` whose parent directory equals `viewedFolder.summary.path`, with `top_n: 500` requested by the desktop UI). If `viewedFolder` contains additional unranked direct file bytes (`residualBytes = Math.max(0, directFilesTotal - sumOfRankedDirectFiles)`), a clearly disclosed aggregate row `Other direct files (N files)` (`isAggregate: true`) is appended so totals reconcile without pretending to be a single file path.
- **Consistent Bar Denominator:**
  - Every item's percentage bar in `Largest items` uses `viewedFolderTotal = metricSize(viewedFolder, metric)` as its denominator (`relativePercent(itemBytes, viewedFolderTotal)`). Switching between `Folders` and `Files` filters never changes the denominator or meaning of the bar.
- **Single-Level Immediate-Child Treemap (No Double Counting):**
  - `computeTreemapLayout(viewedFolder, width, height, metric, directFiles)` maps only the **immediate children** of `viewedFolder` (`depth = 0`):
    - Each immediate child directory (`viewedFolder.children`) with `bytes > 0`.
    - Each immediate direct file in `viewedFolder` (or a combined `Direct files` / `Other` bucket for tiny entries `< 0.5%` of `viewedFolderTotal`).
    - Entries below `SMALL_ENTRY_RATIO` (`0.5%` of `viewedFolderTotal`) are grouped into a single `Other (N items)` tile with `isOtherBucket: true` (`isAggregate: true`).
  - Because only immediate children are laid out (`MAX_DEPTH_LEVELS = 1`, `paddingInner = 4`), tile areas sum to `100%` of `viewedFolderTotal` with zero descendant double-counting.
- **Search Control Omission:**
  - Per `architect-brief.md` ("Omit the mockup's search icon unless functional search is in scope and defined") and `coding-brief.md`, the decorative search icon in the mockup is omitted.

---

## 3. Component Boundaries & File Changes

1. **`crates/diskgoblin-cli/src/ui_server.rs` & `apps/diskgoblin-desktop/src-tauri/src/{commands.rs,lib.rs}`:**
   - Add `POST /api/reveal` (and `reveal_path` in `src-tauri/src/commands.rs`) that validates path existence via `std::fs::symlink_metadata` and opens the path in the native file manager (`open -R <path>` on macOS, `xdg-open` on Linux, `explorer /select,` on Windows).
2. **`apps/diskgoblin-desktop/src/lib/ipc.ts`:**
   - Add `revealPath(path: string, fetchImpl?: typeof fetch): Promise<void>` supporting Tier 1 (Tauri `invoke("reveal_path")`), Tier 2 (`POST /api/reveal`), and Tier 3 (mock tracking `getLastRevealedPath()` for tests).
   - Ensure `createMockScanReport` includes direct files inside `/mock/home` as well as inside subdirectories so both `Folders` and `Files` tabs have rich immediate children at the root and after drilling in.
3. **`apps/diskgoblin-desktop/src/lib/treemapLayout.ts`:**
   - Update `computeTreemapLayout` to lay out the immediate children of `viewedFolder` (child folders + optional direct files/residual direct file bytes) with `SMALL_ENTRY_RATIO` (`0.5%`) `"Other"` aggregation, deterministic stable color-family assignment (`colorIndex` / `fillForPath`), and zero double-counting.
   - Add helper `getImmediateFilesForNode(node: DirNode, report: ScanReport | null, metric: SizeMetric)` to extract immediate child files and any residual direct-file aggregate for `node`.
4. **`apps/diskgoblin-desktop/src/components/ScanToolbar.tsx`:**
   - Redesign as the compact dark Header Bar (`#181C1B` / `#232927`, lime goblin mark + `DiskGoblin`, scan root path input/badge, `Choose folder…`, `Scan` / `Rescan` + `Cancel` when scanning, `Allocated` / `Apparent` segmented toggle with `#C4E878` active pill, and whole-scan summary strip `1.2 GiB scanned · 382 files · 272 folders · 0 errors`).
5. **`apps/diskgoblin-desktop/src/components/BreadcrumbBar.tsx`:**
   - Dark surface breadcrumb above the treemap with folder icon, clickable ancestor segments, bold current viewed folder (`aria-current="page"`), and formatted size badge.
6. **`apps/diskgoblin-desktop/src/components/TreemapView.tsx`:**
   - Dark SVG treemap with muted teal (`#35766E`), olive (`#687A3A`), slate-teal (`#3E757B`), ochre (`#9E6B2B`), and sage (`#7F7837`) palette; `2.5px` lime (`#C4E878`) outline + top-right `✓` badge on the selected tile; adaptive two-line labels (bold item name + size below) that hide cleanly when a tile is too small; single-click to select (`onSelect`), double-click or `Enter`/`Space` to drill into a folder (`onZoom`).
7. **`apps/diskgoblin-desktop/src/components/TopItemsPanel.tsx`:**
   - Right-hand **Largest items** panel with `Folders` and `Files` segmented tabs (`role="tablist"`), scrollable list of immediate children of `viewedFolder`, rank numbers (`1.`, `2.`), folder/file icons, lime progress bars relative to `viewedFolder` total, tabular byte sizes, single-click selection (`aria-selected` / `data-selected`), and double-click / `Enter` folder drill-down.
8. **`apps/diskgoblin-desktop/src/components/InspectorBar.tsx` (or integrated in `App.tsx`) & `ErrorDrawer.tsx`:**
   - Bottom selected-item inspector showing icon, item name, full monospace path (`title` + truncation), selected metric size, and `Reveal in Finder` button (disabled when no selection or when an aggregate bucket is selected).
   - Bottom status footer showing `Read-only` indicator, scan lifecycle message, and dark-themed collapsible `ErrorDrawer` when unreadable paths exist.
9. **`apps/diskgoblin-desktop/scripts/package-macos-app.sh` & `package.json`:**
   - Build a standalone native macOS application bundle (`apps/diskgoblin-desktop/dist-macos/DiskGoblin.app`) with a native Cocoa + `WKWebView` executable, custom `AppIcon.icns`, native macOS traffic-light window controls, default `1440×900` window size (minimum `960×620`), and built-in `DISK_GOBLIN_SCREENSHOT_PATH` snapshot support for automated visual verification at `1440×900` and `1100×720`.
