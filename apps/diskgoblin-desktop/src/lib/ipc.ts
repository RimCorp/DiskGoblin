import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  DirNode,
  DirSummary,
  FileEntry,
  ScanErrorRecord,
  ScanOptionsDto,
  ScanProgress,
  ScanReport,
  SizeMetric,
} from "../types/scan";

/** Progress event emitted by the Rust `start_scan` command (`DG-014`). */
export const PROGRESS_EVENT = "scan:progress";

/** Schema version mirrored from `diskgoblin-core::REPORT_SCHEMA_VERSION`. */
const REPORT_SCHEMA_VERSION = 1;

/** Synthetic scan root used when no path is supplied outside a Tauri runtime. */
export const MOCK_SCAN_ROOT = "/mock/home";

/** Must match the Rust backend's cancellation error string. */
export const CANCEL_MESSAGE = "scan cancelled by user";

const KIB = 1024;
const MIB = 1024 * KIB;
const GIB = 1024 * MIB;

/** Fraction of the synthetic scan completed by each mock progress snapshot. */
const MOCK_PROGRESS_FRACTIONS = [0.25, 0.6, 1] as const;

/**
 * True inside a live Tauri webview. Vitest (node) and the plain browser preview
 * both return false; browser sessions can use the localhost HTTP bridge.
 */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export interface UiBootstrapConfig {
  root_path: string;
  size_metric: SizeMetric;
  one_file_system?: boolean;
  max_depth?: number | null;
  top_n?: number;
  min_size_bytes?: number;
  threads?: number;
  auto_scan?: boolean;
}

let httpBridgeEnabled = false;
export function setHttpBridgeEnabled(enabled: boolean): void {
  httpBridgeEnabled = enabled;
}
export function isHttpBridgeEnabled(): boolean {
  return httpBridgeEnabled;
}
export async function fetchUiBootstrapConfig(fetchImpl: typeof fetch = globalThis.fetch): Promise<UiBootstrapConfig | null> {
  if (isTauriRuntime() || (typeof window === "undefined" && !isHttpBridgeEnabled())) return null;
  try {
    const response = await fetchImpl("/api/config");
    if (!response.ok) return null;
    const config = await response.json() as UiBootstrapConfig;
    setHttpBridgeEnabled(true);
    return config;
  } catch {
    return null;
  }
}

/** Drops trailing separators (except for a bare `/`) so paths join cleanly. */
function normalizeRoot(rootPath: string): string {
  const trimmed = rootPath.trim();
  if (trimmed.length === 0) {
    return MOCK_SCAN_ROOT;
  }
  if (trimmed.length === 1) {
    return trimmed;
  }
  return trimmed.replace(/[\\/]+$/, "");
}

function joinPath(base: string, child: string): string {
  if (base.endsWith("/") || base.endsWith("\\")) {
    return `${base}${child}`;
  }
  return `${base}/${child}`;
}

function metricOf(apparent: number, allocated: number, metric: SizeMetric): number {
  return metric === "apparent" ? apparent : allocated;
}

interface MockDirSpec {
  /** Path segments below the scan root. */
  segments: string[];
  apparent: number;
  allocated: number;
  file_count: number;
  symlink_count?: number;
  error_count?: number;
  children?: MockDirSpec[];
}

function dirSpec(segments: string[], apparent: number, allocated: number, fileCount: number): MockDirSpec {
  return { segments, apparent, allocated, file_count: fileCount };
}

/**
 * Synthetic directory tree. Children always sum to their parent for both
 * metrics so the treemap, breadcrumb and table numbers stay consistent.
 */
function mockTreeSpec(): MockDirSpec {
  return {
    segments: [],
    apparent: 6976 * MIB,
    allocated: 7200 * MIB,
    file_count: 4,
    children: [
      dirSpec(["Videos"], 4096 * MIB, 4224 * MIB, 2),
      dirSpec(["Projects"], 2048 * MIB, 2120 * MIB, 2),
      {
        ...dirSpec(["Photos"], 768 * MIB, 792 * MIB, 0),
        children: [
          dirSpec(["Photos", "2024"], 512 * MIB, 528 * MIB, 120),
          dirSpec(["Photos", "2023"], 256 * MIB, 264 * MIB, 64),
        ],
      },
      {
        ...dirSpec(["Archive"], 64 * MIB, 64 * MIB, 1),
        error_count: 1,
      },
    ],
  };
}

