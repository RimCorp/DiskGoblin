# DG-012: SSD parallel traversal study

## Decision

Keep the CLI default at `--threads 1`. No tested parallel worker count achieves
at least 1.5× speedup across all three fixtures. Explicit parallel selection and
`--threads 0` (automatic, capped at eight workers) are available.

## Environment and method

- Date: 2026-11-01.
- Host: Mac mini (Mac14,12), Apple M2 Pro, 10 CPU cores
  (6 performance / 4 efficiency), 16 GB RAM.
- OS: macOS 27.0.1, build 26A434; aarch64, Rust 1.85.0.
- Storage: internal APPLE SSD AP0512Z NVMe, APFS data volume
  (`/dev/disk3s5`, `/System/Volumes/Data`). Synthetic temporary fixtures are
  created under `/private/tmp` on that volume. Hardware was identified with
  `system_profiler SPHardwareDataType SPNVMeDataType`; volume with `df` / `mount`.
- Criterion 0.5.1, release profile, default ScanOptions except root and threads;
  Rayon requirement `1.10` resolves to 1.12.0 in the existing lockfile.
- Command: `CARGO_HOME=/private/tmp/dg009-cargo-home cargo bench -p diskgoblin-core -- --quick`.
  The CARGO_HOME override uses the pre-provisioned dependency cache.
- Each fixture is created once outside the measured closure. Each iteration
  measures the complete `scan_directory` call, including pool construction,
  filesystem traversal, deterministic replay, hard-link deduplication, directory
  rollups, ranking, and report destruction. Threads=1 uses the sequential oracle.
- Fixtures retain DG-011 topology and 256-byte files: wide_tree has 50 sibling
  directories × 200 files; deep_tree has 20 nested levels × 125 files plus one
  ancestor symlink per level; hardlink_cache_tree has 2,000 original files and
  3,000 additional hard links across two sibling directories.
- Repeated iterations operate on recently created, warm-cache filesystem metadata.
  Caches were not flushed. No unrelated test or build ran during measured sampling.
  Gnuplot was unavailable; Criterion used its Plotters backend successfully.

## Criterion results

The following are the actual printed millisecond estimates from the final complete
12-case quick run. Lower/upper values are Criterion's reported interval bounds;
these quick-mode estimates are exploratory rather than a precision contention
study. Speedup is the same-run sequential point estimate divided by the worker
count's point estimate. A first complete run showed the same default-gate outcome.

| Fixture | Threads | Lower (ms) | Estimate (ms) | Upper (ms) | Speedup vs 1 |
|---|---:|---:|---:|---:|---:|
| `wide_tree` | 1 | 38.943 | 39.037 | 39.414 | 1.000× |
| `wide_tree` | 2 | 22.953 | 23.526 | 23.669 | 1.659× |
| `wide_tree` | 4 | 20.739 | 20.939 | 21.737 | 1.864× |
| `wide_tree` | 8 | 17.088 | 17.145 | 17.373 | 2.277× |
| `deep_tree` | 1 | 11.259 | 11.356 | 11.381 | 1.000× |
| `deep_tree` | 2 | 8.2203 | 8.2596 | 8.2695 | 1.375× |
| `deep_tree` | 4 | 8.2328 | 8.2661 | 8.3992 | 1.374× |
| `deep_tree` | 8 | 8.1435 | 8.1555 | 8.2035 | 1.392× |
| `hardlink_cache_tree` | 1 | 35.135 | 35.263 | 35.774 | 1.000× |
| `hardlink_cache_tree` | 2 | 22.451 | 23.016 | 23.158 | 1.532× |
| `hardlink_cache_tree` | 4 | 26.709 | 26.847 | 27.399 | 1.313× |
| `hardlink_cache_tree` | 8 | 23.584 | 24.359 | 24.553 | 1.448× |

## Scaling analysis

The wide tree improves from 39.037 ms to 17.145 ms at eight workers (2.277×).
Sibling directories expose independent read_dir calls, and metadata operations
within directories can also run concurrently. Work stealing overlaps syscall
latency. Speedup remains sublinear: sorting, building and consuming the ordered
result tree, aggregation, deduplication and rankings retain serial work, while
pool startup and task scheduling add overhead to every measured scan.

