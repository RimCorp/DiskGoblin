# DiskGoblin

> **See what’s gobblin’ your disk space — from the terminal to the treemap.**

**DiskGoblin** is a read-only, symlink-safe, hard-link-aware disk space analyzer written in Rust. It pairs a scriptable terminal CLI with an interactive dark desktop workspace built with **React, TypeScript, and `d3-hierarchy`**, runnable either in your browser over a dependency-free localhost HTTP bridge (`diskgoblin --ui`) or as a packaged macOS desktop app (`DiskGoblin.app`) using `AppKit` and `WKWebView`. Built-in factory telemetry (`factory_eval`) evaluates the coding-agent workflow alongside the application itself.

---

## Key Features

### Core & Engine

- **Read-only and symlink-safe:** Traversal uses `std::fs::symlink_metadata` (`lstat`) without modifying scanned files or following symbolic links.
- **Physical and logical sizes:** Tracks allocated bytes (`st_blocks * 512` on Unix) and apparent bytes (`st_size`); select logical sizes with `--apparent-size` in the CLI or toggle between **Allocated** and **Apparent** in memory in the visual UI.
- **Hard-link deduplication:** Counts each Unix `(dev, ino)` identity once toward byte totals and top-file rankings, with deterministic lexicographical first-encounter attribution.
- **Mount boundary control:** Stay on the root filesystem with `-x` / `--one-file-system`.
- **Cooperative cancellation:** `Ctrl-C` or the desktop **Cancel** button signals a shared cancellation token. Synthetic local-fixture tests verify **<100 ms** cancellation; blocked OS filesystem calls cannot be forcibly interrupted mid-syscall.
- **Versioned JSON:** `--json` emits one document with `schema_version: 1` on `stdout`, with progress suppressed.
- **Partial scan errors:** Unreadable descendants are recorded without aborting sibling traversal; inspect paths with `--show-errors` or the desktop error drawer.

On non-Unix platforms, allocated size falls back to apparent size and inode-based hard-link deduplication is unavailable.

### High-Throughput Parallel Traversal

- **Bounded Rayon thread pool:** Choose workers with `-j` / `--threads`; `1` selects the sequential reference engine, `0` selects automatic parallelism capped at eight workers, and larger values request that many workers.
- **Deterministic lexicographical replay:** Parallel discovery replays sorted siblings in the same depth-first order as sequential traversal. Totals, hard-link attribution, rankings, and errors are identical for an unchanged filesystem; elapsed duration naturally varies.
- **Empirical SSD benchmark study:** The [APFS/NVMe study](docs/experiments/ssd-parallel-traversal-study.md) measured up to **2.277×** speedup on its wide-tree fixture. No worker count met the 1.5× threshold across all three fixtures, so **the default remains one thread** per [ADR-005](docs/decisions/005-parallel-traversal-strategy.md).

### Dark Desktop Workspace & Visual UI

- **Three-region workspace layout:**
  - **Whole-scan header summary:** Keeps root scan totals (bytes scanned, file count, folder count, error count) visible while you drill into subdirectories.
  - **Immediate-child treemap and ranked list:** Both the `d3-hierarchy` squarified treemap and the **Largest items** panel (**Folders** and **Files** tabs) are scoped to the currently viewed directory, using the viewed directory's byte total as the percentage bar denominator.
  - **Bottom item inspector:** Single-clicking a tile or row selects it into the inspector bar with its full path, measured size, and a **Reveal in Finder** (`open -R`) action; double-clicking (or pressing `Enter`) drills into a folder in memory without rescanning the disk.
- **Three-tier frontend transport (`ipc.ts`):** Automatically selects Tauri 2 IPC when present, the CLI's `127.0.0.1` HTTP bridge (`diskgoblin --ui` and `DiskGoblin.app`), or deterministic in-memory mock data (`/mock/home`) for standalone previews and tests.

### Autonomous Software Factory Evaluation

