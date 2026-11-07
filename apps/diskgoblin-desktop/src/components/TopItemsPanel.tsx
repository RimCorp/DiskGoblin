import type { DirNode, ScanReport, SizeMetric } from "../types/scan";
import { findAncestorChain, formatBytes, getImmediateFiles, getImmediateFolders } from "../lib/treemapLayout";
import type { ImmediateChildItem } from "../lib/treemapLayout";
import { ItemIcon } from "./ScanToolbar";
export { relativePercent } from "../lib/treemapLayout";
export type TopItemsTab = "directories" | "files";
export function formatPercent(percentage: number): string { return `${percentage.toFixed(1)}%`; }
export function resolveDirectoryNode(root: DirNode, path: string): DirNode | null {
  const chain = findAncestorChain(root, path);
  return chain[chain.length - 1] ?? null;
}
export interface TopItemsPanelProps {
  report: ScanReport | null;
  viewedFolder?: DirNode | null;
  metric: SizeMetric;
  tab: TopItemsTab;
  onTabChange: (tab: TopItemsTab) => void;
  selectedId?: string | null;
  onSelectItem?: (item: ImmediateChildItem) => void;
  onOpenDirectory?: (node: DirNode) => void;
}
export default function TopItemsPanel({ report, viewedFolder = report?.root, metric, tab, onTabChange, selectedId, onSelectItem, onOpenDirectory }: TopItemsPanelProps) {
  const items = viewedFolder ? tab === "directories" ? getImmediateFolders(viewedFolder, metric) : getImmediateFiles(viewedFolder, report, metric) : [];
  return <section className="panel top-items" aria-label="Largest items">
    <div className="panel-heading"><h2>Largest items</h2><div role="tablist" aria-label="Largest items filter" className="segmented">
      {(["directories", "files"] as const).map(value => <button key={value} role="tab" aria-selected={tab === value} data-testid={`tab-${value}`} onClick={() => onTabChange(value)}>{value === "directories" ? "Folders" : "Files"}</button>)}
    </div></div>
    <div className="items-scroll">
      {!report ? <p className="muted">Run a scan to rank the largest directories and files.</p> : items.length === 0 ? <p className="muted">No immediate {tab === "directories" ? "folders" : "files"} in this folder.</p> : items.map((item, index) => <button
        className="item-row" key={item.id} data-testid={tab === "directories" ? "top-directory-row" : "top-file-row"}
        data-path={item.path} data-selected={selectedId === item.id ? "true" : "false"} aria-pressed={selectedId === item.id}
        title={item.isAggregate ? `${item.name} — unranked direct files` : item.path}
        onClick={() => onSelectItem?.(item)} onDoubleClick={() => { if (item.node) onOpenDirectory?.(item.node); }}
        onKeyDown={event => { if (event.key === "Enter" && item.node) { event.preventDefault(); onOpenDirectory?.(item.node); } }}>
        <span className="rank">{index + 1}.</span><ItemIcon file={item.kind === "file"} />
        <span className="item-details"><strong>{item.name}</strong><span className="bar-track"><span data-testid="size-bar" data-percent={item.percentage.toFixed(1)} style={{ width: `${item.percentage}%` }} /></span></span>
        <span className="item-size">{formatBytes(item.bytes)}<small>{formatPercent(item.percentage)}</small></span>
      </button>)}
    </div>
  </section>;
}
