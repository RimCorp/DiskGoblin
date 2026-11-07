import { describe, expect, it } from "vitest";

import type { DirNode, SizeMetric } from "../../types/scan";
import {
  buildBreadcrumbSegments,
  computeTreemapLayout,
  directoryLabel,
  findAncestorChain,
  formatBytes,
  SMALL_ENTRY_RATIO,
  getImmediateFolders, getImmediateFiles, tileFillForPath, TILE_PALETTE,
} from "../treemapLayout";
import type { TreemapRect } from "../treemapLayout";

function dirNode(
  path: string,
  apparent: number,
  allocated: number,
  children: DirNode[] = [],
): DirNode {
  return {
    summary: {
      path,
      depth: path.split(/[\\/]/).filter((segment) => segment.length > 0).length,
      apparent_size: apparent,
      allocated_size: allocated,
      file_count: 0,
      dir_count: children.length,
      symlink_count: 0,
      error_count: 0,
    },
    children,
  };
}

function sampleRoot(): DirNode {
  return dirNode("/scan", 1000, 4000, [
    dirNode("/scan/alpha", 500, 2000, [
      dirNode("/scan/alpha/one", 300, 1200),
      dirNode("/scan/alpha/two", 200, 800),
    ]),
    dirNode("/scan/beta", 300, 1200),
    dirNode("/scan/gamma", 200, 800),
  ]);
}

function area(rect: TreemapRect): number {
  return (rect.x1 - rect.x0) * (rect.y1 - rect.y0);
}

function expectFiniteRects(rects: TreemapRect[]): void {
  for (const rect of rects) {
    for (const value of [rect.x0, rect.y0, rect.x1, rect.y1, rect.bytes, rect.percentage]) {
      expect(Number.isFinite(value)).toBe(true);
    }
  }
}

