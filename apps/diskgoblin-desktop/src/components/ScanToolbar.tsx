import type { ScanProgress, ScanReport, SizeMetric } from "../types/scan";
import { formatBytes, metricSize } from "../lib/treemapLayout";
export function ItemIcon({ file = false }: { file?: boolean }) {
  return <svg aria-hidden="true" width="25" height="25" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">{file ? <path d="M6 3h8l4 4v14H6z M14 3v5h4" /> : <path d="M3 6h6l3 3h9v11H3z" />}</svg>;
}
export function GoblinMark() {
  return <svg aria-hidden="true" width="36" height="32" viewBox="0 0 64 52" fill="#C4E878"><path d="M17 15 2 6l9 22 9 3c3 14 23 20 30 0l6-3 6-22-16 9C37 5 25 5 17 15Z"/><path fill="#181C1B" d="m20 23 9 3-4 5zm24 0-9 3 4 5zM24 36q8 7 16 0l-3 7H27z"/></svg>;
}
export function formatCount(value: number): string {
  return Number.isFinite(value) ? Math.round(value).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",") : "0";
}
export function describeProgress(progress: ScanProgress, scanning: boolean): string {
  return `${scanning ? "Scanning:" : "Last scan:"} · ${formatCount(progress.files_seen)} files · ${formatCount(progress.dirs_seen)} dirs · ${formatCount(progress.errors_seen)} errors · ${formatBytes(progress.bytes_accumulated)}`;
}
export interface ScanToolbarProps {
  path: string; onPathChange: (path: string) => void; metric: SizeMetric;
  onMetricChange: (metric: SizeMetric) => void; scanning: boolean; progress: ScanProgress | null;
  report?: ScanReport | null; onScan: () => void; onCancel: () => void; onPickDirectory: () => void;
}
export default function ScanToolbar({ path, onPathChange, metric, onMetricChange, scanning, progress, report, onScan, onCancel, onPickDirectory }: ScanToolbarProps) {
  return <section aria-label="Scan controls" className="toolbar">
    <div className="toolbar-controls"><div className="brand"><GoblinMark /><span>DiskGoblin</span></div>
      <label className="path-control"><ItemIcon /><input aria-label="Scan root path" value={path} onChange={e => onPathChange(e.target.value)} placeholder="/path/to/scan" /></label>
      <button onClick={onPickDirectory} disabled={scanning}>Choose Folder</button>
      <button onClick={onScan} disabled={scanning || !path.trim()}>{report ? "Rescan" : "Scan"}</button>
      <button onClick={onCancel} disabled={!scanning}>Cancel</button>
      <div role="group" aria-label="Size metric" className="segmented">{(["allocated", "apparent"] as const).map(value => <button key={value} aria-pressed={metric === value} onClick={() => onMetricChange(value)}>{value === "allocated" ? "Allocated" : "Apparent"}</button>)}</div>
    </div>
    {(report || progress) && <div className="scan-summary"><span>Whole scan · </span>{report ? `${formatBytes(metricSize(report.root, metric))} scanned · ${formatCount(report.root.summary.file_count)} files · ${formatCount(report.root.summary.dir_count)} folders · ${formatCount(report.errors.length)} errors` : "Scan in progress"}
      {progress && <span data-testid="scan-progress" role="status">{describeProgress(progress, scanning)}</span>}
    </div>}
  </section>;
}
