import { useEffect, useState } from "react";

import BreadcrumbBar from "./components/BreadcrumbBar";
import ErrorDrawer from "./components/ErrorDrawer";
import ScanToolbar from "./components/ScanToolbar";
import TopItemsPanel from "./components/TopItemsPanel";
import type { TopItemsTab } from "./components/TopItemsPanel";
import TreemapView from "./components/TreemapView";
import {
  MOCK_SCAN_ROOT,
  cancelScan,
  isTauriRuntime,
  pickDirectory,
  startScan,
  subscribeScanProgress,
} from "./lib/ipc";
import { findAncestorChain } from "./lib/treemapLayout";
import type { DirNode, ScanOptionsDto, ScanProgress, ScanReport, SizeMetric } from "./types/scan";

/** Backend defaults mirrored from `diskgoblin-core::ScanOptions::default`. */
const DEFAULT_TOP_N = 20;
/** `0` lets the core size its worker pool from the host's parallelism. */
const DEFAULT_THREADS = 0;

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return typeof error === "string" ? error : "scan failed";
}

export default function App() {
  // Preview and tests start on the synthetic fixture root; a real Tauri window
  // waits for the user to type a path or pick one.
  const [path, setPath] = useState(() => (isTauriRuntime() ? "" : MOCK_SCAN_ROOT));
  const [metric, setMetric] = useState<SizeMetric>("allocated");
  const [report, setReport] = useState<ScanReport | null>(null);
  const [scanning, setScanning] = useState(false);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [topTab, setTopTab] = useState<TopItemsTab>("directories");
  const [errorsExpanded, setErrorsExpanded] = useState(false);
  // Ancestor chain from the scan root to the zoomed directory; empty = root view.
  const [zoomChain, setZoomChain] = useState<DirNode[]>([]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    subscribeScanProgress((next) => {
      if (!disposed) {
        setProgress(next);
      }
    })
      .then((stop) => {
        if (disposed) {
          stop();
        } else {
          unlisten = stop;
        }
      })
      .catch(() => {
        // A missing event bridge must not break the UI; scans still resolve.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

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

  const handleScan = async () => {
    const rootPath = path.trim();
    if (scanning || rootPath.length === 0) {
      return;
    }
    const options: ScanOptionsDto = {
      root_path: rootPath,
      size_metric: metric,
      one_file_system: false,
      max_depth: null,
      top_n: DEFAULT_TOP_N,
      min_size_bytes: 0,
      threads: DEFAULT_THREADS,
    };
    setScanning(true);
    setNotice(null);
    setProgress(null);
    try {
      const result = await startScan(options);
      setReport(result);
      setMetric(result.size_metric);
      setZoomChain([]);
      setTopTab("directories");
      setErrorsExpanded(false);
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setScanning(false);
    }
  };

  const handleCancel = async () => {
    try {
      await cancelScan();
    } catch (error) {
      setNotice(errorMessage(error));
    }
  };

  const handlePickDirectory = async () => {
    try {
      const picked = await pickDirectory();
      if (picked) {
        setPath(picked);
        setNotice(null);
      }
    } catch (error) {
      setNotice(errorMessage(error));
    }
  };

  /** Focuses a table row's directory in the treemap when it is in the tree. */
  const handleSelectDirectory = (targetPath: string) => {
    if (!report) {
      return;
    }
    const chain = findAncestorChain(report.root, targetPath);
    const node = chain[chain.length - 1];
    if (!node || node.summary.path !== targetPath) {
      setNotice(`${targetPath} is not part of the scanned tree.`);
      return;
    }
    zoomTo(node);
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

      <ScanToolbar
        path={path}
        onPathChange={setPath}
        metric={metric}
        onMetricChange={setMetric}
        scanning={scanning}
        progress={progress}
        onScan={() => {
          void handleScan();
        }}
        onCancel={() => {
          void handleCancel();
        }}
        onPickDirectory={() => {
          void handlePickDirectory();
        }}
      />

      {notice ? (
        <p role="status" style={{ margin: 0, color: "#b91c1c", fontSize: "0.8125rem" }}>
          {notice}
        </p>
      ) : null}

      {report ? (
        <BreadcrumbBar ancestors={ancestors} metric={metric} onNavigate={zoomTo} />
      ) : null}

      <TreemapView rootNode={zoomRoot} metric={metric} onZoom={zoomTo} />

      <TopItemsPanel
        report={report}
        metric={metric}
        tab={topTab}
        onTabChange={setTopTab}
        onSelectDirectory={handleSelectDirectory}
      />

      <ErrorDrawer
        errors={report ? report.errors : []}
        expanded={errorsExpanded}
        onToggle={() => setErrorsExpanded((open) => !open)}
      />
    </main>
  );
}
