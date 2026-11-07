# DiskGoblin

> **See what’s gobblin’ your disk space — from the terminal to the treemap.**

**DiskGoblin** is a read-only, symlink-safe, hard-link deduplicated disk space analyzer written in Rust, with a high-performance CLI and an interactive desktop application built with **Tauri 2, React, and D3**. A shared engine tracks physical allocation and logical apparent size, supports deterministic parallel traversal, and delivers terminal tables, versioned JSON reports, and desktop treemaps. Built-in factory telemetry evaluates the autonomous development workflow alongside the application itself. The CLI can serve the same frontend over a localhost HTTP bridge; native shell integration remains gated as described below.

---

## Key Features

### Core & Engine

- **Read-only and symlink-safe:** Traversal uses `std::fs::symlink_metadata` (`lstat`) without modifying scanned files or following symbolic links.
- **Physical and logical sizes:** Tracks allocated bytes (`st_blocks * 512` on Unix) and apparent bytes (`st_size`); select logical sizes with `--apparent-size`.
- **Hard-link deduplication:** Counts each Unix `(dev, ino)` identity once toward byte totals and top-file rankings, with stable first-encounter attribution.
- **Mount boundary control:** Stay on the root filesystem with `-x` / `--one-file-system`.
- **Cooperative cancellation:** `Ctrl-C` or the desktop Cancel button signals a shared cancellation token. Synthetic local-fixture tests verify **<100 ms** cancellation; blocked OS filesystem calls cannot be forcibly interrupted.
- **Versioned JSON:** `--json` emits one document with `schema_version: 1` on `stdout`, with progress suppressed.
- **Partial scan errors:** Unreadable descendants are recorded without aborting sibling traversal; inspect paths with `--show-errors` or the desktop error drawer.

On non-Unix platforms, allocated size falls back to apparent size and inode-based hard-link deduplication is unavailable.

### High-Throughput Parallel Traversal

- **Bounded Rayon thread pool:** Choose workers with `-j` / `--threads`; `1` selects the sequential reference engine, `0` selects automatic parallelism capped at eight workers, and larger values request that many workers.
- **Deterministic lexicographical replay:** Parallel discovery replays sorted siblings in the same depth-first order as sequential traversal. Totals, hard-link attribution, rankings, and errors are identical for an unchanged filesystem; elapsed duration naturally varies.
- **Empirical SSD benchmark study:** The [APFS/NVMe study](docs/experiments/ssd-parallel-traversal-study.md) measured up to **2.277×** speedup on its wide-tree fixture. No worker count met the 1.5× threshold across all fixtures, so **the default remains one thread**.

### Interactive Desktop App — Tauri 2 + React + D3

- **Interactive SVG squarified treemap:** `d3-hierarchy` computes the layout, rendered by React with hover details and breadcrumb drill-down.
- **Scan controls:** Folder picker, editable path, and allocated/apparent size selection.
- **Live progress banner and Cancel:** Tracks scan counts, bytes, elapsed time, and errors through the shared engine's progress interface.
- **Top directories and files tables:** Ranked results complement the treemap, with a **partial scan error drawer** for inaccessible paths and reasons.

The frontend selects native Tauri IPC, the localhost HTTP bridge, or deterministic mock IPC for a standalone preview. The Rust backend and Tauri command wiring are present, but the native WebView shell still requires integration; see the quick start below.

### Autonomous Software Factory Evaluation

The built-in **`factory_eval`** tool validates run telemetry and generates comparative Markdown tables for harness/model configurations, timing, usage, cost, and outcomes. The checked-in dataset contains **22 run records**; the published [comparative findings](docs/experiments/findings.md) analyze the original **20-run cohort**, with limitations and recommendations for interpreting the evidence.

---

## Three Execution Modes

1. **Terminal CLI:** `diskgoblin [PATH]` prints disk usage tables; add `--json` for a machine-readable report.
2. **Visual Web UI:** `diskgoblin --ui [PATH]` serves the frontend on localhost and opens your browser. It automatically scans the supplied path using the CLI metric and scan settings. Use `--ui-port 0` for an automatic port or `--no-open` to open the printed URL yourself. Port 7878 is the default, with automatic fallback when occupied. Stop the server with Ctrl-C.
3. **Native Desktop App:** `cd apps/diskgoblin-desktop && npm run tauri dev` is the native entry point, subject to the existing runtime dependency gate described below.

