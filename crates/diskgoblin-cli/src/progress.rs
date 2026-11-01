use diskgoblin_core::{format_bytes, ProgressSink, ScanProgress, UnitSystem};
use std::io::{self, IsTerminal, Write};

pub struct StderrProgressSink {
    enabled: bool,
    units: UnitSystem,
}

impl StderrProgressSink {
    pub fn new(quiet: bool, units: UnitSystem) -> Self {
        Self {
            enabled: !quiet && io::stderr().is_terminal(),
            units,
        }
    }

    pub fn finish(&self) {
        if self.enabled {
            let mut stderr = io::stderr().lock();
            let _ = write!(stderr, "\r\x1b[2K");
            let _ = stderr.flush();
        }
    }
}

impl ProgressSink for StderrProgressSink {
    fn on_progress(&self, progress: ScanProgress) {
        if self.enabled {
            let mut stderr = io::stderr().lock();
            let _ = write!(
                stderr,
                "\r\x1b[2KScanning... {} files | {} dirs | {} symlinks | {} errors | {} ({:.2}s)",
                progress.files_seen,
                progress.dirs_seen,
                progress.symlinks_seen,
                progress.errors_seen,
                format_bytes(progress.bytes_accumulated, self.units),
                progress.elapsed_ms as f64 / 1000.0
            );
            let _ = stderr.flush();
        }
    }
}

impl Drop for StderrProgressSink {
    fn drop(&mut self) {
        self.finish();
    }
}
