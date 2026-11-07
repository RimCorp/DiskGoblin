import { renderToStaticMarkup } from "react-dom/server";
import { isValidElement } from "react";
import type { ComponentProps, ReactNode, ReactElement } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "../../App";
import ErrorDrawer, { describeErrorKind } from "../ErrorDrawer";
import ScanToolbar, { describeProgress, formatCount } from "../ScanToolbar";
import TopItemsPanel, {
  formatPercent,
  relativePercent,
  resolveDirectoryNode,
} from "../TopItemsPanel";
import {
  revealPath, getLastRevealedPath,
  CANCEL_MESSAGE,
  MOCK_SCAN_ROOT,
  cancelScan,
  createMockProgressSequence,
  createMockScanReport,
  flattenDirNodes,
  isTauriRuntime,
  fetchUiBootstrapConfig,
  setHttpBridgeEnabled,
  isHttpBridgeEnabled,
  pickDirectory,
  resetMockScanState,
  startScan,
  subscribeScanProgress,
} from "../../lib/ipc";
import { formatBytes } from "../../lib/treemapLayout";
import type {
  DirNode,
  ScanOptionsDto,
  ScanProgress,
  SizeMetric,
} from "../../types/scan";

const MIB = 1024 * 1024;
const GIB = 1024 * MIB;

function options(overrides: Partial<ScanOptionsDto> = {}): ScanOptionsDto {
  return {
    root_path: MOCK_SCAN_ROOT,
    size_metric: "allocated",
    one_file_system: false,
    max_depth: null,
    top_n: 20,
    min_size_bytes: 0,
    threads: 0,
    ...overrides,
  };
}

function render(element: Parameters<typeof renderToStaticMarkup>[0]): string {
  return renderToStaticMarkup(element);
}

/** The full opening `<button ...>` tag whose text content is exactly `label`. */
function buttonTag(html: string, label: string): string {
  const match = html.match(new RegExp(`<button[^>]*>${label}</button>`));
  expect(match, `no button labelled ${label}`).not.toBeNull();
  return match ? match[0] : "";
}

function countOf(html: string, testId: string): number {
  return (html.match(new RegExp(`data-testid="${testId}"`, "g")) ?? []).length;
}

function toolbarProps(overrides: Partial<ComponentProps<typeof ScanToolbar>> = {}) {
  return {
    path: MOCK_SCAN_ROOT,
    onPathChange: () => {},
    metric: "allocated" as SizeMetric,
    onMetricChange: () => {},
    scanning: false,
    progress: null,
    onScan: () => {},
    onCancel: () => {},
    onPickDirectory: () => {},
    ...overrides,
  };
}

function panelProps(overrides: Partial<ComponentProps<typeof TopItemsPanel>> = {}) {
  return {
    report: createMockScanReport(),
    metric: "allocated" as SizeMetric,
    tab: "directories" as const,
    onTabChange: () => {},
    onSelectItem: () => {},
    ...overrides,
  };
}

function nodeAt(root: DirNode, path: string): DirNode | undefined {
  return flattenDirNodes(root).find((node) => node.summary.path === path);
}

beforeEach(() => {
  resetMockScanState();
});

describe("formatCount", () => {
  it("groups thousands without depending on the host locale", () => {
    expect(formatCount(0)).toBe("0");
    expect(formatCount(999)).toBe("999");
    expect(formatCount(1000)).toBe("1,000");
    expect(formatCount(1234567)).toBe("1,234,567");
    expect(formatCount(3200.4)).toBe("3,200");
    expect(formatCount(-1234)).toBe("-1,234");
    expect(formatCount(Number.NaN)).toBe("0");
  });
});

