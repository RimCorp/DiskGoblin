import type { ScanErrorKind, ScanErrorRecord } from "../types/scan";

const KIND_LABELS: Record<ScanErrorKind, string> = {
  permission_denied: "Permission denied",
  not_found: "Not found",
  io_error: "I/O error",
};

const KIND_COLORS: Record<ScanErrorKind, string> = {
  permission_denied: "#E1BC79",
  not_found: "#B5BDB7",
  io_error: "#F1A497",
};

/** Human label for a `ScanErrorRecord.kind`, falling back to the raw value. */
export function describeErrorKind(kind: ScanErrorKind): string {
  return KIND_LABELS[kind] ?? kind;
}

export interface ErrorDrawerProps {
  /** Unreadable paths reported by the scan; the drawer hides when empty. */
  errors: ScanErrorRecord[];
  /** Whether the drawer body is open. */
  expanded?: boolean;
  onToggle?: () => void;
}

export default function ErrorDrawer({ errors, expanded = false, onToggle }: ErrorDrawerProps) {
  if (errors.length === 0) {
    return null;
  }

  const countLabel = `${errors.length} unreadable ${errors.length === 1 ? "path" : "paths"}`;

  return (
    <section
      aria-label="Scan errors"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "0.5rem",
        border: "1px solid #3C4741",
        borderRadius: "0.375rem",
        background: "#232927",
        padding: "0.5rem 0.75rem",
        fontSize: "0.8125rem",
        fontVariantNumeric: "tabular-nums",
        maxHeight: "180px",
        overflowY: "auto",
      }}
    >
      <button
        type="button"
        aria-expanded={expanded}
        onClick={onToggle}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "0.5rem",
          padding: 0,
          border: "none",
          background: "transparent",
          font: "inherit",
          color: "#E1BC79",
          cursor: "pointer",
          textAlign: "left",
        }}
      >
        <span
          data-testid="error-count"
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            minWidth: "1.5rem",
            padding: "0 0.375rem",
            borderRadius: "999px",
            background: "#E1BC79",
            color: "#181C1B",
            fontWeight: 700,
            fontSize: "0.75rem",
          }}
        >
          {errors.length}
        </span>
        <span>{countLabel}</span>
        <span aria-hidden="true">{expanded ? "▾" : "▸"}</span>
      </button>

      {expanded ? (
        <ul
          data-testid="error-list"
          style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: "0.375rem" }}
        >
          {errors.map((error, index) => (
            <li
              key={`${index}:${error.path}`}
              data-testid="scan-error-row"
              data-kind={error.kind}
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "0.125rem",
                borderTop: index === 0 ? "none" : "1px solid #3C4741",
                paddingTop: index === 0 ? 0 : "0.375rem",
              }}
            >
              <code style={{ fontFamily: "ui-monospace, SFMono-Regular, monospace", wordBreak: "break-all" }}>
                {error.path}
              </code>
              <span style={{ color: KIND_COLORS[error.kind] ?? "#475569", fontSize: "0.75rem" }}>
                {describeErrorKind(error.kind)}
              </span>
              <span style={{ color: "#B5BDB7", fontSize: "0.75rem" }}>{error.message}</span>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
