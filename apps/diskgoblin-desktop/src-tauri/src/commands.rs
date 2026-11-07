//! Display-independent command logic and optional Tauri IPC adapters.
use crate::state::{ActiveScanState, ScanRegistration};
use diskgoblin_core::{
    scan_directory_with_progress, CancelToken, FnProgressSink, ScanOptions, ScanProgress,
    ScanReport, SizeMetric,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};

pub const PROGRESS_EVENT: &str = "scan:progress";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOptionsDto {
    pub root_path: PathBuf,
    pub size_metric: SizeMetric,
    pub one_file_system: bool,
    pub max_depth: Option<usize>,
    pub top_n: usize,
    pub min_size_bytes: u64,
    pub threads: usize,
}

impl From<ScanOptionsDto> for ScanOptions {
    fn from(dto: ScanOptionsDto) -> Self {
        Self {
            root_path: dto.root_path,
            size_metric: dto.size_metric,
            one_file_system: dto.one_file_system,
            max_depth: dto.max_depth,
            top_n: dto.top_n,
            min_size_bytes: dto.min_size_bytes,
            threads: dto.threads,
        }
    }
}

/// Implementations must return promptly; the core throttles updates to 50 ms
/// and always emits the final successful snapshot.
pub trait ProgressEmitter: Send + Sync {
    fn emit(&self, event: &str, progress: ScanProgress) -> Result<(), String>;
}

pub fn start_scan(
    state: &ActiveScanState,
    options: ScanOptionsDto,
    emitter: &dyn ProgressEmitter,
) -> Result<ScanReport, String> {
    let token = CancelToken::new();
    let _registration = ScanRegistration(state, state.register(token.clone()));
    std::thread::scope(|scope| {
        scope
            .spawn(move || run_scan(options, token, emitter))
            .join()
            .map_err(|_| "scan worker panicked".to_string())?
    })
}

fn run_scan(
    options: ScanOptionsDto,
    token: CancelToken,
    emitter: &dyn ProgressEmitter,
) -> Result<ScanReport, String> {
    let emission_error = Mutex::new(None);
    let sink = FnProgressSink::new(|progress| {
        if let Err(error) = emitter.emit(PROGRESS_EVENT, progress) {
            let mut failure = emission_error.lock().unwrap_or_else(|e| e.into_inner());
            if failure.is_none() {
                *failure = Some(error);
            }
            token.cancel();
        }
    });
    let result = scan_directory_with_progress(&options.into(), &token, &sink);
    if let Some(error) = emission_error
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
    {
        return Err(format!("failed to emit scan progress: {error}"));
    }
    result.map_err(|error| error.to_string())
}

pub fn cancel_scan(state: &ActiveScanState) -> Result<(), String> {
    state.cancel();
    Ok(())
}

/// Inject the native picker so cancellation and path handling remain headless.
pub fn pick_directory(
    picker: impl FnOnce() -> Result<Option<PathBuf>, String>,
) -> Result<Option<PathBuf>, String> {
    picker()
}

/// Validate without following symlinks before invoking the system file manager.
pub fn reveal_path_with(
    path: PathBuf,
    opener: impl FnOnce(&std::path::Path) -> Result<(), String>,
) -> Result<(), String> {
    if path.as_os_str().is_empty() || std::fs::symlink_metadata(&path).is_err() {
        return Err("path does not exist or is inaccessible".into());
    }
    opener(&path)
}

#[cfg(feature = "tauri-runtime")]
pub mod runtime {
    use super::*;
    use std::sync::Arc;
    use tauri::{AppHandle, Emitter, State};
    use tauri_plugin_dialog::DialogExt;

