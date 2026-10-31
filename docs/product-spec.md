# DiskGoblin — Product Specification

## 1. Product Vision & Goals

**DiskGoblin** is an open-source, high-performance disk-space analyzer built in Rust. It empowers developers and power users to quickly understand where disk storage is consumed, locate oversized files and directories, and inspect storage hierarchies without risking filesystem mutations.

In addition to its utility as a developer tool, DiskGoblin serves as a controlled benchmark and testbed for an **AI-Assisted Software Factory**—evaluating how autonomous coding agents (OpenCode, Codex CLI) execute bounded engineering tasks from specification to verified pull requests.

### Primary Product Goals
1. **Speed & Responsiveness:** Traverse hundreds of thousands of filesystem entries quickly while providing responsive progress updates and clean cancellation.
2. **Safety by Default:** Strictly read-only in MVP. Never follow symbolic links into infinite loops, never traverse across unexpected device mounts unless explicitly requested, and never modify or delete user files.
3. **Accuracy & Transparency:** Report both **allocated physical disk usage** (blocks consumed on disk, matching `du`) and **logical file size** (apparent byte length), deduplicating hard links within a scan so totals never exceed physical reality.
4. **Resilience to Partial Failures:** Gracefully handle permission errors (`EACCES`, `EPERM`), vanishing files (`ENOENT`), and unreadable metadata—surfacing structured warning summaries alongside partial scan results rather than aborting.
5. **Dual-Interface Architecture:** Provide a standalone, scriptable CLI first (`M1`/`M2`), powered by a pure library engine (`diskgoblin-core`) that directly supports a Tauri 2 desktop GUI (`M3`) without duplicating traversal or aggregation logic.
6. **Deterministic Testability:** Every calculation, sorting rule, formatting helper, and error pathway must be deterministically verifiable using synthetic temporary fixtures without touching host system directories.

---

## 2. Target Audience & Platform Scope

- **Primary Users:** Software engineers, systems administrators, and power users diagnosing disk usage on local workstations or servers.
- **Initial Target Platform:** **macOS** (Apple Silicon & Intel, APFS), where Xcode caches, Docker layers, `node_modules`, and `target/` directories routinely consume ratusan of gigabytes and exhibit sparse files, hard links, and TCC permission barriers (`Library/Containers`, `.Trash`, etc.).
- **Portability Requirement:** First-class support for **Linux** (ext4, XFS, Btrfs) in core and CI (`ubuntu-latest` + `macos-latest`), and clean `cfg(unix)` / `cfg(windows)` boundaries in `diskgoblin-core` so Windows support can be added without redesigning domain models.

---

## 3. MVP Capabilities (`M1` — CLI MVP)

The MVP delivers a fast, self-contained command-line executable (`diskgoblin`) and core library (`diskgoblin-core`).

### 3.1 Core Scanning & Aggregation
- **Recursive Directory Traversal:** Scan any user-specified root path (defaults to current working directory `.`).
- **Dual Size Accounting:**
  - **Allocated Size (default on Unix):** Actual disk space consumed (`blocks * 512` via `std::os::unix::fs::MetadataExt::blocks`), accurately reflecting sparse files and filesystem block allocation.
  - **Apparent (Logical) Size (`--apparent-size`):** Byte length (`metadata.len()`).
- **Hard-Link Deduplication:** Track `(dev, ino)` pairs on Unix for files with `nlink > 1`. Count a hard-linked file's size only on its first encountered path within a scan; mark subsequent paths as deduplicated links so directory totals remain mathematically sound.
- **Symlink Safety:** Never follow symbolic links during recursive traversal (preventing cycles and out-of-tree escapes). Record symlinks as `EntryKind::Symlink` with their own lstat metadata size.
- **Mount Boundary Control (`--one-file-system` / `-x`):** Option to restrict traversal to the same filesystem device (`st_dev`) as the root path, preventing accidental traversal into network mounts, Time Machine volumes, or `/Volumes/*`.

### 3.2 Error Handling & Partial Results
- **Non-Fatal Traversal Errors:** Permission denied (`PermissionDenied`), missing files (`NotFound`), or invalid UTF-8 paths do not abort the scan.
- **Structured Error Ledger:** Inaccessible paths are recorded with their path, error category (`PermissionDenied`, `NotFound`, `IoError`, `CycleDetected`), and OS error message.
- **Ancestor Error Counts:** Each directory summary includes `error_count` (direct + recursive unreadable entries) so users immediately see when a directory's reported size is a lower bound due to permission restrictions.

### 3.3 Reporting & Output Modes
- **Human-Readable Terminal Summary (Default):**
  - Root summary banner: Total allocated size, total apparent size, file count, directory count, symlink count, elapsed scan duration, and error count.
  - **Top Directories Table:** Largest subdirectories ranked by size (configurable depth `--max-depth` and limit `--top N`, default `N = 20`), with percentage of root total and visual bar indicator.
  - **Top Files Table:** Largest individual files across the entire scanned subtree (`--top N`, default `N = 20`).
  - **Error Summary Footer:** Concise count of inaccessible paths (with `--show-errors` flag to list individual failed paths).
- **Human-Readable Size Formatting:** Binary IEC (`KiB`, `MiB`, `GiB`, `TiB`) or SI decimal (`KB`, `MB`, `GB`, `TB` via `--si`) with consistent precision (`12.4 GiB`, `840 MiB`, `0 B`).
- **Machine-Readable JSON Output (`--json`):**
  - Deterministic, versioned JSON schema (`schema_version: 1`) containing scan metadata, root summary, aggregated directory tree (bounded by `--max-depth`), top files list, and structured error records.
  - Strictly separates `stdout` (valid JSON payload only) from `stderr` (progress indicator and diagnostic logs).

