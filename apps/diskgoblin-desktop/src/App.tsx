import { useState } from "react";

import BreadcrumbBar from "./components/BreadcrumbBar";
import TreemapView from "./components/TreemapView";
import { findAncestorChain } from "./lib/treemapLayout";
import type { DirNode, ScanReport, SizeMetric } from "./types/scan";

const METRICS: Array<{ value: SizeMetric; label: string }> = [
  { value: "allocated", label: "Allocated" },
  { value: "apparent", label: "Apparent" },
];

export default function App() {
  // Scan loading is not wired to this scaffold yet, so the treemap starts in its
  // empty state; the zoom and breadcrumb wiring below is driven by `report`.
  const [report] = useState<ScanReport | null>(null);
  const [metric, setMetric] = useState<SizeMetric>("allocated");
  // Ancestor chain from the scan root to the zoomed directory; empty = root view.
  const [zoomChain, setZoomChain] = useState<DirNode[]>([]);

  const ancestors = zoomChain.length > 0 ? zoomChain : report ? [report.root] : [];
  const zoomRoot = ancestors.length > 0 ? ancestors[ancestors.length - 1] : null;

  const zoomTo = (node: DirNode) => {
    if (!report) {
      return;
    }
    if (node.summary.path === report.root.summary.path) {
      setZoomChain([]);
      return;
    }
    const chain = findAncestorChain(report.root, node.summary.path);
    setZoomChain(chain.length > 0 ? chain : [...ancestors, node]);
  };

  return (
    <main
      style={{
        fontFamily: "system-ui, sans-serif",
        maxWidth: "72rem",
        margin: "0 auto",
        padding: "1.5rem",
        display: "flex",
        flexDirection: "column",
        gap: "0.75rem",
      }}
    >
      <header>
        <h1 style={{ margin: 0, fontSize: "1.5rem" }}>DiskGoblin</h1>
        <p style={{ margin: "0.25rem 0 0", color: "#475569" }}>
          Read-only disk usage analysis.
        </p>
      </header>

      <div role="group" aria-label="Size metric" style={{ display: "flex", gap: "0.5rem" }}>
        {METRICS.map((option) => (
          <button
            key={option.value}
            type="button"
            aria-pressed={metric === option.value}
            onClick={() => setMetric(option.value)}
            style={{
              padding: "0.25rem 0.625rem",
              borderRadius: "0.375rem",
              border: `1px solid ${metric === option.value ? "#1d4ed8" : "#cbd5e1"}`,
              background: metric === option.value ? "#dbeafe" : "transparent",
              color: "#0f172a",
              font: "inherit",
              cursor: "pointer",
            }}
          >
            {option.label}
          </button>
        ))}
      </div>

      {report ? (
        <BreadcrumbBar ancestors={ancestors} metric={metric} onNavigate={zoomTo} />
      ) : null}

      <TreemapView rootNode={zoomRoot} metric={metric} onZoom={zoomTo} />
    </main>
  );
}
