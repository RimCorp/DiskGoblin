use diskgoblin_core::{
    scan_directory_with_progress, test_fixtures::FixtureBuilder, CancelToken, FatalScanError,
    FnProgressSink, ScanOptions, ScanProgress, SizeMetric,
};
use std::{sync::Mutex, time::Duration};

#[test]
fn cancel_from_callback_after_flat_directory_progress() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = FixtureBuilder::new();
    for index in 0..1024 {
        builder = builder.add_file(format!("file-{index:04}"), 1);
    }
    let fixture = builder.build()?;
    let token = CancelToken::new();
    let updates = Mutex::new(Vec::new());
    let sink = FnProgressSink::new(|progress: ScanProgress| {
        updates.lock().unwrap().push(progress);
        if progress.files_seen == 0 {
            // Allow the next sampled snapshot through the real 50 ms throttle.
            std::thread::sleep(Duration::from_millis(50));
        } else {
            token.cancel();
        }
    });
    let options = ScanOptions {
        root_path: fixture.path().into(),
        size_metric: SizeMetric::Apparent,
        ..ScanOptions::default()
    };
    assert!(matches!(
        scan_directory_with_progress(&options, &token, &sink),
        Err(FatalScanError::Cancelled)
    ));
    let updates = updates.lock().unwrap();
    assert_eq!(updates.len(), 2);
    let last = updates.last().unwrap();
    assert_eq!(last.files_seen, 255);
    assert_eq!(last.bytes_accumulated, 255);
    Ok(())
}

#[test]
fn cancel_empty_scan_flushes_matching_final_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = FixtureBuilder::new().build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        size_metric: SizeMetric::Apparent,
        ..ScanOptions::default()
    };
    let updates = Mutex::new(Vec::new());
    let sink = FnProgressSink::new(|progress| updates.lock().unwrap().push(progress));
    let report = scan_directory_with_progress(&options, &CancelToken::new(), &sink)?;
    let updates = updates.lock().unwrap();
    assert!(updates.len() >= 2, "completion bypasses the throttle");
    assert_eq!(
        *updates.last().unwrap(),
        ScanProgress {
            elapsed_ms: report.duration_ms,
            ..ScanProgress::default()
        }
    );
    Ok(())
}