The built-in **`factory_eval`** tool validates run telemetry and generates comparative Markdown tables for harness/model configurations, timing, usage, cost, and outcomes. The checked-in dataset contains **22 run records**; the published [comparative findings](docs/experiments/findings.md) analyze the **20-run cohort** (`DG-001`–`DG-015` and `DG-017`), with limitations and recommendations for interpreting the evidence.

---

## Three Execution Modes

1. **Terminal CLI:** `diskgoblin [PATH]` prints disk usage tables; add `--json` for a machine-readable report.
2. **Browser Visual Mode:** `diskgoblin --ui [PATH]` serves the compiled frontend on `127.0.0.1` and opens your default browser. It automatically scans the supplied path using the CLI metric and scan settings. Use `--ui-port 0` for an automatic ephemeral port or `--no-open` to open the printed URL yourself. Port `7878` is the default, with automatic fallback when occupied.
3. **macOS Desktop App (`DiskGoblin.app`):** Built via `npm run package:macos` in `apps/diskgoblin-desktop`, which packages the Rust `diskgoblin` engine and compiled web assets inside a lightweight Swift `AppKit` + `WKWebView` macOS application bundle (`apps/diskgoblin-desktop/dist-macos/DiskGoblin.app`) with custom `AppIcon.icns`, native folder picker, and Finder integration.

---

## Desktop & Visual UI Quick Start

### 1. Build and Package the macOS Desktop App (`DiskGoblin.app`)

Requires macOS (Xcode Command Line Tools / `swiftc`), Rust (`1.85+`), and Node.js (`20+`):

```bash
cd apps/diskgoblin-desktop
npm install
npm run package:macos

# Launch the packaged macOS app (defaults to your home directory, or pass a path)
open dist-macos/DiskGoblin.app
# Or pass a specific target folder directly to the app executable:
./dist-macos/DiskGoblin.app/Contents/MacOS/DiskGoblin /path/to/scan
```

### 2. Launch Visual Mode in Your Browser (`diskgoblin --ui`)

```bash
cd apps/diskgoblin-desktop
npm install

# Build the frontend and launch the CLI localhost UI server
npm run ui -- /path/to/scan
```

Or build the frontend once (`npm run build` in `apps/diskgoblin-desktop`) and invoke the CLI directly from anywhere:

```bash
diskgoblin --ui /path/to/scan
```

### 3. Frontend Development & Tests

```bash
cd apps/diskgoblin-desktop
npm install

# Run the Vitest component & math test suite once
npm test -- --run

# Type-check and build the production frontend bundle
npm run build

# Start Vite dev server (uses mock data unless proxied to `diskgoblin --ui --no-open`)
npm run dev
```

> **Note on `src-tauri`:** The repository also includes a Tauri 2 Rust command backend in [`apps/diskgoblin-desktop/src-tauri`](apps/diskgoblin-desktop/src-tauri) exposing the same scan, cancel, and progress contract. Its native Tauri crate dependencies are currently feature-gated in `src-tauri/Cargo.toml` so headless workspace verification compiles without external system GUI libraries, while `npm run package:macos` (`AppKit` + `WKWebView`) provides the primary macOS desktop app bundle.

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
| [`crates/diskgoblin-cli`](crates/diskgoblin-cli) | Thin `diskgoblin` binary: argument parsing, terminal tables, JSON output, progress, SIGINT handling, and the `127.0.0.1` `--ui` HTTP bridge; also hosts the `factory_eval` binary. |
| [`apps/diskgoblin-desktop`](apps/diskgoblin-desktop) | React 18 + TypeScript + Vite frontend, `d3-hierarchy` squarified treemap, macOS `AppKit`/`WKWebView` bundle builder (`scripts/package-macos-app.sh`), and `src-tauri` Rust command bindings. |

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
- [Initial M0 planning charter](PLANNING_CHARTER.md)

---

## License

Licensed under the [MIT License](LICENSE).
