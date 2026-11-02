// Mirrors diskgoblin-core's serde JSON. Rust Option values serialize as null.
// u64/usize values are JSON numbers; values above 2^53 - 1 lose JS precision.
export type SizeMetric = "allocated" | "apparent";

export interface ScanOptionsDto {
  root_path: string;
  size_metric: SizeMetric;
  one_file_system: boolean;
  max_depth: number | null;
  top_n: number;
  min_size_bytes: number;
  threads: number;
}

export interface DirSummary {
  path: string;
  depth: number;
  apparent_size: number;
  allocated_size: number;
  file_count: number;
  dir_count: number;
  symlink_count: number;
  error_count: number;
}

export interface DirNode {
  summary: DirSummary;
  children: DirNode[];
}

export interface FileEntry {
  path: string;
  apparent_size: number;
  allocated_size: number;
  is_deduplicated_hardlink: boolean;
}

export type ScanErrorKind = "permission_denied" | "not_found" | "io_error";

export interface ScanErrorRecord {
  path: string;
  kind: ScanErrorKind;
  message: string;
}

export interface ScanReport {
  schema_version: number;
  root_path: string;
  size_metric: SizeMetric;
  duration_ms: number;
  root: DirNode;
  top_directories: DirSummary[];
  top_files: FileEntry[];
  errors: ScanErrorRecord[];
}

export interface ScanProgress {
  files_seen: number;
  dirs_seen: number;
  symlinks_seen: number;
  errors_seen: number;
  bytes_accumulated: number;
  elapsed_ms: number;
}
