//! Cooperative cancellation and structured, rate-limited scan progress.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// Clones share the same cancellation flag. Cancellation is permanent.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Cumulative scan totals using the selected size metric and hard-link policy.
/// Directory counts exclude the root, matching `ScanReport::root.summary`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ScanProgress {
    pub files_seen: u64,
    pub dirs_seen: u64,
    pub symlinks_seen: u64,
    pub errors_seen: u64,
    pub bytes_accumulated: u64,
    pub elapsed_ms: u64,
}

/// Receives synchronous progress callbacks; implementations should return promptly.
pub trait ProgressSink: Send + Sync {
    fn on_progress(&self, progress: ScanProgress);
}

impl<T: ProgressSink + ?Sized> ProgressSink for &T {
    fn on_progress(&self, progress: ScanProgress) {
        (**self).on_progress(progress);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopProgressSink;

impl ProgressSink for NoopProgressSink {
    fn on_progress(&self, _progress: ScanProgress) {}
}

/// Adapts a thread-safe closure to a progress sink.
pub struct FnProgressSink<F>(pub F);

impl<F> FnProgressSink<F> {
    pub fn new(callback: F) -> Self {
        Self(callback)
    }
}

impl<F: Fn(ScanProgress) + Send + Sync> ProgressSink for FnProgressSink<F> {
    fn on_progress(&self, progress: ScanProgress) {
        (self.0)(progress);
    }
}

/// Forwards the first update immediately, then at most once per interval.
/// Call `flush` to deliver a final snapshot regardless of the interval.
pub struct ThrottledProgressSink<S: ProgressSink> {
    sink: S,
    min_interval: Duration,
    last_forwarded: Mutex<Option<Instant>>,
}

impl<S: ProgressSink> ThrottledProgressSink<S> {
    /// Use the default 50 ms interval (20 Hz).
    pub fn new(sink: S) -> Self {
        Self::with_interval(sink, Duration::from_millis(50))
    }

    pub fn with_interval(sink: S, min_interval: Duration) -> Self {
        Self {
            sink,
            min_interval,
            last_forwarded: Mutex::new(None),
        }
    }

    /// Deliver a completion snapshot even if the preceding update was recent.
    pub fn flush(&self, progress: ScanProgress) {
        self.forward_at(progress, Instant::now(), true);
    }

    fn forward_at(&self, progress: ScanProgress, now: Instant, force: bool) {
        let mut last = match self.last_forwarded.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !force
            && last
                .is_some_and(|previous| now.saturating_duration_since(previous) < self.min_interval)
        {
            return;
        }
        *last = Some(now);
        // Never hold the bookkeeping lock while calling user code.
        drop(last);
        self.sink.on_progress(progress);
    }
}

impl<S: ProgressSink> ProgressSink for ThrottledProgressSink<S> {
    fn on_progress(&self, progress: ScanProgress) {
        self.forward_at(progress, Instant::now(), false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        scan_directory_with_progress, test_fixtures::FixtureBuilder, FatalScanError, ScanOptions,
        SizeMetric,
    };

    #[test]
    fn cancel_token_clones_share_cancellation_across_threads() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        let clone = token.clone();
        std::thread::spawn(move || clone.cancel()).join().unwrap();
        assert!(token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn cancel_throttle_interval_and_flush() {
        let updates = Mutex::new(Vec::new());
        let sink =
            ThrottledProgressSink::new(FnProgressSink::new(|p| updates.lock().unwrap().push(p)));
        let start = Instant::now();
        let first = ScanProgress::default();
        let final_update = ScanProgress {
            files_seen: 42,
            ..first
        };
        sink.forward_at(first, start, false);
        sink.forward_at(first, start + Duration::from_millis(49), false);
        assert_eq!(updates.lock().unwrap().len(), 1);
        sink.forward_at(first, start + Duration::from_millis(50), false);
        assert_eq!(updates.lock().unwrap().len(), 2);
        sink.flush(final_update);
        assert_eq!(updates.lock().unwrap().last(), Some(&final_update));
        let sink = ThrottledProgressSink::with_interval(&NoopProgressSink, Duration::ZERO);
        sink.on_progress(first);
        sink.on_progress(first);
    }

    #[test]
    fn cancel_pre_cancelled_precedes_root_validation() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        let token = CancelToken::new();
        token.cancel();
        let options = ScanOptions {
            root_path: fixture.path().join("missing"),
            ..ScanOptions::default()
        };
        let sink = FnProgressSink::new(|_| panic!("pre-cancelled scan emitted progress"));
        assert!(matches!(
            scan_directory_with_progress(&options, &token, &sink),
            Err(FatalScanError::Cancelled)
        ));
        Ok(())
    }

    #[test]
    fn cancel_from_progress_stops_before_reading_directory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = FixtureBuilder::new().add_file("child/file", 7).build()?;
        let token = CancelToken::new();
        let options = ScanOptions {
            root_path: fixture.path().into(),
            ..ScanOptions::default()
        };
        let sink = FnProgressSink::new(|p: ScanProgress| {
            assert_eq!(p.files_seen, 0);
            token.cancel();
        });
        assert!(matches!(
            scan_directory_with_progress(&options, &token, &sink),
            Err(FatalScanError::Cancelled)
        ));
        Ok(())
    }

    #[test]
    fn cancel_progress_final_snapshot_matches_report() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = FixtureBuilder::new()
            .add_file("a", 11)
            .add_file("child/b", 17)
            .add_hard_link("a", "child/link")
            .build()?;
        for size_metric in [SizeMetric::Apparent, SizeMetric::Allocated] {
            let options = ScanOptions {
                root_path: fixture.path().into(),
                size_metric,
                ..ScanOptions::default()
            };
            let updates = Mutex::new(Vec::new());
            let sink = FnProgressSink::new(|p| updates.lock().unwrap().push(p));
            let report = scan_directory_with_progress(&options, &CancelToken::new(), &sink)?;
            let updates = updates.lock().unwrap();
            let last = updates.last().unwrap();
            assert_eq!(last.files_seen, report.root.summary.file_count);
            assert_eq!(last.dirs_seen, report.root.summary.dir_count);
            assert_eq!(last.symlinks_seen, report.root.summary.symlink_count);
            assert_eq!(last.errors_seen, report.root.summary.error_count);
            assert_eq!(
                last.bytes_accumulated,
                size_metric.select(
                    report.root.summary.apparent_size,
                    report.root.summary.allocated_size
                )
            );
            assert_eq!(last.elapsed_ms, report.duration_ms);
        }
        Ok(())
    }
}
