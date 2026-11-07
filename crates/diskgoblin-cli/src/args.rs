use clap::Parser;
use diskgoblin_core::{ScanOptions, SizeMetric};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "diskgoblin", version, about = "Read-only disk space analyzer")]
pub struct Args {
    /// Launch the interactive visual web UI backed by a local HTTP bridge
    #[arg(long)]
    pub ui: bool,
    /// Local UI server port (0 = automatic OS-assigned port)
    #[arg(long, default_value_t = 7878, value_name = "PORT")]
    pub ui_port: u16,
    /// Do not automatically open the browser
    #[arg(long)]
    pub no_open: bool,
    /// Root directory or file to scan
    #[arg(default_value = ".")]
    pub path: PathBuf,
    /// Use apparent (logical) byte size instead of physical disk allocation
    #[arg(long)]
    pub apparent_size: bool,
    /// Do not cross filesystem mount boundaries
    #[arg(short = 'x', long)]
    pub one_file_system: bool,
    /// Maximum directory depth to include in directory reports
    #[arg(short = 'd', long, default_value_t = 3, value_name = "DEPTH")]
    pub max_depth: usize,
    /// Number of largest directories and files to display
    #[arg(short = 'n', long, default_value_t = 20, value_name = "N")]
    pub top: usize,
    /// Omit entries smaller than BYTES from top lists
    #[arg(long, default_value_t = 0, value_name = "BYTES")]
    pub min_size: u64,
    /// Use decimal SI units instead of binary IEC units
    #[arg(long)]
    pub si: bool,
    /// Output machine-readable JSON to stdout
    #[arg(long)]
    pub json: bool,
    /// List individual inaccessible paths and error reasons in text output
    #[arg(long)]
    pub show_errors: bool,
    /// Suppress non-error progress output on stderr
    #[arg(short = 'q', long)]
    pub quiet: bool,
    /// Worker thread count (0 = automatic, 1 = sequential; M1 scans sequentially)
    #[arg(short = 'j', long, default_value_t = 1, value_name = "N")]
    pub threads: usize,
}

impl Args {
    pub fn scan_options(&self) -> ScanOptions {
        ScanOptions {
            root_path: self.path.clone(),
            size_metric: if self.apparent_size {
                SizeMetric::Apparent
            } else {
                SizeMetric::Allocated
            },
            one_file_system: self.one_file_system,
            max_depth: Some(self.max_depth),
            top_n: self.top,
            min_size_bytes: self.min_size,
            threads: self.threads,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults() {
        let args = Args::try_parse_from(["diskgoblin"]).unwrap();
        assert_eq!(
            args.scan_options(),
            ScanOptions {
                max_depth: Some(3),
                ..ScanOptions::default()
            }
        );
        assert!(!args.ui && !args.no_open);
        assert_eq!(args.ui_port, 7878);
        assert!(!args.si && !args.json && !args.show_errors && !args.quiet);
    }
    #[test]
    fn all_flags_and_mapping() {
        let args = Args::try_parse_from([
            "diskgoblin",
            "fixture",
            "--apparent-size",
            "-x",
            "-d",
            "0",
            "-n",
            "5",
            "--min-size",
            "1024",
            "--si",
            "--json",
            "--show-errors",
            "-q",
            "-j",
            "0",
        ])
        .unwrap();
        assert_eq!(
            args.scan_options(),
            ScanOptions {
                root_path: "fixture".into(),
                size_metric: SizeMetric::Apparent,
                one_file_system: true,
                max_depth: Some(0),
                top_n: 5,
                min_size_bytes: 1024,
                threads: 0
            }
        );
        assert!(args.si && args.json && args.show_errors && args.quiet);
    }
    #[test]
    fn rejects_invalid_arguments() {
        for flags in [
            vec!["--unknown"],
            vec!["--top", "-1"],
            vec!["--max-depth", "abc"],
            vec!["--min-size", "18446744073709551616"],
            vec!["--threads"],
        ] {
            assert_eq!(
                Args::try_parse_from(std::iter::once("diskgoblin").chain(flags))
                    .unwrap_err()
                    .exit_code(),
                2
            );
        }
    }
    #[test]
    fn ui_flags() {
        let args = Args::try_parse_from([
            "diskgoblin",
            "--ui",
            "--ui-port",
            "0",
            "--no-open",
            "fixture",
        ])
        .unwrap();
        assert!(args.ui && args.no_open);
        assert_eq!(args.ui_port, 0);
        assert_eq!(args.path, PathBuf::from("fixture"));
        assert!(Args::try_parse_from(["diskgoblin", "--ui-port", "65536"]).is_err());
    }
    #[test]
    fn help_and_version() {
        for flag in ["--help", "--version"] {
            assert_eq!(
                Args::try_parse_from(["diskgoblin", flag])
                    .unwrap_err()
                    .exit_code(),
                0
            );
        }
    }
}