Build the visual frontend first with `cd apps/diskgoblin-desktop && npm install && npm run build`, then launch `diskgoblin --ui /path/to/scan`. Alternatively, run `npm run ui` from `apps/diskgoblin-desktop` to build and launch together (or `npm run ui -- /path/to/scan` to scan a specific target directory). Set `DISK_GOBLIN_UI_DIST` to serve a custom frontend build. Without built assets, the server provides a built-in scan page.

During frontend development, `npm run dev` proxies `/api` to a separately running `diskgoblin --ui --no-open` on port 7878. A standalone `npm run preview` keeps the synthetic demo usable at `/mock/home`; real scans require the bridge or Tauri runtime.

## Desktop App Quick Start

Use Node.js and npm for the browser preview and frontend checks. Native development additionally requires Rust and the platform's Tauri 2 build prerequisites.

```bash
cd apps/diskgoblin-desktop
npm install

# Browser preview with mock IPC and synthetic scan data
npm run dev

# Native desktop development entry point (see current limitation below)
npm run tauri dev

# Desktop Vitest suite (watch mode)
npm test

# Type-check and build the frontend
npm run build
```

> **Native shell status:** `npm run tauri dev` is the configured native entry point, but it is not runnable as checked in: the Tauri and dialog-plugin dependencies in `src-tauri/Cargo.toml` are commented out and runtime integration remains gated. Browser preview and frontend tests work independently of that shell. The preview's folder picker and scans use mock data rather than reading your disk.

---

## CLI Quick Start

### Prerequisites

- **Rust toolchain** (`1.85+` recommended, `cargo` and `rustc`)

### Run Directly via Cargo

From the repository root, run `diskgoblin` against any directory (defaults to `.`). Specify `--bin diskgoblin` because the CLI package also contains `factory_eval`:

```bash
# Scan the current directory (top 20 directories and files by allocated size)
cargo run -p diskgoblin-cli --bin diskgoblin -- .

# Scan a specific path showing top 10 entries using apparent size
cargo run -p diskgoblin-cli --bin diskgoblin -- ./docs --top 10 --apparent-size

# Build and run the optimized release binary
cargo build --release -p diskgoblin-cli
./target/release/diskgoblin /path/to/analyze
```

### Install Locally

```bash
cargo install --path crates/diskgoblin-cli
diskgoblin --help
```

---

## CLI Usage & Options

```text
Usage: diskgoblin [OPTIONS] [PATH]
```

`PATH` accepts a directory or file and defaults to `.`.

| Flag | Description | Default |
| :--- | :--- | :--- |
| `--ui` | Launch the visual web UI on localhost | Off |
| `--ui-port <PORT>` | UI server port; `0` selects an automatic port | `7878` |
| `--no-open` | Skip opening the browser in UI mode | Off |
| `--apparent-size` | Rank and display logical size instead of physical allocation | Allocated |
| `-x`, `--one-file-system` | Do not recurse across filesystem device boundaries | Off |
| `-d`, `--max-depth <DEPTH>` | Maximum directory depth included in reports; deeper contents still contribute to totals | `3` |
| `-n`, `--top <N>` | Number of largest directories and files to display | `20` |
| `--min-size <BYTES>` | Omit entries below this byte threshold from top lists | `0` |
| `--si` | Decimal SI units (KB, MB, GB) instead of binary IEC (KiB, MiB, GiB) | Off |
| `--json` | Emit a versioned JSON report on `stdout` | Off |
| `--show-errors` | List inaccessible paths and error reasons in text output | Off |
| `-q`, `--quiet` | Suppress non-error progress on `stderr` | Off |
| `-j`, `--threads <N>` | Fully deterministic execution: `1` sequential, `0` automatic (at most eight workers), `N > 1` bounded parallel workers | `1` |
| `-h`, `--help` | Print help | — |
| `-V`, `--version` | Print version | — |

### Common Examples

```bash
# 1. Inspect the 5 largest directories and files in ./docs using apparent size
diskgoblin ./docs --top 5 --apparent-size

# 2. Scan a home folder without crossing mount boundaries, omitting entries under 10 MiB
diskgoblin ~ -x --min-size 10485760 --top 15

# 3. Scan with decimal SI units (KB/MB/GB) and list any permission-denied paths
diskgoblin /var/log --si --show-errors

# 4. Emit a structured JSON report and pipe to jq
diskgoblin ./crates --json | jq '.root.summary'

# 5. Use four workers with deterministic totals and rankings
diskgoblin ./crates --threads 4
```