function buildDirNode(spec: MockDirSpec, base: string, depth: number): DirNode {
  const path = spec.segments.reduce((current, segment) => joinPath(current, segment), base);
  const children = (spec.children ?? []).map((child) => buildDirNode(child, base, depth + 1));
  return {
    summary: {
      path,
      depth,
      apparent_size: spec.apparent,
      allocated_size: spec.allocated,
      file_count: spec.file_count,
      dir_count: children.length,
      symlink_count: spec.symlink_count ?? 0,
      error_count: spec.error_count ?? 0,
    },
    children,
  };
}

/** Every node below (and including) `root`, root first, depth-first order. */
export function flattenDirNodes(root: DirNode): DirNode[] {
  const nodes: DirNode[] = [root];
  for (const child of root.children) {
    nodes.push(...flattenDirNodes(child));
  }
  return nodes;
}

function mockTopDirectories(root: DirNode, metric: SizeMetric): DirSummary[] {
  return flattenDirNodes(root)
    .slice(1)
    .sort(
      (a, b) =>
        metricOf(b.summary.apparent_size, b.summary.allocated_size, metric) -
          metricOf(a.summary.apparent_size, a.summary.allocated_size, metric) ||
        a.summary.path.localeCompare(b.summary.path),
    )
    .map((node) => ({ ...node.summary }));
}

function mockTopFiles(base: string, metric: SizeMetric): FileEntry[] {
  const files: FileEntry[] = [
    {
      path: joinPath(base, "Videos/trip.mov"),
      apparent_size: 3 * GIB,
      allocated_size: 3 * GIB + 64 * MIB,
      is_deduplicated_hardlink: false,
    },
    {
      path: joinPath(base, "Videos/talk.mp4"),
      apparent_size: GIB,
      allocated_size: GIB + 32 * MIB,
      is_deduplicated_hardlink: false,
    },
    {
      path: joinPath(base, "Projects/bundle.zip"),
      apparent_size: 1536 * MIB,
      allocated_size: 1552 * MIB,
      is_deduplicated_hardlink: false,
    },
    {
      path: joinPath(base, "Projects/database.dump"),
      apparent_size: 512 * MIB,
      allocated_size: 512 * MIB,
      is_deduplicated_hardlink: true,
    },
    {
      path: joinPath(base, "Archive/old.tar.gz"),
      apparent_size: 63 * MIB,
      allocated_size: 64 * MIB,
      is_deduplicated_hardlink: false,
    },
  ];
  return files.sort(
    (a, b) =>
      metricOf(b.apparent_size, b.allocated_size, metric) -
        metricOf(a.apparent_size, a.allocated_size, metric) || a.path.localeCompare(b.path),
  );
}

function mockErrors(base: string): ScanErrorRecord[] {
  return [
    {
      path: joinPath(base, "Archive/private"),
      kind: "permission_denied",
      message: "permission denied (os error 13)",
    },
    {
      path: joinPath(base, "Videos/broken-link"),
      kind: "io_error",
      message: "input/output error (os error 5)",
    },
  ];
}

/**
 * Deterministic `ScanReport` fixture used outside a Tauri runtime (Vitest and
 * `vite preview`). Every call returns a fresh deep copy, so callers can mutate
 * the result freely without corrupting later scans.
 */
export function createMockScanReport(options: Partial<ScanOptionsDto> = {}): ScanReport {
  const base = normalizeRoot(options.root_path ?? MOCK_SCAN_ROOT);
  const metric: SizeMetric = options.size_metric === "apparent" ? "apparent" : "allocated";
  const topN = typeof options.top_n === "number" && options.top_n > 0 ? options.top_n : 20;

  const root = buildDirNode(mockTreeSpec(), base, 0);

  return {
    schema_version: REPORT_SCHEMA_VERSION,
    root_path: base,
    size_metric: metric,
    duration_ms: 480,
    root,
    top_directories: mockTopDirectories(root, metric).slice(0, topN),
    top_files: mockTopFiles(base, metric).slice(0, topN),
    errors: mockErrors(base),
  };
}

/** Mock progress snapshots (25 %, 60 %, 100 %) for a synthetic report. */
export function createMockProgressSequence(report: ScanReport): ScanProgress[] {
  const nodes = flattenDirNodes(report.root);
  const totalFiles = nodes.reduce((total, node) => total + node.summary.file_count, 0);
  const totalDirs = nodes.length - 1;

  return MOCK_PROGRESS_FRACTIONS.map((fraction) => ({
    files_seen: Math.round(totalFiles * fraction),
    dirs_seen: Math.round(totalDirs * fraction),
    symlinks_seen: 0,
    errors_seen: Math.floor(report.errors.length * fraction),
    bytes_accumulated: Math.round(
      metricOf(
        report.root.summary.apparent_size,
        report.root.summary.allocated_size,
        report.size_metric,
      ) * fraction,
    ),
    elapsed_ms: Math.round(report.duration_ms * fraction),
  }));
}

