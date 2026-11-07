import { useEffect, useRef, useState } from "react";

import BreadcrumbBar from "./components/BreadcrumbBar";
import ErrorDrawer from "./components/ErrorDrawer";
import ScanToolbar, { ItemIcon } from "./components/ScanToolbar";
import TopItemsPanel from "./components/TopItemsPanel";
import type { TopItemsTab } from "./components/TopItemsPanel";
import TreemapView from "./components/TreemapView";
import {
  MOCK_SCAN_ROOT,
  cancelScan,
  fetchUiBootstrapConfig,
  isTauriRuntime,
  pickDirectory,
  revealPath,
  startScan,
  subscribeScanProgress,
} from "./lib/ipc";
import { findAncestorChain, formatBytes } from "./lib/treemapLayout";
import type { ImmediateChildItem } from "./lib/treemapLayout";
import "./style.css";
import type { DirNode, ScanOptionsDto, ScanProgress, ScanReport, SizeMetric } from "./types/scan";

/** Backend defaults mirrored from `diskgoblin-core::ScanOptions::default`. */
const DEFAULT_TOP_N = 500;
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
  const [selectedItem, setSelectedItem] = useState<ImmediateChildItem | null>(null);
  const [leftWidth, setLeftWidth] = useState(65);
  const workspace = useRef<HTMLDivElement>(null);
  const generation = useRef(0);
  const scanActive = useRef(false);
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

  useEffect(() => {
    let disposed = false;
    void fetchUiBootstrapConfig().then(async (config) => {
      if (disposed || !config) return;
      setPath(config.root_path);
      setMetric(config.size_metric);
      if (config.auto_scan === false || config.root_path.trim().length === 0) return;
      if (scanActive.current) return;
      scanActive.current = true;
      const currentGeneration = ++generation.current;
      setScanning(true);
      setNotice(null);
      setProgress(null);
      try {
        const result = await startScan({
          root_path: config.root_path,
          size_metric: config.size_metric,
          one_file_system: config.one_file_system ?? false,
          max_depth: config.max_depth ?? null,
          top_n: Math.max(config.top_n ?? 0, DEFAULT_TOP_N),
          min_size_bytes: config.min_size_bytes ?? 0,
          threads: config.threads ?? DEFAULT_THREADS,
        });
        if (!disposed && generation.current === currentGeneration) { setReport(result); setZoomChain([]); }
      } catch (error) {
        if (!disposed) setNotice(errorMessage(error));
      } finally {
        scanActive.current = false;
        if (!disposed) setScanning(false);
      }
    });
    return () => { disposed = true; };
  }, []);

  const ancestors = zoomChain.length > 0 ? zoomChain : report ? [report.root] : [];
  const zoomRoot = ancestors.length > 0 ? ancestors[ancestors.length - 1] : null;

  const zoomTo = (node: DirNode) => {
    setSelectedItem(null);
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
    if (scanActive.current || rootPath.length === 0) {
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
    scanActive.current = true;
    const currentGeneration = ++generation.current;
    setReport(null);
    setSelectedItem(null);
    setZoomChain([]);
    setScanning(true);
    setNotice(null);
    setProgress(null);
    try {
      const result = await startScan(options);
      if (currentGeneration !== generation.current) return;
      setReport(result);
      setMetric(result.size_metric);
      setZoomChain([]);
      setTopTab("directories");
      setErrorsExpanded(false);
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      scanActive.current = false;
      setScanning(false);
    }
  };

  const handleCancel = async () => {
    try {
      ++generation.current;
      await cancelScan();
      setNotice("scan cancelled by user");
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

  const handleReveal = async (selectedPath: string) => {
    try { await revealPath(selectedPath); setNotice(null); }
    catch (error) { setNotice(`Reveal failed: ${errorMessage(error)}`); }
  };
  const selectedBytes = selectedItem ? metric === "apparent" ? selectedItem.apparent_size : selectedItem.allocated_size : 0;
  return <main>
    <ScanToolbar path={path} onPathChange={setPath} metric={metric} onMetricChange={setMetric}
      scanning={scanning} progress={progress} report={report} onScan={() => void handleScan()}
      onCancel={() => void handleCancel()} onPickDirectory={() => void handlePickDirectory()} />
    {notice && <p role="status" className="notice">{notice}</p>}
    <div className="workspace" ref={workspace}>
      <section className="panel map-panel" style={{ flexBasis: `${leftWidth}%` }}>
        {report && <BreadcrumbBar ancestors={ancestors} metric={metric} onNavigate={zoomTo} />}
        <TreemapView rootNode={zoomRoot} metric={metric} onZoom={zoomTo} directFiles={report?.top_files}
          selectedId={selectedItem?.id} onSelect={rect => { if (rect.item) setSelectedItem(rect.item); }} />
      </section>
      <div role="separator" aria-orientation="vertical" aria-label="Resize workspace panels" tabIndex={0}
        aria-valuemin={40} aria-valuemax={80} aria-valuenow={Math.round(leftWidth)} className="divider"
        onKeyDown={event => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); setLeftWidth(value => Math.min(80, Math.max(40, value + (event.key === "ArrowLeft" ? -2 : 2)))); } }}
        onPointerDown={event => { event.currentTarget.setPointerCapture(event.pointerId); }}
        onPointerMove={event => { if (!event.currentTarget.hasPointerCapture(event.pointerId) || !workspace.current) return;
          const bounds = workspace.current.getBoundingClientRect();
          setLeftWidth(Math.min(80, Math.max(40, (event.clientX - bounds.left) / bounds.width * 100)));
        }} onPointerUp={event => event.currentTarget.releasePointerCapture(event.pointerId)} />
      <TopItemsPanel report={report} viewedFolder={zoomRoot} metric={metric} tab={topTab} onTabChange={setTopTab}
        selectedId={selectedItem?.id} onSelectItem={setSelectedItem} onOpenDirectory={zoomTo} />
    </div>
    <section className="panel inspector" aria-label="Selected item inspector">
      <ItemIcon file={selectedItem?.kind === "file"} />
      <div className="inspector-label">{selectedItem ? <><strong>{selectedItem.name}</strong><div className="selected-path" title={selectedItem.path}>{selectedItem.isAggregate ? "Aggregate of multiple items; no single file path." : selectedItem.path}</div></> : <span className="muted">No item selected — click a folder or file to inspect its path and size.</span>}</div>
      {selectedItem && <span className="inspector-size">{formatBytes(selectedBytes)}</span>}
      <button disabled={!selectedItem || selectedItem.isAggregate} onClick={() => { if (selectedItem) void handleReveal(selectedItem.path); }}>Reveal in Finder</button>
    </section>
    <footer><span><span className="readonly-dot">●</span> Read-only · {scanning ? "Scanning…" : notice ? notice.includes("cancelled") ? "Scan cancelled." : "See notice above." : report ? "Scan complete." : "Ready to scan."}</span>
      <ErrorDrawer errors={report?.errors ?? []} expanded={errorsExpanded} onToggle={() => setErrorsExpanded(open => !open)} />
    </footer>
  </main>;
}