describe("<ScanToolbar />", () => {
  it("renders the path input, folder picker, metric toggle, and idle button states", () => {
    const html = render(<ScanToolbar {...toolbarProps()} />);

    expect(html).toContain('aria-label="Scan root path"');
    expect(html).toContain(`value="${MOCK_SCAN_ROOT}"`);
    expect(html).toContain("Choose Folder");

    // The active metric is the only pressed toggle.
    expect(html).toMatch(/<button[^>]*aria-pressed="true"[^>]*>Allocated<\/button>/);
    expect(html).toMatch(/<button[^>]*aria-pressed="false"[^>]*>Apparent<\/button>/);

    // Idle: Scan is available, Cancel is not.
    expect(buttonTag(html, "Scan")).not.toContain("disabled");
    expect(buttonTag(html, "Cancel")).toContain("disabled");

    // No progress banner until the first snapshot arrives.
    expect(html).not.toContain('data-testid="scan-progress"');
  });

  it("toggles the active metric from the selected size metric prop", () => {
    const apparent = render(<ScanToolbar {...toolbarProps({ metric: "apparent" })} />);
    expect(apparent).toMatch(/<button[^>]*aria-pressed="false"[^>]*>Allocated<\/button>/);
    expect(apparent).toMatch(/<button[^>]*aria-pressed="true"[^>]*>Apparent<\/button>/);
  });

  it("disables Scan while scanning, enables Cancel, and blocks scans without a path", () => {
    const scanning = render(<ScanToolbar {...toolbarProps({ scanning: true })} />);
    expect(buttonTag(scanning, "Scan")).toContain("disabled");
    expect(buttonTag(scanning, "Cancel")).not.toContain("disabled");

    for (const path of ["", "   "]) {
      const empty = render(<ScanToolbar {...toolbarProps({ path })} />);
      expect(buttonTag(empty, "Scan")).toContain("disabled");
      expect(buttonTag(empty, "Cancel")).toContain("disabled");
    }
  });

  it("renders live ScanProgress counters for files, dirs, errors and bytes", () => {
    const progress: ScanProgress = {
      files_seen: 3200,
      dirs_seen: 7,
      symlinks_seen: 3,
      errors_seen: 2,
      bytes_accumulated: 4 * MIB,
      elapsed_ms: 480,
    };
    const html = render(<ScanToolbar {...toolbarProps({ scanning: true, progress })} />);

    expect(html).toContain('data-testid="scan-progress"');
    expect(html).toContain('role="status"');
    expect(html).toContain("3,200 files");
    expect(html).toContain("7 dirs");
    expect(html).toContain("2 errors");
    expect(html).toContain("4 MiB");
    expect(html).toContain(describeProgress(progress, true));
    expect(describeProgress(progress, true)).toContain("Scanning:");
    expect(describeProgress(progress, false)).toContain("Last scan:");
  });
});

