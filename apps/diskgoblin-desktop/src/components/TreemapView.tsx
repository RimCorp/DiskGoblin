import { useEffect, useMemo, useRef, useState } from "react";
import type { DirNode, FileEntry, SizeMetric } from "../types/scan";
import { computeTreemapLayout, directoryLabel, formatBytes, tileFillForPath } from "../lib/treemapLayout";
import type { TreemapRect } from "../lib/treemapLayout";
export interface TreemapViewProps {
  rootNode: DirNode | null; metric: SizeMetric; onZoom: (node: DirNode) => void;
  onSelect?: (rect: TreemapRect) => void; selectedId?: string | null; directFiles?: FileEntry[];
  width?: number; height?: number;
}
export default function TreemapView({ rootNode, metric, onZoom, onSelect, selectedId, directFiles, width = 960, height = 540 }: TreemapViewProps) {
  const container = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width, height });
  const [hover, setHover] = useState<TreemapRect | null>(null);
  useEffect(() => {
    if (!container.current || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry.contentRect.width > 0 && entry.contentRect.height > 0) setSize({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(container.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => { setHover(null); }, [rootNode, metric]);
  const rects = useMemo(() => rootNode ? computeTreemapLayout(rootNode, size.width, size.height, metric, directFiles) : [], [rootNode, size, metric, directFiles]);
  const describe = (rect: TreemapRect) => `${rect.name} · ${rect.path} · ${formatBytes(rect.bytes)} · ${rect.percentage.toFixed(1)}%${rect.isOtherBucket ? " · Aggregate: grouped items, not a single file" : ""}`;
  return <div ref={container} className="treemap-container">
    {!rootNode ? <p className="muted">Run a scan to explore a directory as a treemap.</p> : rects.length === 0 ? <p className="muted">No measured size to visualize for {directoryLabel(rootNode.summary.path)}.</p> : <svg role="img" aria-label="Disk usage treemap" viewBox={`0 0 ${size.width} ${size.height}`} style={{ width: "100%", height: "100%", display: "block" }}>
      {rects.map(rect => {
        const w = rect.x1 - rect.x0, h = rect.y1 - rect.y0;
        const selected = selectedId === rect.id;
        const fontSize = Math.max(13, Math.min(22, w / 13, h / 5));
        // White maintains at least 4.5:1 contrast even on the ochre and moss fills.
        const labelFits = w > 88 && h > 62;
        const maxChars = Math.floor((w - 32) / (fontSize * .66));
        const name = rect.name.length > maxChars ? `${rect.name.slice(0, Math.max(1, maxChars - 1))}…` : rect.name;
        return <g key={rect.id}>
          <rect x={rect.x0 + 1.5} y={rect.y0 + 1.5} width={Math.max(0, w - 3)} height={Math.max(0, h - 3)} rx={6}
            fill={tileFillForPath(rect.path, rect.isOtherBucket)} stroke={selected ? "#C4E878" : "#181C1B"} strokeWidth={selected ? 2.5 : 3}
            role="button" tabIndex={0} aria-label={describe(rect)} aria-pressed={selected}
            onMouseEnter={() => setHover(rect)} onMouseLeave={() => setHover(null)} onFocus={() => setHover(rect)} onBlur={() => setHover(null)}
            onClick={() => onSelect?.(rect)} onDoubleClick={() => { if (rect.node) onZoom(rect.node); }}
            onKeyDown={event => { if (event.key === " ") { event.preventDefault(); onSelect?.(rect); } else if (event.key === "Enter" && rect.node) { event.preventDefault(); onZoom(rect.node); } }}><title>{describe(rect)}</title></rect>
          {labelFits && <g pointerEvents="none"><text x={rect.x0 + 16} y={rect.y0 + Math.min(h / 2, 60)} fill="#FFFFFF" fontWeight={700} fontSize={fontSize}>{name}</text><text x={rect.x0 + 16} y={rect.y0 + Math.min(h / 2, 60) + 26} fill="#FFFFFF" fontSize={Math.max(12, fontSize - 3)} style={{ fontVariantNumeric: "tabular-nums" }}>{formatBytes(rect.bytes)}</text></g>}
          {selected && w > 32 && h > 32 && <g pointerEvents="none"><rect x={rect.x1 - 28} y={rect.y0 + 8} width={20} height={20} rx={4} fill="#C4E878"/><text x={rect.x1 - 25} y={rect.y0 + 23} fontSize={17} fill="#181C1B">✓</text></g>}
        </g>;
      })}
    </svg>}
    {hover && <div className="tooltip" role="tooltip"><strong>{hover.name}</strong><div>{hover.path}</div><div>{formatBytes(hover.bytes)} · {hover.percentage.toFixed(1)}%</div>{hover.isOtherBucket && <div>Aggregate: {hover.groupedCount ?? "unranked"} grouped items; no single file path.</div>}</div>}
  </div>;
}