describe("computeTreemapLayout", () => {
  it("lays out deterministic rectangles inside the canvas for both metrics", () => {
    const root = sampleRoot();
    for (const metric of ["allocated", "apparent"] as SizeMetric[]) {
      const rects = computeTreemapLayout(root, 800, 600, metric);
      expectFiniteRects(rects);
      expect(rects.length).toBeGreaterThanOrEqual(3);

      for (const rect of rects) {
        expect(rect.x0).toBeGreaterThanOrEqual(0);
        expect(rect.y0).toBeGreaterThanOrEqual(0);
        expect(rect.x1).toBeLessThanOrEqual(800);
        expect(rect.y1).toBeLessThanOrEqual(600);
        expect(rect.x1).toBeGreaterThanOrEqual(rect.x0);
        expect(rect.y1).toBeGreaterThanOrEqual(rect.y0);
      }

      const topLevel = rects.filter((rect) => rect.depth === 0);
      const covered = topLevel.reduce((total, rect) => total + area(rect), 0);
      expect(Math.abs(covered - 800 * 600) / (800 * 600)).toBeLessThan(1e-9);

      expect(computeTreemapLayout(root, 800, 600, metric)).toEqual(rects);
    }
  });

  it("orders rectangles by size and keeps percentages relative to the zoom root", () => {
    const rects = computeTreemapLayout(sampleRoot(), 1000, 1000, "allocated");
    const alpha = rects.find((rect) => rect.path === "/scan/alpha");
    const beta = rects.find((rect) => rect.path === "/scan/beta");
    const gamma = rects.find((rect) => rect.path === "/scan/gamma");
    expect(alpha && beta && gamma).toBeTruthy();
    if (!alpha || !beta || !gamma) {
      return;
    }

    expect(area(alpha)).toBeGreaterThan(area(beta));
    expect(area(beta)).toBeGreaterThan(area(gamma));
    expect(alpha.percentage).toBeCloseTo(50, 10);
    expect(beta.percentage).toBeCloseTo(30, 10);
    expect(gamma.percentage).toBeCloseTo(20, 10);
    expect(alpha.depth).toBe(0);
    expect(alpha.name).toBe("alpha");
    expect(alpha.node?.summary.path).toBe("/scan/alpha");
  });

  it("returns an empty layout instead of NaN for unusable inputs", () => {
    const root = sampleRoot();
    expect(computeTreemapLayout(root, 0, 600, "allocated")).toEqual([]);
    expect(computeTreemapLayout(root, 800, 0, "allocated")).toEqual([]);
    expect(computeTreemapLayout(root, -10, 600, "allocated")).toEqual([]);
    expect(computeTreemapLayout(root, 800, -10, "allocated")).toEqual([]);
    expect(computeTreemapLayout(root, Number.NaN, 600, "allocated")).toEqual([]);
    expect(computeTreemapLayout(root, 800, Number.POSITIVE_INFINITY, "allocated")).toEqual([]);

    const emptyDir = dirNode("/empty", 0, 0, [dirNode("/empty/child", 0, 0)]);
    expect(computeTreemapLayout(emptyDir, 800, 600, "allocated")).toEqual([]);
    expect(computeTreemapLayout(emptyDir, 800, 600, "apparent")).toEqual([]);

    // A directory measured as empty under one metric still lays out the other.
    const allocatedOnly = dirNode("/sparse", 0, 1024, [dirNode("/sparse/a", 0, 1024)]);
    expect(computeTreemapLayout(allocatedOnly, 800, 600, "apparent")).toEqual([]);
    expect(computeTreemapLayout(allocatedOnly, 800, 600, "allocated")).toHaveLength(1);
  });

  it("shows leaf direct bytes as an explicit aggregate when files are unranked", () => {
    const leaf = dirNode("/scan/only", 4096, 4096);
    leaf.summary.file_count = 2;
    const rects = computeTreemapLayout(leaf, 320, 240, "allocated");
    expect(rects).toHaveLength(1);
    expect(rects[0]).toMatchObject({ bytes: 4096, percentage: 100, isOtherBucket: true });
    expect(rects[0].node).toBeUndefined();
  });

  it("never renders zero-byte children", () => {
    const root = dirNode("/scan", 100, 100, [
      dirNode("/scan/real", 100, 100),
      dirNode("/scan/ghost", 0, 0),
    ]);
    const rects = computeTreemapLayout(root, 400, 300, "allocated");
    expect(rects.some((rect) => rect.path === "/scan/ghost")).toBe(false);
    expectFiniteRects(rects);
  });

  it("aggregates siblings below 0.5% of the parent total into an Other bucket", () => {
    const tiny = Array.from({ length: 5 }, (_, index) => dirNode(`/scan/tiny-${index}`, 4, 4));
    const root = dirNode("/scan", 1000, 1000, [dirNode("/scan/big", 950, 950), ...tiny]);

    const rects = computeTreemapLayout(root, 900, 600, "allocated");
    expectFiniteRects(rects);

    const topLevel = rects.filter((rect) => rect.depth === 0);
    expect(topLevel).toHaveLength(2);

    const bucket = topLevel.find((rect) => rect.isOtherBucket);
    expect(bucket).toBeDefined();
    expect(bucket?.name).toBe("Other (5 directories)");
    expect(bucket?.groupedCount).toBe(5);
    expect(bucket?.bytes).toBe(20);
    expect(bucket?.node).toBeUndefined();
    expect(bucket?.percentage).toBeCloseTo(2, 10);

    expect(topLevel.some((rect) => rect.path.startsWith("/scan/tiny-"))).toBe(false);
    const renderedBytes = topLevel.reduce((total, rect) => total + rect.bytes, 0);
    expect(renderedBytes).toBe(970);
  });

  it("keeps siblings at or above the 0.5% threshold as their own tiles", () => {
    const thresholdChild = Math.round(1000 * SMALL_ENTRY_RATIO);
    const root = dirNode("/scan", 1000, 1000, [
      dirNode("/scan/big", 1000 - thresholdChild, 1000 - thresholdChild),
      dirNode("/scan/edge", thresholdChild, thresholdChild),
    ]);

    const rects = computeTreemapLayout(root, 600, 400, "allocated");
    expect(rects.filter((rect) => rect.depth === 0)).toHaveLength(2);
    expect(rects.some((rect) => rect.isOtherBucket)).toBe(false);
    expect(rects.some((rect) => rect.path === "/scan/edge")).toBe(true);
  });

  it("adds no bucket when every sibling is comfortably above the threshold", () => {
    const rects = computeTreemapLayout(sampleRoot(), 800, 600, "apparent");
    expect(rects.some((rect) => rect.isOtherBucket)).toBe(false);
  });

  it("never overlays grandchildren or double counts descendant bytes", () => {
    const root = sampleRoot();
    const rects = computeTreemapLayout(root, 800, 600, "allocated");
    expect(rects).toHaveLength(3);
    expect(rects.every(rect => rect.depth === 0)).toBe(true);
    expect(rects.reduce((sum, rect) => sum + rect.bytes, 0)).toBe(root.summary.allocated_size);
  });
  it("keeps zero folders discoverable and computes residual direct files in both metrics", () => {
    const root = dirNode("/scan", 1000, 2000, [dirNode("/scan/child", 400, 800), dirNode("/scan/empty", 0, 0)]);
    root.summary.file_count = 4;
    root.children[0].summary.file_count = 1;
    const files = [{ path: "/scan/a.txt", apparent_size: 200, allocated_size: 300, is_deduplicated_hardlink: false }];
    const report = { top_files: [...files, { ...files[0], path: "/scan/child/nested" }] } as import("../../types/scan").ScanReport;
    expect(getImmediateFolders(root, "allocated").map(item => item.name)).toEqual(["child", "empty"]);
    for (const metric of ["allocated", "apparent"] as const) {
      const items = getImmediateFiles(root, report, metric);
      const aggregate = items.find(item => item.isAggregate)!;
      expect(aggregate.groupedCount).toBe(2);
      expect(aggregate.bytes).toBe(metric === "allocated" ? 900 : 400);
      expect(items.some(item => item.path.endsWith("nested"))).toBe(false);
      const rects = computeTreemapLayout(root, 800, 600, metric, files);
      expect(rects.reduce((sum, rect) => sum + rect.bytes, 0)).toBe(metric === "allocated" ? 2000 : 1000);
      expect(rects.reduce((sum, rect) => sum + area(rect), 0)).toBeCloseTo(800 * 600);
    }
  });
  it("assigns stable path colors independently of geometry and measurement", () => {
    const path = "/scan/alpha";
    const fill = tileFillForPath(path);
    expect(TILE_PALETTE).toContain(fill);
    for (const metric of ["allocated", "apparent"] as const) {
      const rect = computeTreemapLayout(sampleRoot(), 1100, 720, metric).find(rect => rect.path === path)!;
      expect(tileFillForPath(rect.path)).toBe(fill);
    }
    expect(tileFillForPath(path, true)).toBe("#36403D");
  });
});