describe("<TopItemsPanel />", () => {
  it("renders the top directories tab with formatted sizes and percentage bars", () => {
    const report = createMockScanReport({ size_metric: "allocated" });
    const html = render(<TopItemsPanel {...panelProps({ report })} />);

    expect(html).toMatch(/<button[^>]*aria-selected="true"[^>]*>Folders<\/button>/);
    expect(html).toMatch(/<button[^>]*aria-selected="false"[^>]*>Files<\/button>/);

    expect(countOf(html, "top-directory-row")).toBe(report.root.children.length);
    expect(countOf(html, "top-file-row")).toBe(0);
    expect(countOf(html, "size-bar")).toBe(report.root.children.length);

    const videos = report.top_directories[0];
    expect(videos.path).toBe(`${MOCK_SCAN_ROOT}/Videos`);
    expect(html).toContain(formatBytes(videos.allocated_size));
    expect(html).toContain("4.1 GiB");
    expect(html).toContain(videos.path);

    const videosPercentage = relativePercent(videos.allocated_size, report.root.summary.allocated_size);
    expect(relativePercent(videos.allocated_size, report.root.summary.allocated_size)).toBeCloseTo(58.81, 2);
    expect(html).toContain(formatPercent(videosPercentage));
    expect(html).toContain(`data-percent="${videosPercentage.toFixed(1)}"`);
  });

  it("renders the top files tab with formatted sizes and root-relative shares", () => {
    const report = createMockScanReport();
    const html = render(<TopItemsPanel {...panelProps({ report, tab: "files" })} />);

    expect(html).toMatch(/<button[^>]*aria-selected="true"[^>]*>Files<\/button>/);
    expect(countOf(html, "top-file-row")).toBe(1);
    expect(countOf(html, "top-directory-row")).toBe(0);

    const largest = report.top_files.find(file => file.path === `${MOCK_SCAN_ROOT}/notes.txt`)!;
    expect(largest.path).toBe(`${MOCK_SCAN_ROOT}/notes.txt`);
    expect(html).toContain(formatBytes(largest.allocated_size));
    expect(html).toContain("16 MiB");
    expect(html).toContain(largest.path);

    const share = relativePercent(largest.allocated_size, report.root.summary.allocated_size);
    expect(html).toContain(formatPercent(share));
  });

  it("switches sizes with the selected metric", () => {
    const apparent = render(
      <TopItemsPanel {...panelProps({ report: createMockScanReport(), metric: "apparent" })} />,
    );
    const allocated = render(
      <TopItemsPanel {...panelProps({ report: createMockScanReport(), metric: "allocated" })} />,
    );
    expect(apparent).not.toBe(allocated);
    expect(apparent).toContain("4 GiB");
    expect(allocated).toContain("4.1 GiB");
  });

  it("resolves directory rows against the scanned tree", () => {
    const report = createMockScanReport();
    const root = report.root;

    expect(resolveDirectoryNode(root, `${MOCK_SCAN_ROOT}/Videos`)?.summary.path).toBe(
      `${MOCK_SCAN_ROOT}/Videos`,
    );
    expect(resolveDirectoryNode(root, `${MOCK_SCAN_ROOT}/Photos/2024`)?.summary.apparent_size).toBe(
      512 * MIB,
    );
    expect(resolveDirectoryNode(root, MOCK_SCAN_ROOT)?.summary.path).toBe(MOCK_SCAN_ROOT);

    expect(resolveDirectoryNode(root, `${MOCK_SCAN_ROOT}/Missing`)).toBeNull();
    // A shared prefix is not the same directory.
    expect(resolveDirectoryNode(root, `${MOCK_SCAN_ROOT}/Video`)).toBeNull();
    expect(resolveDirectoryNode(root, "")).toBeNull();

    // Every listed top directory resolves to a clickable row.
    const html = render(<TopItemsPanel {...panelProps({ report })} />);
    expect(html).not.toContain('data-present="false"');
    for (const summary of report.top_directories) {
      expect(resolveDirectoryNode(root, summary.path)).not.toBeNull();
    }
  });

  it("scopes both tabs to the viewed folder and synchronizes selection", () => {
    const report = createMockScanReport();
    const viewedFolder = report.root.children[0];
    const html = render(<TopItemsPanel {...panelProps({ report, viewedFolder, tab: "files", selectedId: `${MOCK_SCAN_ROOT}/Videos/trip.mov` })} />);
    expect(countOf(html, "top-file-row")).toBe(2);
    expect(html).toContain('data-selected="true"');
    expect(html).not.toContain("notes.txt");
    expect(html).toContain('data-percent="68.7"');
  });

  it("renders an empty state before a report exists", () => {
    const html = render(<TopItemsPanel {...panelProps({ report: null })} />);
    expect(html).toContain("Run a scan to rank the largest directories and files.");
    expect(countOf(html, "top-directory-row")).toBe(0);
  });

  it("clamps relative percentages to a usable bar width", () => {
    expect(relativePercent(50, 100)).toBe(50);
    expect(relativePercent(0, 100)).toBe(0);
    expect(relativePercent(200, 100)).toBe(100);
    expect(relativePercent(-5, 100)).toBe(0);
    expect(relativePercent(10, 0)).toBe(0);
    expect(relativePercent(Number.NaN, 100)).toBe(0);
    expect(formatPercent(58.6666)).toBe("58.7%");
  });
});

