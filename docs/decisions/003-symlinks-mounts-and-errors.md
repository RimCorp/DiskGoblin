# ADR-003: Symlink Safety, Filesystem Mount Boundaries, and Partial Failure Model

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Principal Architect, Product Owner

## Context

Real-world filesystems contain hazards that break naive recursive scanners:
1. **Symbolic link cycles and escapes:** Following symlinks (`std::fs::metadata`) can cause infinite recursion (`a/loop -> ..`) or double-count external directories (`/Users/me/project/link -> /Applications`).
2. **Mount boundaries:** On macOS, `/System/Volumes/Data` is firmlinked/mounted alongside `/Volumes/ExternalDrive` or network shares (`SMB`/`NFS`). Traversing into external mounts unexpectedly stalls scans or inflates local disk totals.
3. **Permission & transient errors:** Scanning `~` or `/` on macOS inevitably hits TCC-protected folders (`~/Library/Mail`, `~/.Trash`) returning `EACCES`/`EPERM`, or transient files that disappear between `read_dir` and `lstat` (`ENOENT`).

## Decision

1. **Symlinks Are Never Followed:**
   - All metadata queries use `std::fs::symlink_metadata` (`lstat`).
   - Symbolic links are classified as `EntryKind::Symlink`. Their own symlink inode size is counted, `symlink_count` is incremented, and traversal **never recurses** into symlink targets (even if the root path itself contains symlinks; if the user passes a symlink as the root argument on the CLI, only the root argument may be resolved if explicitly desired, or inspected as-is—we resolve root path only if it is a directory symlink, never descendant symlinks).

2. **Mount Boundary Enforcement (`--one-file-system` / `-x`):**
   - At scan initialization, `diskgoblin-core` records the root directory's `device_id` (`st_dev`).
   - When `ScanOptions::one_file_system` is `true`, any child directory whose `device_id != root_device_id` is skipped from recursion.

3. **Two-Tier Error Model (Fatal vs. Non-Fatal Partial Results):**
   - **Fatal Errors (`FatalScanError`):** Returned as `Err(FatalScanError)` from `scan_directory` only when the scan root itself does not exist (`RootNotFound`), cannot be read (`RootUnreadable`), or the scan is aborted via `CancelToken` (`Cancelled`).
   - **Non-Fatal Entry Errors (`ScanErrorRecord`):** When reading a descendant directory or entry fails during traversal:
     - The error is classified into `ScanErrorKind::{PermissionDenied, NotFound, IoError}`.
     - A `ScanErrorRecord { path, kind, message }` is appended to `ScanReport::errors`.
     - The enclosing directory's `error_count` is incremented and rolled up to all ancestor `DirSummary::error_count` fields.
     - Traversal continues normally for all sibling directories.

## Consequences

### Positive
- Eliminates infinite symlink loops without needing expensive cycle-detection graphs.
- Gives users complete confidence when scanning `/Users/<username>` on macOS: TCC errors are cleanly isolated, counted per directory, and surfaced without crashing.
- JSON consumers can programmatically inspect `report.errors` and `dir.summary.error_count` to know whether a subtree's size is exact or a lower bound.

### Negative
- Users who intentionally store data behind symlinks won't see target sizes included (standard behavior for `du -P` / `ncdu` / `dust`). A `--follow-symlinks` option with cycle tracking can be evaluated post-MVP if requested.
