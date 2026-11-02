import { useMemo, useState } from "react";

import type { DirNode, SizeMetric } from "../types/scan";
import { computeTreemapLayout, directoryLabel, formatBytes } from "../lib/treemapLayout";
import type { TreemapRect } from "../lib/treemapLayout";

const DEPTH_FILLS = ["#2563eb", "#0e7490", "#6d28d9", "#b45309"];
const OTHER_FILL = "#64748b";
const STROKE = "#0f172a";

const LABEL_MIN_WIDTH = 48;
const LABEL_MIN_HEIGHT = 18;
const SIZE_MIN_WIDTH = 64;
const SIZE_MIN_HEIGHT = 34;
const CHAR_WIDTH = 6.4;

export interface TreemapViewProps {
  /** Directory currently zoomed to; its children are rendered. */
  rootNode: DirNode | null;
  metric: SizeMetric;
  /** Called with the clicked (non-bucket) directory to drill down into. */
  onZoom: (node: DirNode) => void;
  width?: number;
  height?: number;
}

interface HoverState {
  rect: TreemapRect;
  clientX: number;
  clientY: number;
}

function fillFor(rect: TreemapRect): string {
  if (rect.isOtherBucket) {
    return OTHER_FILL;
  }
  return DEPTH_FILLS[rect.depth % DEPTH_FILLS.length];
}

function truncate(text: string, maxWidth: number): string {
  const maxChars = Math.floor(maxWidth / CHAR_WIDTH);
  if (maxChars <= 1) {
    return "";
  }
  if (text.length <= maxChars) {
    return text;
  }
  return `${text.slice(0, maxChars - 1)}…`;
}

function describe(rect: TreemapRect): string {
  const parts = [rect.path || rect.name, formatBytes(rect.bytes), `${rect.percentage.toFixed(1)}%`];
  if (rect.isOtherBucket && rect.groupedCount !== undefined) {
    parts.push(`${rect.groupedCount} grouped directories`);
  }
  return parts.join(" · ");
}

export default function TreemapView({
  rootNode,
  metric,
  onZoom,
  width = 960,
  height = 540,
}: TreemapViewProps) {
  const [hover, setHover] = useState<HoverState | null>(null);

  const rects = useMemo(() => {
    if (!rootNode) {
      return [];
    }
    return computeTreemapLayout(rootNode, width, height, metric);
  }, [rootNode, width, height, metric]);

  if (!rootNode) {
    return (
      <div style={{ padding: "1rem", color: "#475569" }}>
        Run a scan to explore a directory as a treemap.
      </div>
    );
  }

  if (rects.length === 0) {
    return (
      <div style={{ padding: "1rem", color: "#475569" }}>
        No measured size to visualize for {directoryLabel(rootNode.summary.path) || rootNode.summary.path}.
      </div>
    );
  }

  return (
    <div style={{ position: "relative", width: "100%" }}>
      <svg
        role="img"
        aria-label="Disk usage treemap"
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="xMidYMid meet"
        style={{ width: "100%", height: "auto", display: "block", background: "#0b1220" }}
      >
        {rects.map((rect) => {
          const boxWidth = rect.x1 - rect.x0;
          const boxHeight = rect.y1 - rect.y0;
          if (boxWidth <= 0 || boxHeight <= 0) {
            return null;
          }
          const clickable = !rect.isOtherBucket && rect.node !== undefined;
          const isHovered = hover?.rect.id === rect.id;
          const showLabel = boxWidth >= LABEL_MIN_WIDTH && boxHeight >= LABEL_MIN_HEIGHT;
          const showSize = boxWidth >= SIZE_MIN_WIDTH && boxHeight >= SIZE_MIN_HEIGHT;
          return (
            <g key={`${rect.depth}:${rect.id}`}>
              <rect
                x={rect.x0}
                y={rect.y0}
                width={boxWidth}
                height={boxHeight}
                fill={fillFor(rect)}
                fillOpacity={rect.isOtherBucket ? 0.75 : 0.85}
                stroke={STROKE}
                strokeWidth={isHovered ? 2 : 1}
                style={{ cursor: clickable ? "zoom-in" : "default" }}
                tabIndex={clickable ? 0 : undefined}
                role={clickable ? "button" : undefined}
                aria-label={describe(rect)}
                onMouseMove={(event) =>
                  setHover({
                    rect,
                    clientX: event.clientX,
                    clientY: event.clientY,
                  })
                }
                onMouseLeave={() => setHover(null)}
                onClick={() => {
                  if (clickable && rect.node) {
                    onZoom(rect.node);
                  }
                }}
                onKeyDown={(event) => {
                  if (!clickable || !rect.node) {
                    return;
                  }
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onZoom(rect.node);
                  }
                }}
              >
                <title>{describe(rect)}</title>
              </rect>
              {showLabel ? (
                <text
                  x={rect.x0 + 4}
                  y={rect.y0 + 14}
                  fill="#f8fafc"
                  fontSize={11}
                  fontFamily="system-ui, sans-serif"
                  pointerEvents="none"
                >
                  {truncate(rect.name, boxWidth - 8)}
                </text>
              ) : null}
              {showSize ? (
                <text
                  x={rect.x0 + 4}
                  y={rect.y0 + 28}
                  fill="#dbeafe"
                  fontSize={10}
                  fontFamily="system-ui, sans-serif"
                  pointerEvents="none"
                >
                  {truncate(formatBytes(rect.bytes), boxWidth - 8)}
                </text>
              ) : null}
            </g>
          );
        })}
      </svg>
      {hover ? (
        <div
          role="tooltip"
          style={{
            position: "fixed",
            top: hover.clientY + 12,
            left: hover.clientX + 12,
            zIndex: 10,
            pointerEvents: "none",
            maxWidth: "22rem",
            padding: "0.5rem 0.625rem",
            borderRadius: "0.375rem",
            background: "#0f172a",
            color: "#f8fafc",
            boxShadow: "0 6px 20px rgba(15, 23, 42, 0.35)",
            fontSize: "0.75rem",
            lineHeight: 1.35,
            whiteSpace: "normal",
            wordBreak: "break-all",
          }}
        >
          <div style={{ fontWeight: 600 }}>{hover.rect.name}</div>
          <div>{hover.rect.path}</div>
          <div>
            {formatBytes(hover.rect.bytes)} · {hover.rect.percentage.toFixed(2)}%
          </div>
          {hover.rect.isOtherBucket && hover.rect.groupedCount !== undefined ? (
            <div style={{ color: "#cbd5e1" }}>
              {hover.rect.groupedCount} directories grouped as "Other"
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