### 3.4 Progress Reporting & Cancellation
- **Live Progress Feedback:** When `stderr` is an interactive TTY and `--quiet` is not passed, display periodic scan progress (files scanned, directories scanned, current bytes accumulated, elapsed time) throttled to at most 10–20 updates per second.
- **Graceful Cancellation (`SIGINT` / Ctrl-C):**
  - Cooperative cancellation token (`CancelToken` backed by `AtomicBool`) checked during traversal.
  - Pressing Ctrl-C stops traversal within `< 100ms` and exits cleanly with exit code `130` (or outputs partial results if `--partial-on-cancel` is requested).

---

## 4. Non-Goals for MVP

To prevent scope creep and keep agent tasks tightly bounded, the following are **explicitly excluded from MVP (`M0`–`M2`)**:
- **No Destructive Operations:** No file deletion, moving to Trash, truncation, compression, or chmod/chown operations. DiskGoblin MVP is strictly read-only.
- **No Privilege Escalation:** DiskGoblin will not invoke `sudo` or request macOS Full Disk Access via native entitlements in the CLI MVP; it operates strictly under the invoking user's permissions.
- **No Interactive Terminal UI (TUI):** The CLI MVP produces clean batch/streaming terminal output and JSON. Interactive drill-down navigation belongs to the `M3` Tauri Desktop app (or a post-MVP TUI).
- **No Content Hashing or Duplicate Detection:** Calculating SHA-256/BLAKE3 hashes of files to find duplicate content is deferred to post-MVP.
- **No Persistent Daemon or Database:** Scans run on-demand in memory. Snapshot persistence and diffing are post-MVP capabilities.

---

## 5. User Workflows

### Workflow 1: Quick Interactive Directory Triage (CLI)
```bash
$ diskgoblin ~/Projects
```
1. User runs `diskgoblin` on `~/Projects`.
2. `stderr` shows a transient progress line: `Scanning... 48,210 files | 3,120 dirs | 18.4 GiB allocated (0.4s)`.
3. Upon completion, `stdout` displays:
   - Total size (`18.4 GiB allocated / 19.1 GiB apparent`), item counts, and duration (`0.42s`).
   - Top 20 largest directories (highlighting heavy `target/` and `node_modules/` directories with `%` bars).
   - Top 20 largest individual files (highlighting `.rlib`, `.pack`, or `.dmg` files).
   - A notice if any subdirectories could not be read due to permissions.

### Workflow 2: Root / Home Directory Investigation with Permission Errors & Mount Isolation
```bash
$ diskgoblin ~ --one-file-system --max-depth 2 --top 15
```
1. User scans their macOS home directory while staying on the main APFS volume (`--one-file-system`).
2. macOS TCC blocks access to certain `~/Library/Application Support/...` or `~/.Trash` folders without Full Disk Access.
3. DiskGoblin records `PermissionDenied` errors without crashing, marks affected parent directories with a warning indicator (`[!] 3 unreadable paths`), and completes the scan of all accessible paths.

### Workflow 3: Scripted / CI Automation via JSON
```bash
$ diskgoblin ./build-artifacts --json --top 50 > scan-report.json
```
1. Progress output is automatically suppressed on `stderr` (or kept off `stdout`).
2. `scan-report.json` receives a deterministic JSON document suitable for `jq`, CI artifact size monitoring, or feeding into the `M3` evaluation harness.

### Workflow 4: Visual Exploration in Desktop App (`M3` — Desktop MVP)
1. User launches **DiskGoblin Desktop** (Tauri 2 app), selects a folder via native file picker (or types a path), and clicks **Scan**.
2. The Rust backend runs `diskgoblin-core` on a background worker thread, streaming throttled `ScanProgress` events over Tauri IPC.
3. The UI displays a live progress counter and a **Cancel** button.
4. When complete, the UI renders an interactive **Treemap** (and synchronized Top Directories / Top Files tables + Error drawer). Clicking a treemap node zooms into that subdirectory; breadcrumbs navigate back up.

---

## 6. CLI Interface Specification (`M1`)

```text
Usage: diskgoblin [OPTIONS] [PATH]

Arguments:
  [PATH]  Root directory or file to scan [default: .]

Options:
      --apparent-size        Use apparent (logical) byte size instead of physical disk allocation
  -x, --one-file-system      Do not cross filesystem mount boundaries (same device ID as root)
  -d, --max-depth <DEPTH>    Maximum directory depth to include in directory reports [default: 3]
  -n, --top <N>              Number of largest directories and files to display [default: 20]
      --min-size <BYTES>     Omit entries smaller than <BYTES> from top lists [default: 0]
      --si                   Use decimal SI units (KB, MB, GB) instead of binary IEC (KiB, MiB, GiB)
      --json                 Output results as machine-readable JSON to stdout
      --show-errors          List individual inaccessible paths and error reasons in text output
  -q, --quiet                Suppress non-error progress output on stderr
  -j, --threads <N>          Worker thread count (0 = automatic, 1 = sequential baseline) [default: 1 in M1, auto in M2]
  -h, --help                 Print help
  -V, --version              Print version
```

### Exit Codes
- `0`: Scan completed successfully (including scans with non-fatal child permission errors, provided the root path itself was readable).
- `1`: Fatal runtime error (e.g., root path does not exist or root path itself is unreadable).
- `2`: Invalid CLI arguments (handled by `clap`).
- `130`: Scan cancelled by user (`SIGINT`).