describe("<ErrorDrawer />", () => {
  it("renders nothing when the scan reported no errors", () => {
    expect(render(<ErrorDrawer errors={[]} />)).toBe("");
    expect(render(<ErrorDrawer errors={[]} expanded />)).toBe("");
  });

  it("shows the unreadable path count badge while collapsed", () => {
    const report = createMockScanReport();
    const html = render(<ErrorDrawer errors={report.errors} />);

    expect(report.errors.length).toBeGreaterThan(0);
    expect(html).toContain('data-testid="error-count"');
    expect(html).toContain(`${report.errors.length} unreadable paths`);
    expect(html).toMatch(/<button[^>]*aria-expanded="false"/);
    expect(html).not.toContain('data-testid="error-list"');
    expect(html).not.toContain('data-testid="scan-error-row"');
  });

  it("renders every ScanErrorRecord row when expanded", () => {
    const report = createMockScanReport();
    const html = render(<ErrorDrawer errors={report.errors} expanded />);

    expect(html).toMatch(/<button[^>]*aria-expanded="true"/);
    expect(html).toContain('data-testid="error-list"');
    expect(countOf(html, "scan-error-row")).toBe(report.errors.length);

    for (const error of report.errors) {
      expect(html).toContain(error.path);
      expect(html).toContain(error.message);
      expect(html).toContain(describeErrorKind(error.kind));
    }
    expect(html).toContain('data-kind="permission_denied"');
    expect(html).toContain("Permission denied");
    expect(html).toContain("I/O error");
  });

  it("singularises a single unreadable path and labels unknown kinds", () => {
    const html = render(
      <ErrorDrawer errors={[{ path: "/secret", kind: "not_found", message: "no such file" }]} />,
    );
    expect(html).toContain("1 unreadable path");
    expect(html).not.toContain("1 unreadable paths");

    expect(describeErrorKind("permission_denied")).toBe("Permission denied");
    expect(describeErrorKind("not_found")).toBe("Not found");
    expect(describeErrorKind("io_error")).toBe("I/O error");
    expect(describeErrorKind("mystery" as never)).toBe("mystery");
  });
});

