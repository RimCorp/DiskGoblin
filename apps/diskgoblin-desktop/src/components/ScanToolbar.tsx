import type { ScanProgress, SizeMetric } from "../types/scan";
import { formatBytes } from "../lib/treemapLayout";

const METRICS: Array<{ value: SizeMetric; label: string }> = [
  { value: "allocated", label: "Allocated" },
  { value: "apparent", label: "Apparent" },
];

/** Thousands-separated integer, independent of host locale (deterministic). */
export function formatCount(value: number): string {
  if (!Number.isFinite(value)) {
    return "0";
  }
  const rounded = Math.round(value);
  const digits = Math.abs(rounded).toString();
  return `${rounded < 0 ? "-" : ""}${digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}`;
}

/**
 * Single-line progress summary. Built as one string so the rendered markup holds
 * a single text node (no SSR comment separators between counters).
 */
export function describeProgress(progress: ScanProgress, scanning: boolean): string {
  const label = scanning ? "Scanning" : "Last scan";
  return [
    `${label}:`,
    `${formatCount(progress.files_seen)} files`,
    `${formatCount(progress.dirs_seen)} dirs`,
    `${formatCount(progress.errors_seen)} errors`,
    formatBytes(progress.bytes_accumulated),
  ].join(" · ");
}

export interface ScanToolbarProps {
  /** Root path being scanned, bound to the text input. */
  path: string;
  onPathChange: (path: string) => void;
  metric: SizeMetric;
  onMetricChange: (metric: SizeMetric) => void;
  /** True while a scan is running, which disables Scan and enables Cancel. */
  scanning: boolean;
  /** Latest progress snapshot; `null` before the first update. */
  progress: ScanProgress | null;
  onScan: () => void;
  onCancel: () => void;
  onPickDirectory: () => void;
}

const buttonStyle = {
  padding: "0.3rem 0.75rem",
  borderRadius: "0.375rem",
  border: "1px solid #cbd5e1",
  background: "#f8fafc",
  color: "#0f172a",
  font: "inherit",
  cursor: "pointer",
} as const;

export default function ScanToolbar({
  path,
  onPathChange,
  metric,
  onMetricChange,
  scanning,
  progress,
  onScan,
  onCancel,
  onPickDirectory,
}: ScanToolbarProps) {
  const canScan = !scanning && path.trim().length > 0;
  const statusText = progress ? describeProgress(progress, scanning) : "";

  return (
    <section
      aria-label="Scan controls"
      style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}
    >
      <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem", alignItems: "center" }}>
        <input
          type="text"
          aria-label="Scan root path"
          placeholder="/path/to/scan"
          value={path}
          onChange={(event) => onPathChange(event.target.value)}
          style={{
            flex: "1 1 18rem",
            minWidth: "12rem",
            padding: "0.3rem 0.5rem",
            border: "1px solid #cbd5e1",
            borderRadius: "0.375rem",
            font: "inherit",
          }}
        />
        <button type="button" onClick={onPickDirectory} style={buttonStyle}>
          Choose Folder
        </button>
        <div role="group" aria-label="Size metric" style={{ display: "flex", gap: "0.25rem" }}>
          {METRICS.map((option) => (
            <button
              key={option.value}
              type="button"
              aria-pressed={metric === option.value}
              onClick={() => onMetricChange(option.value)}
              style={{
                ...buttonStyle,
                borderColor: metric === option.value ? "#1d4ed8" : "#cbd5e1",
                background: metric === option.value ? "#dbeafe" : "transparent",
              }}
            >
              {option.label}
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={onScan}
          disabled={!canScan}
          style={{ ...buttonStyle, background: canScan ? "#1d4ed8" : "#e2e8f0", color: canScan ? "#f8fafc" : "#64748b" }}
        >
          Scan
        </button>
        <button type="button" onClick={onCancel} disabled={!scanning} style={buttonStyle}>
          Cancel
        </button>
      </div>

      {progress ? (
        <p
          role="status"
          data-testid="scan-progress"
          style={{ margin: 0, fontSize: "0.8125rem", color: scanning ? "#1d4ed8" : "#475569" }}
        >
          {statusText}
        </p>
      ) : null}
    </section>
  );
}
