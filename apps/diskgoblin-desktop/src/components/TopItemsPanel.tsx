import type { DirNode, ScanReport, SizeMetric } from "../types/scan";
import { directoryLabel, findAncestorChain, formatBytes, metricSize } from "../lib/treemapLayout";

export type TopItemsTab = "directories" | "files";

const TABS: Array<{ value: TopItemsTab; label: string }> = [
  { value: "directories", label: "Top Directories" },
  { value: "files", label: "Top Files" },
];

/**
 * Resolves a table path back to its `DirNode`. Returns `null` when the path is
 * not part of the scanned tree, in which case a row is not clickable.
 */
export function resolveDirectoryNode(root: DirNode, path: string): DirNode | null {
  if (!root || !path) {
    return null;
  }
  const exact = findAncestorChain(root, path);
  if (exact.length === 0) {
    return null;
  }
  const node = exact[exact.length - 1];
  return node.summary.path === path ? node : null;
}

/** Share of the scan root's total, clamped to `[0, 100]` for the visual bar. */
export function relativePercent(bytes: number, rootTotal: number): number {
  if (!Number.isFinite(bytes) || !Number.isFinite(rootTotal) || rootTotal <= 0) {
    return 0;
  }
  return Math.min(100, Math.max(0, (bytes / rootTotal) * 100));
}

/** Formats a percentage with one decimal for display. */
export function formatPercent(percentage: number): string {
  return `${percentage.toFixed(1)}%`;
}

export interface TopItemsPanelProps {
  report: ScanReport | null;
  metric: SizeMetric;
  /** Active tab; controlled so callers (and tests) can pin either view. */
  tab: TopItemsTab;
  onTabChange: (tab: TopItemsTab) => void;
  /** Called with a directory path that exists in the scanned tree. */
  onSelectDirectory: (path: string) => void;
}

const cellStyle = {
  padding: "0.3rem 0.5rem",
  borderBottom: "1px solid #e2e8f0",
  textAlign: "left" as const,
  verticalAlign: "top" as const,
  fontSize: "0.8125rem",
};

function SizeBar({ percentage }: { percentage: number }) {
  return (
    <div
      aria-hidden="true"
      style={{ background: "#e2e8f0", borderRadius: "0.25rem", height: "0.4rem", width: "100%" }}
    >
      <div
        data-testid="size-bar"
        data-percent={percentage.toFixed(1)}
        style={{
          width: `${percentage}%`,
          minWidth: percentage > 0 ? "2px" : "0",
          height: "100%",
          borderRadius: "0.25rem",
          background: "#2563eb",
        }}
      />
    </div>
  );
}

export default function TopItemsPanel({
  report,
  metric,
  tab,
  onTabChange,
  onSelectDirectory,
}: TopItemsPanelProps) {
  const rootTotal = report ? metricSize(report.root, metric) : 0;

  return (
    <section
      aria-label="Top directories and files"
      style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}
    >
      <div role="tablist" aria-label="Top items" style={{ display: "flex", gap: "0.25rem" }}>
        {TABS.map((option) => (
          <button
            key={option.value}
            type="button"
            role="tab"
            aria-selected={tab === option.value}
            data-testid={`tab-${option.value}`}
            onClick={() => onTabChange(option.value)}
            style={{
              padding: "0.25rem 0.625rem",
              borderRadius: "0.375rem",
              border: `1px solid ${tab === option.value ? "#1d4ed8" : "#cbd5e1"}`,
              background: tab === option.value ? "#dbeafe" : "transparent",
              color: "#0f172a",
              font: "inherit",
              cursor: "pointer",
            }}
          >
            {option.label}
          </button>
        ))}
      </div>

      {!report ? (
        <p style={{ margin: 0, color: "#475569", fontSize: "0.8125rem" }}>
          Run a scan to rank the largest directories and files.
        </p>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th scope="col" style={cellStyle}>
                {tab === "directories" ? "Directory" : "File"}
              </th>
              <th scope="col" style={cellStyle}>
                Size
              </th>
              <th scope="col" style={{ ...cellStyle, width: "40%" }}>
                Share of root
              </th>
            </tr>
          </thead>
          <tbody>
            {tab === "directories"
              ? report.top_directories.map((summary) => {
                  const bytes = metric === "apparent" ? summary.apparent_size : summary.allocated_size;
                  const percentage = relativePercent(bytes, rootTotal);
                  const node = resolveDirectoryNode(report.root, summary.path);
                  const present = node !== null;
                  return (
                    <tr
                      key={summary.path}
                      data-testid="top-directory-row"
                      data-path={summary.path}
                      data-present={present ? "true" : "false"}
                    >
                      <td style={cellStyle}>
                        <button
                          type="button"
                          disabled={!present}
                          onClick={() => onSelectDirectory(summary.path)}
                          title={present ? `Zoom to ${summary.path}` : `${summary.path} is not in the scanned tree`}
                          style={{
                            display: "flex",
                            flexDirection: "column",
                            alignItems: "flex-start",
                            gap: "0.125rem",
                            padding: 0,
                            border: "none",
                            background: "transparent",
                            font: "inherit",
                            textAlign: "left",
                            cursor: present ? "pointer" : "default",
                            color: present ? "#1d4ed8" : "#475569",
                          }}
                        >
                          <span style={{ fontWeight: 600 }}>
                            {directoryLabel(summary.path) || summary.path}
                          </span>
                          <span style={{ color: "#64748b", fontSize: "0.75rem", wordBreak: "break-all" }}>
                            {summary.path}
                          </span>
                        </button>
                      </td>
                      <td style={cellStyle}>{formatBytes(bytes)}</td>
                      <td style={cellStyle}>
                        <SizeBar percentage={percentage} />
                        <span data-testid="percentage" style={{ color: "#475569", fontSize: "0.75rem" }}>
                          {formatPercent(percentage)}
                        </span>
                      </td>
                    </tr>
                  );
                })
              : report.top_files.map((file) => {
                  const bytes = metric === "apparent" ? file.apparent_size : file.allocated_size;
                  const percentage = relativePercent(bytes, rootTotal);
                  return (
                    <tr key={file.path} data-testid="top-file-row" data-path={file.path}>
                      <td style={cellStyle}>
                        <span style={{ display: "flex", flexDirection: "column", gap: "0.125rem" }}>
                          <span style={{ fontWeight: 600 }}>
                            {directoryLabel(file.path) || file.path}
                          </span>
                          <span style={{ color: "#64748b", fontSize: "0.75rem", wordBreak: "break-all" }}>
                            {file.path}
                          </span>
                        </span>
                      </td>
                      <td style={cellStyle}>{formatBytes(bytes)}</td>
                      <td style={cellStyle}>
                        <SizeBar percentage={percentage} />
                        <span data-testid="percentage" style={{ color: "#475569", fontSize: "0.75rem" }}>
                          {formatPercent(percentage)}
                        </span>
                      </td>
                    </tr>
                  );
                })}
          </tbody>
        </table>
      )}

      {report && tab === "directories" && report.top_directories.length === 0 ? (
        <p style={{ margin: 0, color: "#475569", fontSize: "0.8125rem" }}>No directories recorded.</p>
      ) : null}
      {report && tab === "files" && report.top_files.length === 0 ? (
        <p style={{ margin: 0, color: "#475569", fontSize: "0.8125rem" }}>No files recorded.</p>
      ) : null}
    </section>
  );
}