export type ScanProgressListener = (progress: ScanProgress) => void;

/** Active mock listeners; unused inside a real Tauri runtime. */
const mockListeners = new Set<ScanProgressListener>();

/** Generation of the current mock scan, so stale cancels cannot leak forward. */
let mockGeneration = 0;
let cancelledGeneration = -1;

function emitMockProgress(progress: ScanProgress): void {
  for (const listener of [...mockListeners]) {
    listener(progress);
  }
}

/**
 * Starts a scan through Tauri, the localhost HTTP bridge, or the preview fixture.
 */
export async function startScan(options: ScanOptionsDto, fetchImpl: typeof fetch = globalThis.fetch): Promise<ScanReport> {
  if (isTauriRuntime()) {
    return invoke<ScanReport>("start_scan", { options });
  }

  if (isHttpBridgeEnabled() || (typeof window !== "undefined" && options.root_path !== MOCK_SCAN_ROOT)) {
    let polling: Promise<void> | null = null;
    let active = true;
    const poll = async () => {
      try {
        const response = await fetchImpl("/api/progress");
        if (response.ok) {
          const progress = await response.json() as ScanProgress | null;
          if (progress && active) emitMockProgress(progress);
        }
      } catch {
        // Transient progress failures must not discard the scan result.
      }
    };
    const timer = setInterval(() => {
      if (!polling) polling = poll().finally(() => { polling = null; });
    }, 100);
    try {
      const response = await fetchImpl("/api/scan", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(options),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "scan failed");
      clearInterval(timer);
      await polling;
      await poll();
      return body as ScanReport;
    } catch (error) {
      if (!(error instanceof TypeError && !isHttpBridgeEnabled() && options.root_path === MOCK_SCAN_ROOT)) throw error;
    } finally {
      active = false;
      clearInterval(timer);
    }
  }

  const generation = ++mockGeneration;
  const report = createMockScanReport(options);
  // Yield between snapshots so a cancel issued right after `startScan` wins,
  // without depending on timers (keeps Vitest fast and deterministic).
  for (const progress of createMockProgressSequence(report)) {
    await Promise.resolve();
    if (cancelledGeneration === generation) {
      throw new Error(CANCEL_MESSAGE);
    }
    emitMockProgress(progress);
  }
  return report;
}

/** Cancels the in-flight scan, if any. */
export async function cancelScan(fetchImpl: typeof fetch = globalThis.fetch): Promise<void> {
  if (isTauriRuntime()) {
    await invoke<void>("cancel_scan");
    return;
  }
  if (isHttpBridgeEnabled()) {
    await fetchImpl("/api/cancel", { method: "POST" });
  }
  cancelledGeneration = mockGeneration;
}

/**
 * Opens the native folder picker. Outside Tauri the mock returns the synthetic
 * scan root so the browser preview stays usable.
 */
export async function pickDirectory(fetchImpl: typeof fetch = globalThis.fetch): Promise<string | null> {
  if (isTauriRuntime()) {
    return invoke<string | null>("pick_directory");
  }
  if (isHttpBridgeEnabled()) {
    const response = await fetchImpl("/api/pick-directory", { method: "POST" });
    if (!response.ok) throw new Error("directory picker failed");
    const body = await response.json() as { path?: string | null };
    return body.path ?? null;
  }
  return MOCK_SCAN_ROOT;
}

/**
 * Subscribes to backend scan progress. Resolves to an unlisten callback that is
 * safe to call more than once.
 */
export async function subscribeScanProgress(
  listener: ScanProgressListener,
): Promise<() => void> {
  if (isTauriRuntime()) {
    const unlisten = await listen<ScanProgress>(PROGRESS_EVENT, (event) => {
      listener(event.payload);
    });
    return () => {
      unlisten();
    };
  }

  mockListeners.add(listener);
  return () => {
    mockListeners.delete(listener);
  };
}

/** Clears mock scan state between tests. Never touches a live Tauri backend. */
export function resetMockScanState(): void {
  setHttpBridgeEnabled(false);
  mockListeners.clear();
  mockGeneration = 0;
  cancelledGeneration = -1;
}
