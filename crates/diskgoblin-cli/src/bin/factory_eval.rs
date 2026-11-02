//! `factory_eval` — validate recorded software-factory telemetry and render the
//! Milestone `M4` experiment summary.
//!
//! Usage:
//!
//! ```text
//! cargo run -p diskgoblin-cli --bin factory_eval -- docs/experiments/runs/
//! ```
//!
//! With no argument the tool reads `docs/experiments/runs` relative to the
//! current directory. It prints a GitHub-Flavored Markdown report to `stdout` and
//! exits `0`; any unreadable file, malformed record, or failed validation is
//! reported on `stderr` and exits `1`.

use diskgoblin_core::{load_run_records_from_dir, render_experiment_summary_markdown};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Directory read when no path argument is supplied.
const DEFAULT_RUN_DIR: &str = "docs/experiments/runs";

/// Loads and validates every run record in `dir`, then renders the report.
fn build_report(dir: &Path) -> Result<String, String> {
    let records = load_run_records_from_dir(dir)?;
    Ok(render_experiment_summary_markdown(&records))
}

fn main() -> ExitCode {
    let dir = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from(DEFAULT_RUN_DIR), PathBuf::from);

    let markdown = match build_report(&dir) {
        Ok(markdown) => markdown,
        Err(error) => {
            eprintln!("factory_eval: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut stdout = std::io::stdout().lock();
    if let Err(error) = stdout.write_all(markdown.as_bytes()) {
        eprintln!("factory_eval: failed to write report to stdout: {error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = stdout.flush() {
        eprintln!("factory_eval: failed to flush stdout: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use diskgoblin_core::{
        AgentRunRecord, ExecutionMetrics, HarnessInfo, HumanInterventions, ModelInfo,
        QualityReviewMetrics, RunOutcome, TimingMetrics, UsageMetrics, AGENT_RUN_SCHEMA_VERSION,
    };

    fn sample_record(issue_id: &str, status: &str, wall_seconds: u64) -> AgentRunRecord {
        AgentRunRecord {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            run_id: format!("2026-10-31T10-00-00Z_{issue_id}_opencode_test-model"),
            issue_id: issue_id.to_owned(),
            milestone: "M4".to_owned(),
            starting_commit_sha: "aaaa".to_owned(),
            final_commit_sha: "bbbb".to_owned(),
            agents_md_sha256: "agents".to_owned(),
            issue_spec_sha256: "spec".to_owned(),
            harness: HarnessInfo {
                name: "opencode".to_owned(),
                version: "1.0.0".to_owned(),
                sandbox_mode: "workspace-write".to_owned(),
            },
            model: ModelInfo {
                provider: "anthropic".to_owned(),
                model_id: "test-model".to_owned(),
                billing_mode: "subscription".to_owned(),
            },
            timing: TimingMetrics {
                started_at: "2026-10-31T10:00:00Z".to_owned(),
                completed_at: "2026-10-31T10:10:00Z".to_owned(),
                wall_clock_seconds: wall_seconds,
                rate_limit_pause_seconds: 0,
                active_execution_seconds: wall_seconds,
            },
            usage: UsageMetrics {
                input_tokens: 1000,
                cached_input_tokens: 0,
                output_tokens: 100,
                reasoning_tokens: 0,
                measured_api_cost_usd: 0.0,
                imputed_list_cost_usd: 0.25,
                rate_limit_events: 0,
            },
            execution_metrics: ExecutionMetrics {
                tool_calls_total: 12,
                tool_call_retries: 0,
                compile_or_test_failures: 0,
                repair_attempts: 0,
            },
            quality_and_review: QualityReviewMetrics {
                cargo_fmt_passed: true,
                cargo_clippy_passed: true,
                cargo_test_passed: true,
                scope_check_passed: true,
                reviewer_agent_verdict: "APPROVE".to_owned(),
                reviewer_blocking_findings: 0,
                reviewer_non_blocking_findings: 0,
            },
            human_interventions: HumanInterventions {
                count: 0,
                total_human_minutes: 0.0,
                categories: Vec::new(),
            },
            outcome: RunOutcome {
                status: status.to_owned(),
                pr_number: Some(17),
            },
        }
    }

    #[test]
    fn renders_markdown_for_a_directory_of_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        for (name, record) in [
            ("a.json", sample_record("DG-001", "accepted_first_pr", 600)),
            ("b.json", sample_record("DG-002", "rejected", 900)),
        ] {
            std::fs::write(
                dir.path().join(name),
                serde_json::to_string_pretty(&record).expect("serialize"),
            )
            .expect("write fixture");
        }

        let report = build_report(dir.path()).expect("report");
        assert!(report.starts_with("# DiskGoblin Factory Experiment Summary"));
        assert!(report.contains("| Harness | Provider / Model |"));
        assert!(report.contains("| opencode | anthropic / test-model | 2 | 50.0 | 50.0 |"));
        assert!(report.contains("| DG-001 |"));
        assert!(report.contains("| DG-002 |"));
    }

    #[test]
    fn fails_when_the_directory_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing");
        let error = build_report(&missing).expect_err("missing directory must fail");
        assert!(error.contains("missing"), "{error}");
    }

    #[test]
    fn fails_when_a_record_is_invalid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut record = sample_record("DG-003", "accepted_first_pr", 600);
        record.timing.active_execution_seconds = 1;
        std::fs::write(
            dir.path().join("bad.json"),
            serde_json::to_string(&record).expect("serialize"),
        )
        .expect("write fixture");

        let error = build_report(dir.path()).expect_err("invalid record must fail");
        assert!(error.contains("bad.json"), "{error}");
        assert!(error.contains("active_execution_seconds"), "{error}");
    }

    #[test]
    fn default_directory_is_the_documented_run_log_location() {
        assert_eq!(
            Path::new(DEFAULT_RUN_DIR),
            Path::new("docs/experiments/runs")
        );
    }
}
