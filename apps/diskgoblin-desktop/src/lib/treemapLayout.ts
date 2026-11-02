import { hierarchy, treemap, treemapSquarify } from "d3-hierarchy";

import type { DirNode, SizeMetric } from "../types/scan";

/**
 * A single rectangle to render inside the treemap SVG.
 *
 * `depth` is relative to the zoomed root: `0` covers the zoomed root's own
 * children, `1` covers grandchildren rendered inside their parent tile.
 * `path` and `node` are absent for synthetic "Other" buckets, which carry
 * `isOtherBucket: true` and the number of directories they hide.
 */
export interface TreemapRect {
  id: string;
  path: string;
  name: string;
  bytes: number;
  percentage: number;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  depth: number;
  isOtherBucket: boolean;
  groupedCount?: number;
  node?: DirNode;
}

/** Siblings below this share of their parent's total collapse into "Other". */
export const SMALL_ENTRY_RATIO = 0.005;

/** Levels rendered per view: the zoomed root's children, plus grandchildren. */
const MAX_DEPTH_LEVELS = 2;

/** Upper bound on rectangles per view so the SVG DOM stays fast (ADR-006). */
const MAX_RECTS = 400;

/** Tiles smaller than this on either axis are not subdivided any further. */
const MIN_EXPAND_PX = 48;

const BYTE_UNITS = ["B", "KiB", "MiB", "GiB", "TiB"] as const;

/** The size of a node under the selected metric, clamped to a usable number. */
export function metricSize(node: DirNode, metric: SizeMetric): number {
  const raw =
    metric === "apparent" ? node.summary.apparent_size : node.summary.allocated_size;
  return Number.isFinite(raw) && raw > 0 ? raw : 0;
}

/** Human-readable IEC byte label, e.g. `1.5 KiB` or `3 MiB`. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B";
  }
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) {
    return `${Math.round(value)} B`;
  }
  const rounded = value >= 100 ? value.toFixed(0) : value.toFixed(1);
  return `${rounded.replace(/\.0$/, "")} ${BYTE_UNITS[unit]}`;
}

/** Trailing path segment, tolerating both `/` and `\` separators. */
export function directoryLabel(path: string): string {
  if (!path) {
    return "";
  }
  const trimmed = path.length > 1 ? path.replace(/[\\/]+$/, "") : path;
  const parts = trimmed.split(/[\\/]+/);
  const last = parts[parts.length - 1];
  return last && last.length > 0 ? last : trimmed || path;
}

export interface BreadcrumbSegment {
  key: string;
  label: string;
  path: string;
  depth: number;
  node: DirNode;
}

/**
 * Turns the ancestor chain (scan root first, zoomed node last) into
 * breadcrumb segments. Keys are unique even when a path repeats a basename.
 */
export function buildBreadcrumbSegments(ancestors: DirNode[]): BreadcrumbSegment[] {
  return ancestors.map((node, index) => {
    const path = node.summary.path;
    return {
      key: `${index}:${path}`,
      label: directoryLabel(path) || path,
      path,
      depth: index,
      node,
    };
  });
}

function isPathWithin(candidate: string, ancestor: string): boolean {
  if (candidate === ancestor) {
    return true;
  }
  if (!candidate.startsWith(ancestor)) {
    return false;
  }
  const separatorTerminated =
    ancestor.endsWith("/") || ancestor.endsWith("\\") || ancestor.length === 0;
  return separatorTerminated || candidate[ancestor.length] === "/" || candidate[ancestor.length] === "\\";
}

/**
 * Ancestor chain from `root` down to `targetPath` (inclusive), used to rebuild
 * breadcrumb state after a drill-down. Returns `[]` when the path is not in
 * the tree.
 */
export function findAncestorChain(root: DirNode, targetPath: string): DirNode[] {
  if (!root || !root.summary || !targetPath) {
    return [];
  }
  if (root.summary.path === targetPath) {
    return [root];
  }
  for (const child of root.children) {
    const childPath = child.summary.path;
    if (!isPathWithin(targetPath, childPath)) {
      continue;
    }
    const chain = findAncestorChain(child, targetPath);
    if (chain.length > 0) {
      return [root, ...chain];
    }
  }
  return [];
}

interface LeafTile {
  id: string;
  path: string;
  name: string;
  bytes: number;
  isOtherBucket: boolean;
  groupedCount?: number;
  node?: DirNode;
}

interface TileNode extends LeafTile {
  children?: TileNode[];
}

interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

function compareTiles(a: LeafTile, b: LeafTile): number {
  return b.bytes - a.bytes || a.id.localeCompare(b.id);
}

/**
 * Builds the tiles for one directory level: real child directories sorted by
 * size, plus a synthetic "Other" bucket when any sibling is under
 * `SMALL_ENTRY_RATIO` of the parent's total. Zero-byte children are dropped so
 * they never produce zero-area rectangles.
 */