The deep tree improves from 11.356 ms to 8.1555 ms (1.392×), but barely scales
beyond two workers. Directory discovery still follows a dependent chain: the
next level cannot be enumerated until its parent is discovered. Concurrent lstat
operations on sibling files help, but extra workers do not remove that dependency
or the serial replay costs. This fixture alone fails the 1.5× default gate for
all tested parallel worker counts.

The hard-link tree is fastest with two workers (23.016 ms, 1.532×). Four workers
regress relative to two (26.847 ms, 1.314× versus sequential); eight recover to
24.359 ms (1.448×), still slower than two. Thousands of lstat calls share only
two directories and many inode identities. APFS/VFS directory or inode lock
contention, cache-coherence traffic and scheduling overhead are plausible causes
of this non-monotonic scaling. These are inferences from timings: the experiment
contains no kernel lock tracing or hardware I/O queue measurements, so it does
not isolate those causes. Hard-link HashSet attribution itself runs serially in
canonical replay and has no shared worker lock.

All tested worker counts improve point estimates versus sequential on this host,
but no single count meets 1.5× across the suite. Eight workers, which auto mode
selects on this 10-core host, miss the threshold on both deep and hard-link trees.
The ADR-005 empirical default gate is therefore not satisfied.

## Correctness, verification, and limitations

Workers use read_dir and symlink_metadata only, sort sibling paths, and collect
results in sorted order. Replay assigns scan-local IDs on the calling thread in
the sequential scanner's depth-first order. This matters for names such as `a`
and `a-`: globally sorting full file paths would attribute a shared inode to a
different directory than sorting sibling names before descending into `a`.
The existing aggregator therefore receives identical events and makes identical
hard-link, rollup and ranking decisions.

Tests compare complete reports (normalizing duration_ms only) and raw events for
threads 0, 2, 4 and 8 against sequential traversal, using nested directories,
empty directories, cross-directory hard links, ancestor/dangling symlinks and
permission errors. Both size metrics, depth pruning and one-file-system modes
are covered. Injected device IDs test mount pruning without requiring privileged
mounts; injected metadata failures test recoverable error placement. A barrier
proves all four workers are active before cancellation, checks all return, and
asserts cancellation-to-return is under 100 ms. Public API progress cancellation,
final progress totals, root validation and pre-cancellation are also tested.

Verification on the final implementation:

- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --workspace --all-targets`: BLOCKED by the existing
  `platform::tests::unix_socket_is_other` test. The sandbox denies
  `UnixListener::bind` with OS error 1, Operation not permitted. All five new
  parallel scanner tests pass. No production/test change was made to bypass
  the socket failure.
- Supplemental `cargo test --workspace --lib --bins --tests -- --skip platform::tests::unix_socket_is_other`:
  PASS, 37 unit and 37 integration tests; one existing unit test filtered out.
  Explicit target selection is necessary because Criterion does not accept
  libtest's `--skip` flag under `--all-targets`.
- `cargo bench -p diskgoblin-core -- --quick`: PASS, all 12 cases.

The full required test gate still needs a run where Unix socket binding is
permitted. Rust 1.99 CI was not available locally; periodic unsigned checks use
bitwise masks to avoid manual_is_multiple_of warnings.

The explicitly requested discovery-tree approach temporarily retains metadata
and paths for every entry before replay, so its discovery memory is O(entries),
whereas aggregation still retains only directory summaries and top-N files.
The architecture's million-file / 150 MiB target has not been established by
these small fixtures. Future work should measure RSS and evaluate bounded
ordered buffering before making parallel mode the default. Recursive discovery
and replay also consume stack with directory depth; the sequential engine keeps
its iterative implementation.

Cancellation is cooperative between filesystem operations, entry enumeration,
parallel tasks and replay events. The tests establish the <100 ms behavior on
synthetic local fixtures; an OS syscall blocked on a stalled filesystem, a large
uninterruptible sort, or a blocking user ProgressSink cannot be forcibly
interrupted by CancelToken. During discovery atomic progress counters expose
counts; bytes remain zero until canonical attribution is complete, and the
aggregator flushes the final byte totals.

These warm-cache quick runs do not characterize cold-cache performance, other
SSD models, external/network filesystems, 16-worker scaling, peak memory or
sustained thermal behavior. Longer Criterion sampling and OS profiling are
appropriate before drawing broader conclusions about SSD saturation.