describe("ipc adapter", () => {
  it("detects the Tauri runtime and falls back to the mock outside it", async () => {
    expect(typeof window).toBe("undefined");
    expect(isTauriRuntime()).toBe(false);
    expect(await pickDirectory()).toBe(MOCK_SCAN_ROOT);
  });

  it("returns a valid, deterministic ScanReport fixture", () => {
    const report = createMockScanReport();

    expect(report.schema_version).toBe(1);
    expect(report.root_path).toBe(MOCK_SCAN_ROOT);
    expect(report.size_metric).toBe("allocated");
    expect(report.root.summary.path).toBe(MOCK_SCAN_ROOT);

    // Nested directories, top directories, top files and at least one error.
    expect(report.root.children.length).toBeGreaterThan(0);
    expect(nodeAt(report.root, `${MOCK_SCAN_ROOT}/Photos/2024`)).toBeDefined();
    expect(report.root.children.length).toBeGreaterThanOrEqual(4);
    expect(report.top_files.length).toBeGreaterThanOrEqual(4);
    expect(report.errors.length).toBeGreaterThan(0);
    for (const error of report.errors) {
      expect(typeof error.path).toBe("string");
      expect(["permission_denied", "not_found", "io_error"]).toContain(error.kind);
      expect(typeof error.message).toBe("string");
      expect(error.message.length).toBeGreaterThan(0);
    }

    // Deterministic, but a fresh copy on every call.
    expect(createMockScanReport()).toEqual(report);
    expect(createMockScanReport()).not.toBe(report);
    const mutated = createMockScanReport();
    mutated.top_directories.pop();
    expect(createMockScanReport().top_directories.length).toBe(report.top_directories.length);
  });

  it("reconciles parent sizes with child folders and direct file bytes", () => {
    const report = createMockScanReport();
    const check = (node: DirNode) => {
      if (node.children.length === 0) {
        return;
      }
      for (const key of ["apparent_size", "allocated_size"] as const) {
        const sum = node.children.reduce((total, child) => total + child.summary[key], 0);
        const direct = report.top_files.filter(file => file.path.slice(0, file.path.lastIndexOf("/")) === node.summary.path).reduce((total, file) => total + file[key], 0);
        expect(sum + direct).toBe(node.summary[key]);
      }
      node.children.forEach(check);
    };
    check(report.root);
  });

  it("sorts top items by the selected metric and honours the requested options", () => {
    const apparent = createMockScanReport({ root_path: "/tmp/scan/", size_metric: "apparent", top_n: 2 });

    expect(apparent.root_path).toBe("/tmp/scan");
    expect(apparent.size_metric).toBe("apparent");
    expect(apparent.top_directories).toHaveLength(2);
    expect(apparent.top_files).toHaveLength(2);

    for (const summary of apparent.top_directories) {
      expect(summary.path.startsWith("/tmp/scan/")).toBe(true);
    }
    for (const file of apparent.top_files) {
      expect(file.path.startsWith("/tmp/scan/")).toBe(true);
    }
    for (const error of apparent.errors) {
      expect(error.path.startsWith("/tmp/scan/")).toBe(true);
    }

    const sizes = apparent.top_directories.map((summary) => summary.apparent_size);
    expect(sizes).toEqual([...sizes].sort((a, b) => b - a));
  });

  it("streams deterministic progress snapshots that end on the final totals", async () => {
    const expected = createMockScanReport({ size_metric: "apparent" });
    const sequence = createMockProgressSequence(expected);
    expect(sequence).toHaveLength(3);
    expect(sequence[0].files_seen).toBeLessThan(sequence[2].files_seen);

    const seen: ScanProgress[] = [];
    const unlisten = await subscribeScanProgress((progress) => seen.push(progress));

    const report = await startScan(options({ size_metric: "apparent" }));
    expect(report).toEqual(expected);
    expect(seen).toEqual(sequence);
    expect(seen[seen.length - 1].bytes_accumulated).toBe(expected.root.summary.apparent_size);

    unlisten();
    await startScan(options());
    expect(seen).toHaveLength(3);
  });

  it("cancels an in-flight mock scan and stays usable afterwards", async () => {
    const seen: ScanProgress[] = [];
    const unlisten = await subscribeScanProgress((progress) => seen.push(progress));

    const pending = startScan(options());
    await cancelScan();
    await expect(pending).rejects.toThrow(CANCEL_MESSAGE);
    expect(seen).toHaveLength(0);

    const report = await startScan(options());
    expect(report.root_path).toBe(MOCK_SCAN_ROOT);
    expect(seen).toHaveLength(3);
    unlisten();
  });

  it("deduplicates listeners and clearing mock state drops them", async () => {
    let first = 0;
    let second = 0;
    const stopFirst = await subscribeScanProgress(() => {
      first += 1;
    });
    const stopSecond = await subscribeScanProgress(() => {
      second += 1;
    });

    await startScan(options());
    expect(first).toBe(3);
    expect(second).toBe(3);

    stopFirst();
    stopFirst();
    await startScan(options());
    expect(first).toBe(3);
    expect(second).toBe(6);

    resetMockScanState();
    await startScan(options());
    expect(second).toBe(6);
    stopSecond();
  });
});

