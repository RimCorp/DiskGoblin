# ADR-002: Disk Size Metrics and Hard-Link Deduplication

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Principal Architect, Product Owner

## Context

Disk space analyzers must answer two distinct questions:
1. *"How much physical space on my drive will I reclaim if I remove this directory?"* (Allocated / physical size)
2. *"How many bytes of logical data are inside these files?"* (Apparent / logical size)

On macOS (APFS) and Linux (ext4/XFS/Btrfs):
- **Sparse files** (such as Docker VM images, database files, or sparse build caches) may have an apparent size of `64 GiB` (`st_size`) while only occupying `2 GiB` on disk (`st_blocks * 512`).
- **Small files** occupy at least one filesystem allocation block (typically `4 KiB` on APFS/ext4), so 100,000 tiny 50-byte files consume ~`400 MiB` on disk despite having an apparent size of ~`5 MiB`.
- **Hard links** (`st_nlink > 1`, common in `pnpm`/`uv` caches, Git object stores, and backup trees) share the exact same underlying inode (`st_dev`, `st_ino`). Counting every directory entry's size naively causes directory totals to exceed physical disk capacity.

## Decision

1. **Dual Size Tracking:**
   Every `FileEntry` and `DirSummary` in `diskgoblin-core` records **both** `allocated_size: u64` and `apparent_size: u64`.
   - On Unix (`#[cfg(unix)]`), `allocated_size` is computed as `metadata.blocks().saturating_mul(512)` and `apparent_size` is `metadata.len()`.
   - On non-Unix platforms (until platform-specific APIs like `GetCompressedFileSizeW` are added), `allocated_size` falls back to `apparent_size`.

2. **Default Sorting & Display Metric:**
   `SizeMetric::Allocated` is the default metric for ranking `top_directories` and `top_files` and rendering primary sizes in the CLI. Users can switch to logical byte length via `--apparent-size` (`SizeMetric::Apparent`). Both metrics are always present in `--json` output and in the root summary banner.

3. **Hard-Link Deduplication:**
   During a scan, the scanner maintains a set of seen hard-linked file identifiers (`HashSet<FileId>` where `FileId = (st_dev, st_ino)`).
   - To minimize memory overhead, **only entries with `hard_link_count > 1` are inserted into the `HashSet`**. Regular single-link files (`nlink == 1`, which represent >99% of typical files) never touch the hash set.
   - When a file with `nlink > 1` is encountered for the **first time**, its `allocated_size` and `apparent_size` are added to its ancestor directories.
   - On **subsequent encounters** within the same scan, the file's `is_deduplicated_hardlink` flag is set to `true`, and it contributes `0` bytes to directory size rollups (while still incrementing `file_count`).
   - For determinism across runs, the sequential scanner visits directory entries sorted lexicographically by file name, ensuring the same path always wins first-encounter attribution.

## Consequences

### Positive
- Matches user expectations from tools like `du`, `ncdu`, and `dust` on macOS/Linux.
- Prevents sparse files and hard-linked package caches (`uv`, `pnpm`, Cargo hardlinks) from wildly inflating reported disk usage.
- Zero hash-table memory overhead for ordinary files (`nlink == 1`).

### Negative
- In parallel traversal (`M2`), first-encounter attribution among hard links in different subdirectories could vary unless deduplication occurs during deterministic post-processing or uses a deterministic path-ordering rule. We address this in `ADR-005` by attributing hard-linked inodes deterministically during aggregation or recording a canonical smallest path.