function buildLevelTiles(parent: DirNode, metric: SizeMetric): LeafTile[] {
  const parentTotal = metricSize(parent, metric);
  if (parentTotal <= 0) {
    return [];
  }

  const children = parent.children
    .map((child) => ({ child, bytes: metricSize(child, metric) }))
    .filter((entry) => entry.bytes > 0)
    .sort(
      (a, b) =>
        b.bytes - a.bytes || a.child.summary.path.localeCompare(b.child.summary.path),
    );
  if (children.length === 0) {
    return [];
  }

  const threshold = parentTotal * SMALL_ENTRY_RATIO;
  const tiles: LeafTile[] = [];
  const small: Array<{ child: DirNode; bytes: number }> = [];

  for (const entry of children) {
    if (entry.bytes < threshold) {
      small.push(entry);
      continue;
    }
    const path = entry.child.summary.path;
    tiles.push({
      id: path,
      path,
      name: directoryLabel(path),
      bytes: entry.bytes,
      isOtherBucket: false,
      node: entry.child,
    });
  }

  if (small.length === 0) {
    return tiles;
  }

  const smallBytes = small.reduce((total, entry) => total + entry.bytes, 0);
  tiles.push({
    id: `${parent.summary.path}::other`,
    path: parent.summary.path,
    name: `Other (${small.length} directories)`,
    bytes: smallBytes,
    isOtherBucket: true,
    groupedCount: small.length,
  });

  return tiles.sort(compareTiles);
}

/** Squarified layout of one level of tiles inside a `width` x `height` box. */
function squarify(
  tiles: LeafTile[],
  width: number,
  height: number,
): Array<{ tile: LeafTile } & Box> {
  if (tiles.length === 0 || !Number.isFinite(width) || !Number.isFinite(height)) {
    return [];
  }
  if (width <= 0 || height <= 0) {
    return [];
  }

  const wrapper: TileNode = {
    id: "__root__",
    path: "",
    name: "",
    bytes: 0,
    isOtherBucket: false,
    children: tiles as TileNode[],
  };

  const layout = treemap<TileNode>()
    .tile(treemapSquarify)
    .size([width, height])
    .paddingInner(0)
    .round(false);

  const laidOut = layout(
    hierarchy<TileNode>(wrapper, (node) => node.children)
      .sum((node) => (node.children && node.children.length > 0 ? 0 : node.bytes))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0) || a.data.id.localeCompare(b.data.id)),
  );

  const boxes: Array<{ tile: LeafTile } & Box> = [];
  for (const leaf of laidOut.leaves()) {
    const { x0, y0, x1, y1 } = leaf;
    if (![x0, y0, x1, y1].every((value) => Number.isFinite(value))) {
      continue;
    }
    boxes.push({ tile: leaf.data, x0, y0, x1, y1 });
  }
  return boxes;
}

function appendLevel(
  tiles: LeafTile[],
  offsetX: number,
  offsetY: number,
  width: number,
  height: number,
  depth: number,
  out: TreemapRect[],
  metric: SizeMetric,
  rootTotal: number,
): void {
  const boxes = squarify(tiles, width, height);

  for (const box of boxes) {
    const { tile } = box;
    out.push({
      id: tile.id,
      path: tile.path,
      name: tile.name,
      bytes: tile.bytes,
      percentage: rootTotal > 0 ? (tile.bytes / rootTotal) * 100 : 0,
      x0: offsetX + box.x0,
      y0: offsetY + box.y0,
      x1: offsetX + box.x1,
      y1: offsetY + box.y1,
      depth,
      isOtherBucket: tile.isOtherBucket,
      groupedCount: tile.groupedCount,
      node: tile.node,
    });
  }

  if (depth + 1 >= MAX_DEPTH_LEVELS) {
    return;
  }

  for (const box of boxes) {
    const { tile } = box;
    if (tile.isOtherBucket || !tile.node) {
      continue;
    }
    const boxWidth = box.x1 - box.x0;
    const boxHeight = box.y1 - box.y0;
    if (boxWidth < MIN_EXPAND_PX || boxHeight < MIN_EXPAND_PX) {
      continue;
    }
    const childTiles = buildLevelTiles(tile.node, metric);
    if (childTiles.length < 2 || out.length + childTiles.length > MAX_RECTS) {
      continue;
    }
    appendLevel(
      childTiles,
      offsetX + box.x0,
      offsetY + box.y0,
      boxWidth,
      boxHeight,
      depth + 1,
      out,
      metric,
      rootTotal,
    );
  }
}

/**
 * Deterministic squarified treemap rectangles for the zoomed directory.
 *
 * Returns `[]` for non-positive or non-finite canvas sizes and for nodes whose
 * selected metric size is zero, so callers never see `NaN` coordinates.
 */
export function computeTreemapLayout(
  rootNode: DirNode,
  width: number,
  height: number,
  metric: SizeMetric,
): TreemapRect[] {
  if (!rootNode || !rootNode.summary) {
    return [];
  }
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    return [];
  }

  const rootTotal = metricSize(rootNode, metric);
  if (rootTotal <= 0) {
    return [];
  }

  const rootPath = rootNode.summary.path;
  const childTiles = buildLevelTiles(rootNode, metric);
  if (childTiles.length === 0) {
    return [
      {
        id: rootPath,
        path: rootPath,
        name: directoryLabel(rootPath) || rootPath,
        bytes: rootTotal,
        percentage: 100,
        x0: 0,
        y0: 0,
        x1: width,
        y1: height,
        depth: 0,
        isOtherBucket: false,
        node: rootNode,
      },
    ];
  }

  const rects: TreemapRect[] = [];
  appendLevel(childTiles, 0, 0, width, height, 0, rects, metric, rootTotal);
  return rects;
}