describe("<App /> wiring", () => {
  it("composes the toolbar, treemap, top items, and error drawer", () => {
    const html = render(<App />);

    expect(html).toContain("DiskGoblin");
    expect(html).toContain('aria-label="Scan controls"');
    expect(html).toContain('aria-label="Scan root path"');
    expect(html).toContain("Choose Folder");
    expect(html).toContain("Run a scan to explore a directory as a treemap.");
    expect(html).toContain("Run a scan to rank the largest directories and files.");
    // Nothing scanned yet: no breadcrumb, no error drawer, no progress banner.
    expect(html).not.toContain('aria-label="Directory breadcrumb"');
    expect(html).not.toContain('aria-label="Scan errors"');
    expect(html).not.toContain('data-testid="scan-progress"');
  });
});


describe("HTTP bridge adapter", () => {
  it("probes config only when enabled in node and resets bridge state", async () => {
    const config = { ...options({ root_path: "/fixture" }), auto_scan: true };
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(new Response(JSON.stringify(config)));
    expect(await fetchUiBootstrapConfig(fetchImpl)).toBeNull();
    expect(fetchImpl).not.toHaveBeenCalled();
    setHttpBridgeEnabled(true);
    expect(await fetchUiBootstrapConfig(fetchImpl)).toEqual(config);
    expect(fetchImpl).toHaveBeenCalledWith("/api/config");
    expect(isHttpBridgeEnabled()).toBe(true);
    resetMockScanState();
    expect(isHttpBridgeEnabled()).toBe(false);
  });

  it("returns null on a failed bootstrap probe", async () => {
    setHttpBridgeEnabled(true);
    expect(await fetchUiBootstrapConfig(vi.fn<typeof fetch>().mockResolvedValue(new Response("", { status: 404 })))).toBeNull();
    expect(await fetchUiBootstrapConfig(vi.fn<typeof fetch>().mockRejectedValue(new TypeError("offline")))).toBeNull();
  });

  it("posts real options and delivers the final progress snapshot", async () => {
    setHttpBridgeEnabled(true);
    const scanOptions = options({ root_path: "/fixture" });
    const report = createMockScanReport(scanOptions);
    const progress = createMockProgressSequence(report)[2];
    const fetchImpl = vi.fn<typeof fetch>().mockImplementation(async (url) =>
      new Response(JSON.stringify(url === "/api/progress" ? progress : report)));
    const seen: ScanProgress[] = [];
    const stop = await subscribeScanProgress(p => seen.push(p));
    expect(await startScan(scanOptions, fetchImpl)).toEqual(report);
    expect(fetchImpl).toHaveBeenCalledWith("/api/scan", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(scanOptions),
    });
    expect(seen).toEqual([progress]);
    stop();
  });

  it("polls while a scan is pending and stops after it resolves", async () => {
    vi.useFakeTimers();
    try {
      setHttpBridgeEnabled(true);
      const report = createMockScanReport();
      const progress = createMockProgressSequence(report)[0];
      let finish!: (response: Response) => void;
      const pendingResponse = new Promise<Response>(resolve => { finish = resolve; });
      const fetchImpl = vi.fn<typeof fetch>().mockImplementation(async url =>
        url === "/api/scan" ? pendingResponse : new Response(JSON.stringify(progress)));
      const seen: ScanProgress[] = [];
      await subscribeScanProgress(p => seen.push(p));
      const pending = startScan(options(), fetchImpl);
      await vi.advanceTimersByTimeAsync(250);
      expect(seen.length).toBeGreaterThan(0);
      finish(new Response(JSON.stringify(report)));
      await pending;
      const calls = fetchImpl.mock.calls.length;
      await vi.advanceTimersByTimeAsync(500);
      expect(fetchImpl).toHaveBeenCalledTimes(calls);
    } finally {
      vi.useRealTimers();
    }
  });

  it("forwards cancellation and folder picking through HTTP", async () => {
    setHttpBridgeEnabled(true);
    const fetchImpl = vi.fn<typeof fetch>().mockImplementation(async url =>
      new Response(JSON.stringify(url === "/api/cancel" ? { cancelled: true } : { path: "/selected" })));
    await cancelScan(fetchImpl);
    expect(fetchImpl).toHaveBeenCalledWith("/api/cancel", { method: "POST" });
    expect(await pickDirectory(fetchImpl)).toBe("/selected");
    expect(fetchImpl).toHaveBeenCalledWith("/api/pick-directory", { method: "POST" });
    expect(await pickDirectory(vi.fn<typeof fetch>().mockResolvedValue(new Response('{"path":null}')))).toBeNull();
  });

  it("surfaces scan errors without substituting mock data", async () => {
    setHttpBridgeEnabled(true);
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(new Response(JSON.stringify({ error: CANCEL_MESSAGE }), { status: 400 }));
    await expect(startScan(options(), fetchImpl)).rejects.toThrow(CANCEL_MESSAGE);
    await expect(startScan(options(), vi.fn<typeof fetch>().mockRejectedValue(new TypeError("offline")))).rejects.toThrow("offline");
  });
});