    #[tauri::command]
    pub async fn reveal_path(path: PathBuf) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || {
            super::reveal_path_with(path, |path| {
                use std::process::Command;
                let status = if cfg!(target_os = "macos") {
                    Command::new("open").arg("-R").arg(path).status()
                } else if cfg!(target_os = "windows") {
                    Command::new("explorer")
                        .arg(format!("/select,{}", path.display()))
                        .status()
                } else {
                    Command::new("xdg-open")
                        .arg(
                            path.parent()
                                .filter(|p| !p.as_os_str().is_empty())
                                .unwrap_or(std::path::Path::new(".")),
                        )
                        .status()
                }
                .map_err(|e| e.to_string())?;
                if status.success() {
                    Ok(())
                } else {
                    Err(format!("reveal failed: {status}"))
                }
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    impl ProgressEmitter for AppHandle {
        fn emit(&self, event: &str, progress: ScanProgress) -> Result<(), String> {
            Emitter::emit(self, event, progress).map_err(|error| error.to_string())
        }
    }

    #[tauri::command]
    pub async fn start_scan(
        app: AppHandle,
        state: State<'_, Arc<ActiveScanState>>,
        options: ScanOptionsDto,
    ) -> Result<ScanReport, String> {
        let state = Arc::clone(state.inner());
        let token = CancelToken::new();
        // Register before queueing work so Cancel also reaches a queued scan.
        let generation = state.register(token.clone());
        tauri::async_runtime::spawn_blocking(move || {
            let _registration = ScanRegistration(&state, generation);
            run_scan(options, token, &app)
        })
        .await
        .map_err(|error| format!("scan worker failed: {error}"))?
    }

    #[tauri::command]
    pub fn cancel_scan(state: State<'_, Arc<ActiveScanState>>) -> Result<(), String> {
        super::cancel_scan(state.inner())
    }

    #[tauri::command]
    pub async fn pick_directory(app: AppHandle) -> Result<Option<PathBuf>, String> {
        tauri::async_runtime::spawn_blocking(move || {
            super::pick_directory(|| {
                app.dialog()
                    .file()
                    .blocking_pick_folder()
                    .map(|path| path.into_path().map_err(|error| error.to_string()))
                    .transpose()
            })
        })
        .await
        .map_err(|error| format!("directory picker failed: {error}"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diskgoblin_core::{test_fixtures::FixtureBuilder, REPORT_SCHEMA_VERSION};
    use std::{sync::mpsc, thread, time::Duration};

    #[test]
    fn reveal_validates_before_opening_and_preserves_errors() {
        let fixture = tempfile::tempdir().unwrap();
        assert!(reveal_path_with(fixture.path().into(), |p| {
            assert_eq!(p, fixture.path());
            Ok(())
        })
        .is_ok());
        assert_eq!(
            reveal_path_with(fixture.path().into(), |_| Err("opener failed".into())),
            Err("opener failed".into())
        );
        assert!(reveal_path_with(fixture.path().join("missing"), |_| panic!(
            "must not open missing path"
        ))
        .is_err());
        assert!(reveal_path_with(PathBuf::new(), |_| panic!("must not open empty path")).is_err());
    }

    struct TestEmitter<F>(F);
    impl<F: Fn(&str, ScanProgress) -> Result<(), String> + Send + Sync> ProgressEmitter
        for TestEmitter<F>
    {
        fn emit(&self, event: &str, progress: ScanProgress) -> Result<(), String> {
            (self.0)(event, progress)
        }
    }

    fn options(path: PathBuf) -> ScanOptionsDto {
        ScanOptionsDto {
            root_path: path,
            size_metric: SizeMetric::Apparent,
            one_file_system: false,
            max_depth: None,
            top_n: 20,
            min_size_bytes: 0,
            threads: 1,
        }
    }

    #[test]
    fn dto_preserves_every_option_and_json_names() {
        let dto: ScanOptionsDto = serde_json::from_value(serde_json::json!({
            "root_path": "synthetic", "size_metric": "allocated",
            "one_file_system": true, "max_depth": 3, "top_n": 7,
            "min_size_bytes": 123, "threads": 4
        }))
        .unwrap();
        assert_eq!(
            ScanOptions::from(dto),
            ScanOptions {
                root_path: "synthetic".into(),
                size_metric: SizeMetric::Allocated,
                one_file_system: true,
                max_depth: Some(3),
                top_n: 7,
                min_size_bytes: 123,
                threads: 4,
            }
        );
    }

    #[test]
    fn scan_runs_on_worker_and_emits_serializable_final_report() {
        let fixture = FixtureBuilder::new()
            .add_file("a", 11)
            .add_file("nested/b", 17)
            .build()
            .unwrap();
        let state = ActiveScanState::default();
        let caller = thread::current().id();
        let updates = Mutex::new(Vec::new());
        let emitter = TestEmitter(|event: &str, progress: ScanProgress| {
            assert_ne!(thread::current().id(), caller);
            assert_eq!(event, PROGRESS_EVENT);
            updates.lock().unwrap().push(progress);
            Ok(())
        });
        for threads in [1, 2] {
            let mut dto = options(fixture.path().into());
            dto.threads = threads;
            let report = start_scan(&state, dto, &emitter).unwrap();
            assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
            assert_eq!(report.root.summary.file_count, 2);
            assert_eq!(report.root.summary.apparent_size, 28);
            assert_eq!(report.root.summary.dir_count, 1);
            assert!(report.errors.is_empty());
            let last = *updates.lock().unwrap().last().unwrap();
            assert_eq!(last.files_seen, 2);
            assert_eq!(last.bytes_accumulated, 28);
            assert_eq!(last.elapsed_ms, report.duration_ms);
            let json = serde_json::to_string(&report).unwrap();
            assert_eq!(serde_json::from_str::<ScanReport>(&json).unwrap(), report);
        }
    }

    #[test]
    fn cancel_scan_reaches_in_flight_worker_without_timing_assumptions() {
        let fixture = FixtureBuilder::new()
            .add_file("nested/a", 5)
            .build()
            .unwrap();
        let state = ActiveScanState::default();
        let (started_tx, started_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let resume_rx = Mutex::new(resume_rx);
        let emitter = TestEmitter(|_: &str, _: ScanProgress| {
            started_tx.send(()).unwrap();
            resume_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            Ok(())
        });
        thread::scope(|scope| {
            let worker =
                scope.spawn(|| start_scan(&state, options(fixture.path().into()), &emitter));
            started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            cancel_scan(&state).unwrap();
            resume_tx.send(()).unwrap();
            assert_eq!(
                worker.join().unwrap().unwrap_err(),
                "scan cancelled by user"
            );
        });
        // Cancellation does not leak into the next scan.
        let emitter = TestEmitter(|_: &str, _: ScanProgress| Ok(()));
        assert!(start_scan(&state, options(fixture.path().into()), &emitter).is_ok());
    }

    #[test]
    fn missing_root_and_emitter_failure_are_returned_and_state_is_reusable() {
        let fixture = tempfile::tempdir().unwrap();
        let state = ActiveScanState::default();
        let emitter = TestEmitter(|_: &str, _: ScanProgress| Ok(()));
        let error =
            start_scan(&state, options(fixture.path().join("missing")), &emitter).unwrap_err();
        assert!(error.contains("root path does not exist"));
        let failing = TestEmitter(|_: &str, _: ScanProgress| Err("disconnected".to_string()));
        assert_eq!(
            start_scan(&state, options(fixture.path().into()), &failing).unwrap_err(),
            "failed to emit scan progress: disconnected"
        );
        assert!(start_scan(&state, options(fixture.path().into()), &emitter).is_ok());
    }

    #[test]
    fn worker_panic_is_a_command_error_and_next_scan_succeeds() {
        let fixture = tempfile::tempdir().unwrap();
        let state = ActiveScanState::default();
        let emitter = TestEmitter(|_: &str, _: ScanProgress| panic!("synthetic emitter panic"));
        assert_eq!(
            start_scan(&state, options(fixture.path().into()), &emitter).unwrap_err(),
            "scan worker panicked"
        );
        let emitter = TestEmitter(|_: &str, _: ScanProgress| Ok(()));
        assert!(start_scan(&state, options(fixture.path().into()), &emitter).is_ok());
    }

    #[test]
    fn picker_preserves_selected_path_dismissal_and_errors() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().to_path_buf();
        assert_eq!(
            pick_directory(|| Ok(Some(path.clone()))).unwrap(),
            Some(path)
        );
        assert_eq!(pick_directory(|| Ok(None)).unwrap(), None);
        assert_eq!(
            pick_directory(|| Err("unavailable".into())).unwrap_err(),
            "unavailable"
        );
    }
}
