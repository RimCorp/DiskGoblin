import { ItemIcon } from "./ScanToolbar";
import type { DirNode, SizeMetric } from "../types/scan";
import {
  buildBreadcrumbSegments,
  directoryLabel,
  formatBytes,
  metricSize,
} from "../lib/treemapLayout";

export interface BreadcrumbBarProps {
  /** Ancestor chain from the scan root (index 0) to the zoomed node (last). */
  ancestors: DirNode[];
  metric: SizeMetric;
  /** Called when an ancestor segment is clicked to zoom back up to it. */
  onNavigate: (node: DirNode) => void;
}

export default function BreadcrumbBar({ ancestors, metric, onNavigate }: BreadcrumbBarProps) {
  const segments = buildBreadcrumbSegments(ancestors);

  if (segments.length === 0) {
    return null;
  }

  return (
    <nav
      aria-label="Directory breadcrumb"
      style={{
        display: "flex",
        flexWrap: "wrap",
        alignItems: "center",
        gap: "0.25rem",
        padding: "0.375rem 0.5rem",
        border: "1px solid #3C4741",
        borderRadius: "0.375rem",
        fontFamily: "system-ui, sans-serif",
        fontSize: "0.875rem",
        fontVariantNumeric: "tabular-nums",
        marginBottom: "12px",
        background: "#232927",
      }}
    >
      <ItemIcon />
      {segments.map((segment, index) => {
        const isCurrent = index === segments.length - 1;
        const label = segment.label || directoryLabel(segment.path) || segment.path;
        const size = formatBytes(metricSize(segment.node, metric));
        const content = (
          <>
            <span>{label}</span>
            <span style={{ color: "#B5BDB7", marginLeft: "0.375rem" }}>{size}</span>
          </>
        );
        return (
          <span key={segment.key} style={{ display: "inline-flex", alignItems: "center" }}>
            {index > 0 ? (
              <span aria-hidden="true" style={{ color: "#94a3b8", margin: "0 0.25rem" }}>
                /
              </span>
            ) : null}
            {isCurrent ? (
              <span
                aria-current="page"
                title={segment.path}
                style={{ fontWeight: 600, color: "#F2F0E5" }}
              >
                {content}
              </span>
            ) : (
              <button
                type="button"
                title={`Zoom out to ${segment.path}`}
                onClick={() => onNavigate(segment.node)}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  padding: "0.125rem 0.375rem",
                  border: "1px solid transparent",
                  borderRadius: "0.25rem",
                  background: "transparent",
                  color: "#F2F0E5",
                  font: "inherit",
                  cursor: "pointer",
                }}
              >
                {content}
              </button>
            )}
          </span>
        );
      })}
    </nav>
  );
}
