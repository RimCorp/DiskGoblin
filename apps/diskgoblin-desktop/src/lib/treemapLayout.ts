import { hierarchy, treemap, treemapSquarify } from "d3-hierarchy";

import type { DirNode, FileEntry, ScanReport, SizeMetric } from "../types/scan";

/**
 * A single rectangle to render inside the treemap SVG.
 *
 * `depth` is always zero: every tile represents an immediate child.
 * Synthetic buckets retain their parent path but have no `node`; they carry
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
  item?: ImmediateChildItem;
}

/** Siblings below this share of their parent's total collapse into "Other". */
export const SMALL_ENTRY_RATIO = 0.005;

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

export const TILE_PALETTE = ["#687A3A", "#35766E", "#3E757B", "#9E6B2B", "#7F7837", "#436B5E"] as const;
export const OTHER_TILE_FILL = "#36403D";
export function tileFillForPath(path: string, isOtherBucket = false): string {
  let hash = 2166136261;
  for (const char of path) hash = Math.imul(hash ^ char.charCodeAt(0), 16777619) >>> 0;
  hash = Math.imul(hash ^ (hash >>> 16), 2246822507) >>> 0;
  hash = Math.imul(hash ^ (hash >>> 13), 3266489909) >>> 0;
  hash = (hash ^ (hash >>> 16)) >>> 0;
  return isOtherBucket ? OTHER_TILE_FILL : TILE_PALETTE[hash % TILE_PALETTE.length];
}

export interface ImmediateChildItem {
  id: string;
  name: string;
  path: string;
  kind: "directory" | "file" | "aggregate";
  apparent_size: number;
  allocated_size: number;
  bytes: number;
  percentage: number;
  isAggregate: boolean;
  groupedCount?: number;
  node?: DirNode;
}
export function relativePercent(bytes: number, total: number): number {
  return Number.isFinite(bytes) && Number.isFinite(total) && total > 0
    ? Math.min(100, Math.max(0, bytes / total * 100)) : 0;
}
function compareItems(a: ImmediateChildItem, b: ImmediateChildItem): number {
  return b.bytes - a.bytes || a.path.localeCompare(b.path);
}
export function getImmediateFolders(viewedFolder: DirNode, metric: SizeMetric): ImmediateChildItem[] {
  return viewedFolder.children.map(node => ({
    id: node.summary.path, path: node.summary.path, name: directoryLabel(node.summary.path),
    kind: "directory" as const, apparent_size: node.summary.apparent_size,
    allocated_size: node.summary.allocated_size, bytes: metricSize(node, metric),
    percentage: relativePercent(metricSize(node, metric), metricSize(viewedFolder, metric)),
    isAggregate: false, node,
  })).sort(compareItems);
}
export function parentPath(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const index = normalized.lastIndexOf("/");
  return index === 0 ? "/" : normalized.slice(0, index);
}
export function getImmediateFiles(viewedFolder: DirNode, report: ScanReport | null, metric: SizeMetric): ImmediateChildItem[] {
  const rootPath = viewedFolder.summary.path.replace(/\\/g, "/").replace(/\/+$/, "") || "/";
  const ranked = (report?.top_files ?? []).filter(file => parentPath(file.path) === rootPath);
  const total = metricSize(viewedFolder, metric);
  const items: ImmediateChildItem[] = ranked.map(file => ({
    ...file, id: file.path, name: directoryLabel(file.path), kind: "file", isAggregate: false,
    bytes: file.is_deduplicated_hardlink ? 0 : file[metric + "_size" as "allocated_size" | "apparent_size"],
    percentage: 0,
  }));
  const residual = (key: "apparent_size" | "allocated_size") => Math.max(0,
    viewedFolder.summary[key] - viewedFolder.children.reduce((sum, node) => sum + node.summary[key], 0)
      - ranked.reduce((sum, file) => sum + (file.is_deduplicated_hardlink ? 0 : file[key]), 0));
  const unrankedCount = Math.max(0, viewedFolder.summary.file_count
    - viewedFolder.children.reduce((sum, node) => sum + node.summary.file_count, 0) - ranked.length);
  const apparent = residual("apparent_size"), allocated = residual("allocated_size");
  const bytes = metric === "apparent" ? apparent : allocated;
  if (bytes > 0 && unrankedCount > 0) items.push({
    id: `${viewedFolder.summary.path}::direct`, path: viewedFolder.summary.path,
    name: `Other direct files (${unrankedCount} files)`, kind: "aggregate", isAggregate: true,
    apparent_size: apparent, allocated_size: allocated, bytes, groupedCount: unrankedCount, percentage: 0,
  });
  return items.map(item => ({ ...item, percentage: relativePercent(item.bytes, total) })).sort(compareItems);
}

/** One rectangle per immediate child. Descendants never overlay their ancestors. */
export function computeTreemapLayout(rootNode: DirNode, width: number, height: number,
  metric: SizeMetric, directFiles: FileEntry[] = []): TreemapRect[] {
  if (![width, height].every(n => Number.isFinite(n) && n > 0) || metricSize(rootNode, metric) <= 0) return [];
  const total = metricSize(rootNode, metric);
  const files = getImmediateFiles(rootNode, { top_files: directFiles } as ScanReport, metric);
  const entries = [...getImmediateFolders(rootNode, metric), ...files].filter(item => item.bytes > 0);
  const small = entries.filter(item => item.bytes / total < SMALL_ENTRY_RATIO);
  const tiles = entries.filter(item => item.bytes / total >= SMALL_ENTRY_RATIO);
  if (small.length) tiles.push({
    id: `${rootNode.summary.path}::other`, path: rootNode.summary.path,
    name: `Other (${small.length} ${small.every(item => item.kind === "directory") ? "directories" : "items"})`,
    bytes: small.reduce((sum, item) => sum + item.bytes, 0), kind: "aggregate", isAggregate: true,
    apparent_size: small.reduce((sum, item) => sum + item.apparent_size, 0),
    allocated_size: small.reduce((sum, item) => sum + item.allocated_size, 0),
    groupedCount: small.length, percentage: 0,
  });
  if (!tiles.length) return [];
  const tree = hierarchy<{ children?: ImmediateChildItem[] } & Partial<ImmediateChildItem>>({ children: tiles })
    .sum(item => item.children ? 0 : item.bytes ?? 0)
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0) || (a.data.id ?? "").localeCompare(b.data.id ?? ""));
  return treemap<typeof tree.data>().tile(treemapSquarify).size([width, height]).paddingInner(0)(tree)
    .leaves().map(({ data, x0, y0, x1, y1 }) => ({
      id: data.id!, path: data.path!, name: data.name!, bytes: data.bytes!,
      percentage: relativePercent(data.bytes!, total), x0, y0, x1, y1, depth: 0,
      isOtherBucket: data.isAggregate ?? false, groupedCount: data.groupedCount, node: data.node,
      item: data as ImmediateChildItem,
    }));
}