describe("reveal adapter", () => {
  it("validates paths and records and resets preview selection", async () => {
    await expect(revealPath(" ")).rejects.toThrow("path is required");
    await revealPath("/mock/home/notes.txt");
    expect(getLastRevealedPath()).toBe("/mock/home/notes.txt");
    resetMockScanState();
    expect(getLastRevealedPath()).toBeNull();
  });
  it("posts the selected path and surfaces HTTP reveal failures", async () => {
    setHttpBridgeEnabled(true);
    const fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(new Response('{"revealed":true}'));
    await revealPath("/fixture/file", fetchImpl);
    expect(fetchImpl).toHaveBeenCalledWith("/api/reveal", { method: "POST", headers: { "Content-Type": "application/json" }, body: '{"path":"/fixture/file"}' });
    await expect(revealPath("/missing", vi.fn<typeof fetch>().mockResolvedValue(new Response('{"error":"path does not exist or is inaccessible"}', { status: 400 })))).rejects.toThrow("path does not exist or is inaccessible");
  });
});

function findElement(node: ReactNode, testId: string): ReactElement | undefined {
  if (Array.isArray(node)) return node.map(child => findElement(child, testId)).find(Boolean);
  if (!isValidElement(node)) return undefined;
  if (node.props["data-testid"] === testId) return node;
  return findElement(node.props.children, testId);
}
describe("Largest items input actions", () => {
  it("selects without navigating on click, and opens on double click or Enter", () => {
    const onSelectItem = vi.fn(), onOpenDirectory = vi.fn();
    const report = createMockScanReport();
    const tree = TopItemsPanel({ ...panelProps({ report }), onSelectItem, onOpenDirectory });
    const row = findElement(tree, "top-directory-row")!;
    row.props.onClick();
    expect(onSelectItem).toHaveBeenCalledWith(expect.objectContaining({ path: report.root.children[0].summary.path }));
    expect(onOpenDirectory).not.toHaveBeenCalled();
    row.props.onDoubleClick();
    expect(onOpenDirectory).toHaveBeenCalledWith(report.root.children[0]);
    const preventDefault = vi.fn();
    row.props.onKeyDown({ key: "Enter", preventDefault });
    expect(preventDefault).toHaveBeenCalledOnce();
    expect(onOpenDirectory).toHaveBeenCalledTimes(2);
  });
  it("keeps direct files selectable without trying to navigate", () => {
    const onSelectItem = vi.fn(), onOpenDirectory = vi.fn();
    const row = findElement(TopItemsPanel({ ...panelProps({ tab: "files" }), onSelectItem, onOpenDirectory }), "top-file-row")!;
    row.props.onClick(); row.props.onDoubleClick(); row.props.onKeyDown({ key: "Enter" });
    expect(onSelectItem).toHaveBeenCalledWith(expect.objectContaining({ name: "notes.txt", kind: "file" }));
    expect(onOpenDirectory).not.toHaveBeenCalled();
  });
});
