# DiskGoblin

> **Delve into your disk usage and see what's gobblin' your space.**

**DiskGoblin** is a fast, deterministic, strictly **read-only** disk usage analyzer written in Rust. It traverses directory trees safely without following symbolic links, deduplicates hard-linked files by `(dev, ino)` on Unix, distinguishes physical disk allocation (`st_blocks * 512`) from logical apparent size (`st_size`), and emits both human-readable terminal tables and versioned JSON reports.

---

## Features (Milestone M1 — CLI MVP)

- **Strictly Read-Only & Symlink-Safe:** Uses `std::fs::symlink_metadata` (`lstat`) exclusively. Never modifies files or follows symlinks into recursive directory loops.
- **Physical vs. Apparent Size:** Reports actual allocated disk blocks (`512`-byte blocks on Unix) by default, or logical file lengths via `--apparent-size`.
- **Hard-Link Deduplication:** Tracks `(dev, ino)` pairs on Unix (`nlink > 1`) so hard-linked files are counted toward directory totals and top-file rankings only once.
- **Mount-Boundary Control:** Optionally stay on the root filesystem device (`-x` / `--one-file-system`).
- **Resilient Partial Scans:** Permission errors (`EACCES` / `EPERM`) on subdirectories are recorded and surfaced in the summary (`--show-errors` to list individual paths) without aborting the scan.
- **Machine-Readable JSON (`schema_version: 1`):** Clean `stdout`/`stderr` separation — `--json` emits a single parseable JSON document on `stdout` while suppressing progress on `stderr`.
- **Cooperative `SIGINT` Cancellation:** Pressing `Ctrl-C` cleanly aborts in-flight traversal and exits with status `130`.

---

## Quick Start

### Prerequisites

- **Rust toolchain** (`1.85+` recommended, `cargo` and `rustc`)

### Run Directly via Cargo

From the repository root, run `diskgoblin` against any directory (defaults to `.`):

```bash
# Scan the current directory (top 20 directories and files by allocated size)
cargo run -p diskgoblin-cli -- .

# Scan a specific path showing top 10 entries using apparent size
cargo run -p diskgoblin-cli -- ./docs --top 10 --apparent-size

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

Arguments:
  [PATH]  Root directory or file to scan [default: .]

Options:
      --apparent-size      Use apparent (logical) byte size instead of physical disk allocation
  -x, --one-file-system    Do not cross filesystem mount boundaries
  -d, --max-depth <DEPTH>  Maximum directory depth to include in directory reports [default: 3]
  -n, --top <N>            Number of largest directories and files to display [default: 20]
      --min-size <BYTES>   Omit entries smaller than BYTES from top lists [default: 0]
      --si                 Use decimal SI units (KB, MB, GB) instead of binary IEC units (KiB, MiB, GiB)
      --json               Output machine-readable JSON to stdout
      --show-errors        List individual inaccessible paths and error reasons in text output
  -q, --quiet              Suppress non-error progress output on stderr
  -j, --threads <N>        Worker thread count (0 = automatic, 1 = sequential; M1 scans sequentially) [default: 1]
  -h, --help               Print help
  -V, --version            Print version
```

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
```

---

## Sample Output

### Human-Readable Terminal Report

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

## Workspace Structure & Development

DiskGoblin is organized as a Cargo workspace enforcing strict library/CLI separation ([`ADR-001`](docs/decisions/ADR-001-rust-workspace-and-crate-boundaries.md)):

- [`crates/diskgoblin-core`](crates/diskgoblin-core): Pure library crate containing domain models, platform metadata extraction (`lstat`, `dev`/`ino`, `blocks`), sequential traversal, bottom-up tree aggregation, byte formatting, and versioned JSON serialization.
- [`crates/diskgoblin-cli`](crates/diskgoblin-cli): Thin binary wrapper (`diskgoblin`) providing `clap` CLI argument parsing, TTY progress reporting on `stderr`, `SIGINT` cancellation, and human table rendering.

### Quality Gates

Run the full verification suite locally:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

---

## Architecture & Documentation

- [Product Specification](docs/product-spec.md)
- [Architecture Specification](docs/architecture.md)
- [Implementation Plan & Backlog](docs/implementation-plan.md)
- [Testing Strategy](docs/testing-strategy.md)
- [Autonomous Software Factory Design](docs/factory-design.md)
- [Agent Operating Contract (`AGENTS.md`)](AGENTS.md)