---

## Sample Output

### Human-Readable Terminal Report

Illustrative output; sizes, counts, and duration depend on the scanned tree.

```text
$ diskgoblin ./docs --top 5 --apparent-size
Root: ./docs
Allocated: 172.0 KiB | Apparent: 134.5 KiB
Files: 30 | Dirs: 3 | Symlinks: 0 | Duration: 0 ms | Errors: 0

Top Directories (Apparent)
Rank         Size       %  Bar                   Error  Path
   1     57.6 KiB   42.9%  █████████░░░░░░░░░░░         ./docs/backlog
   2     18.5 KiB   13.8%  ███░░░░░░░░░░░░░░░░░         ./docs/decisions
   3      6.9 KiB    5.1%  █░░░░░░░░░░░░░░░░░░░         ./docs/experiments

Top Files (Apparent)
Rank         Size       %  Path
   1     14.6 KiB   10.9%  ./docs/architecture.md
   2     11.1 KiB    8.2%  ./docs/implementation-plan.md
   3     11.0 KiB    8.2%  ./docs/product-spec.md
   4      8.6 KiB    6.4%  ./docs/factory-design.md
   5      6.9 KiB    5.1%  ./docs/experiments/experiment-plan.md

Errors: 0 inaccessible paths
```

### Exit Codes

| Exit Code | Meaning |
| :--- | :--- |
| `0` | Scan completed (including scans with non-fatal unreadable subdirectories reported in `Errors`) |
| `1` | Fatal scan error (e.g., root path does not exist or cannot be accessed) |
| `2` | Invalid CLI arguments or flags |
| `130` | Scan cancelled cooperatively via `SIGINT` (`Ctrl-C`) |

---

## Factory Telemetry Evaluator

From the repository root:

```bash
cargo run -p diskgoblin-cli --bin factory_eval -- docs/experiments/runs/
```

The evaluator loads and validates the directory's JSON run records, then prints a GitHub-Flavored Markdown comparison and per-run ledger to `stdout`. Invalid or unreadable records produce an error on `stderr` and exit code `1`; successful evaluation exits `0`. Redirect stdout to save a report.

Read the [comparative findings](docs/experiments/findings.md) for the published cohort, methodology, confounders, and follow-ups. Regenerating the report uses all **22 current records**, so its totals differ from the original 20-run study.

---

## Workspace Architecture & Quality Gates

Three components share the scanning engine, following [ADR-001](docs/decisions/001-workspace-structure.md) and [ADR-006](docs/decisions/006-desktop-and-visualization.md):

| Component | Responsibility |
| :--- | :--- |
| [`crates/diskgoblin-core`](crates/diskgoblin-core) | Pure Rust library: domain models, platform metadata, sequential and parallel traversal, cancellation/progress, aggregation, hard-link deduplication, formatting, JSON, and factory metrics. No CLI parsing or direct terminal output. |
| [`crates/diskgoblin-cli`](crates/diskgoblin-cli) | Thin `diskgoblin` binary: argument parsing, terminal tables, JSON output, progress, and SIGINT handling; also hosts the `factory_eval` binary. |
| [`apps/diskgoblin-desktop`](apps/diskgoblin-desktop) | React + TypeScript + Vite frontend, D3 treemap, and a separate `src-tauri` Rust backend depending directly on the core library. |

The root Cargo workspace contains the two crates. The desktop backend currently declares its own Cargo workspace, so root workspace checks do not cover it.

### Quality Gates

From the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

For the desktop frontend, after `npm install`:

```bash
cd apps/diskgoblin-desktop
npm test -- --run
npm run build
```

`npm test` starts Vitest watch mode; `-- --run` runs the suite once for verification.

---

## Documentation Index

- [Product specification](docs/product-spec.md)
- [Architecture specification](docs/architecture.md)
- [Architecture decision records](docs/decisions/)
- [Implementation plan & backlog](docs/implementation-plan.md)
- [Issue specifications](docs/backlog/)
- [Testing strategy](docs/testing-strategy.md)
- [SSD parallel traversal study](docs/experiments/ssd-parallel-traversal-study.md)
- [Autonomous software factory design](docs/factory-design.md)
- [Factory experiment plan](docs/experiments/experiment-plan.md)
- [Factory comparative findings](docs/experiments/findings.md)
- [Factory telemetry run records](docs/experiments/runs/)
- [Agent operating contract (`AGENTS.md`)](AGENTS.md)
