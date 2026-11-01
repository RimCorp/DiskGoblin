use diskgoblin_core::{format_bytes, format_percentage, render_ascii_bar, ScanReport, UnitSystem};
use std::io::{self, Write};

pub struct RenderOptions {
    pub units: UnitSystem,
    pub show_errors: bool,
}

pub fn render_human_report<W: Write>(
    report: &ScanReport,
    opts: &RenderOptions,
    mut writer: W,
) -> io::Result<()> {
    let root = &report.root.summary;
    let total = report
        .size_metric
        .select(root.apparent_size, root.allocated_size);
    writeln!(writer, "Root: {}", report.root_path.display())?;
    writeln!(
        writer,
        "Allocated: {} | Apparent: {}",
        format_bytes(root.allocated_size, opts.units),
        format_bytes(root.apparent_size, opts.units)
    )?;
    writeln!(
        writer,
        "Files: {} | Dirs: {} | Symlinks: {} | Duration: {} ms | Errors: {}",
        root.file_count, root.dir_count, root.symlink_count, report.duration_ms, root.error_count
    )?;
    writeln!(writer, "\nTop Directories ({:?})", report.size_metric)?;
    writeln!(
        writer,
        "Rank         Size       %  Bar                   Error  Path"
    )?;
    for (index, dir) in report.top_directories.iter().enumerate() {
        let size = report
            .size_metric
            .select(dir.apparent_size, dir.allocated_size);
        writeln!(
            writer,
            "{:>4} {:>12} {:>7}  {}  {:5}  {}",
            index + 1,
            format_bytes(size, opts.units),
            format_percentage(size, total),
            render_ascii_bar(size, total, 20),
            if dir.error_count > 0 { "[!]" } else { "" },
            dir.path.display()
        )?;
    }
    writeln!(writer, "\nTop Files ({:?})", report.size_metric)?;
    writeln!(writer, "Rank         Size       %  Path")?;
    for (index, file) in report.top_files.iter().enumerate() {
        let size = report
            .size_metric
            .select(file.apparent_size, file.allocated_size);
        writeln!(
            writer,
            "{:>4} {:>12} {:>7}  {}",
            index + 1,
            format_bytes(size, opts.units),
            format_percentage(size, total),
            file.path.display()
        )?;
    }
    writeln!(
        writer,
        "\nErrors: {} inaccessible paths",
        report.errors.len()
    )?;
    if opts.show_errors {
        for error in &report.errors {
            writeln!(
                writer,
                "[!] {}: {:?}: {}",
                error.path.display(),
                error.kind,
                error.message
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use diskgoblin_core::{
        DirNode, DirSummary, FileEntry, ScanErrorKind, ScanErrorRecord, SizeMetric,
    };
    fn report() -> ScanReport {
        let root = DirSummary {
            path: "fixture".into(),
            depth: 0,
            apparent_size: 4000,
            allocated_size: 8192,
            file_count: 2,
            dir_count: 1,
            symlink_count: 1,
            error_count: 1,
        };
        ScanReport {
            schema_version: 1,
            root_path: root.path.clone(),
            size_metric: SizeMetric::Allocated,
            duration_ms: 12,
            root: DirNode {
                summary: root.clone(),
                children: vec![],
            },
            top_directories: vec![DirSummary {
                path: "fixture/sub".into(),
                apparent_size: 1000,
                allocated_size: 4096,
                ..root
            }],
            top_files: vec![FileEntry {
                path: "fixture/file".into(),
                apparent_size: 1000,
                allocated_size: 4096,
                is_deduplicated_hardlink: false,
            }],
            errors: vec![ScanErrorRecord {
                path: "fixture/secret".into(),
                kind: ScanErrorKind::PermissionDenied,
                message: "denied".into(),
            }],
        }
    }
    fn render(report: &ScanReport, units: UnitSystem, show_errors: bool) -> String {
        let mut out = vec![];
        render_human_report(report, &RenderOptions { units, show_errors }, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }
    #[test]
    fn summary_tables_and_error_visibility() {
        let text = render(&report(), UnitSystem::IecBinary, false);
        assert!(text.starts_with("Root: fixture\nAllocated: 8.0 KiB | Apparent: 3.9 KiB\nFiles: 2 | Dirs: 1 | Symlinks: 1 | Duration: 12 ms | Errors: 1\n"));
        assert!(text.contains("Top Directories (Allocated)"));
        assert!(
            text.contains("   1      4.0 KiB   50.0%  ██████████░░░░░░░░░░  [!]    fixture/sub\n")
        );
        assert!(text.contains("Top Files (Allocated)\nRank         Size       %  Path\n   1      4.0 KiB   50.0%  fixture/file\n"));
        assert!(text.ends_with("Errors: 1 inaccessible paths\n"));
        assert!(!text.contains("fixture/secret"));
        assert!(render(&report(), UnitSystem::IecBinary, true)
            .contains("[!] fixture/secret: PermissionDenied: denied\n"));
    }
    #[test]
    fn apparent_metric_and_si_units() {
        let mut report = report();
        report.size_metric = SizeMetric::Apparent;
        let text = render(&report, UnitSystem::SiDecimal, false);
        assert!(text.contains("Allocated: 8.2 KB | Apparent: 4.0 KB"));
        assert!(text.contains("1.0 KB   25.0%"));
    }
    #[test]
    fn zero_totals_and_empty_lists() {
        let mut report = report();
        report.root.summary.allocated_size = 0;
        assert!(render(&report, UnitSystem::IecBinary, false).contains("0.0%"));
        report.top_directories.clear();
        report.top_files.clear();
        report.errors.clear();
        report.root.summary.error_count = 0;
        assert!(render(&report, UnitSystem::IecBinary, true)
            .ends_with("Errors: 0 inaccessible paths\n"));
    }
    #[test]
    fn propagates_writer_errors() {
        assert!(render_human_report(
            &report(),
            &RenderOptions {
                units: UnitSystem::IecBinary,
                show_errors: false
            },
            io::Cursor::new(&mut [0u8; 0][..])
        )
        .is_err());
    }
}