describe("formatBytes", () => {
  it("formats IEC units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-2048)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1 KiB");
    expect(formatBytes(1536)).toBe("1.5 KiB");
    expect(formatBytes(1024 * 1024)).toBe("1 MiB");
    expect(formatBytes(1024 * 1024 * 2.5)).toBe("2.5 MiB");
    expect(formatBytes(1024 ** 3)).toBe("1 GiB");
    expect(formatBytes(1024 ** 4)).toBe("1 TiB");
    expect(formatBytes(1024 ** 5)).toBe("1024 TiB");
    expect(formatBytes(512 * 1024 * 1024 * 1024 * 1024)).toBe("512 TiB");
  });
});

describe("directoryLabel", () => {
  it("returns the trailing path segment across separators", () => {
    expect(directoryLabel("/home/alex/Projects")).toBe("Projects");
    expect(directoryLabel("/home/alex/Projects/")).toBe("Projects");
    expect(directoryLabel("C:\\Users\\alex")).toBe("alex");
    expect(directoryLabel("relative")).toBe("relative");
    expect(directoryLabel("/")).toBe("/");
    expect(directoryLabel("")).toBe("");
  });
});

describe("breadcrumb helpers", () => {
  it("builds labelled segments with unique keys", () => {
    const root = sampleRoot();
    const alpha = root.children[0];
    const chain = findAncestorChain(root, "/scan/alpha/one");
    expect(chain.map((node) => node.summary.path)).toEqual([
      "/scan",
      "/scan/alpha",
      "/scan/alpha/one",
    ]);

    const segments = buildBreadcrumbSegments(chain);
    expect(segments.map((segment) => segment.label)).toEqual(["scan", "alpha", "one"]);
    expect(new Set(segments.map((segment) => segment.key)).size).toBe(3);
    expect(segments[1].node).toBe(alpha);
  });

  it("resolves roots, unknown paths, and partial joins safely", () => {
    const root = sampleRoot();
    expect(findAncestorChain(root, "/scan")).toEqual([root]);
    expect(findAncestorChain(root, "/scan/missing")).toEqual([]);
    expect(findAncestorChain(root, "/elsewhere")).toEqual([]);
    // A sibling with a shared prefix must not be treated as an ancestor.
    const ambiguous = dirNode("/scan", 10, 10, [dirNode("/scan/alpha", 5, 5)]);
    expect(findAncestorChain(ambiguous, "/scan/alpha2")).toEqual([]);
    expect(buildBreadcrumbSegments([])).toEqual([]);
  });
});
