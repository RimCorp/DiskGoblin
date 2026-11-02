//! Software factory telemetry: run-record schema, validation, and experiment
//! aggregation.
//!
//! Every agent task execution produces one JSON record under
//! `docs/experiments/runs/<run_id>.json`. The record shape is specified by
//! `docs/experiments/experiment-plan.md` §4 and written by
//! `scripts/record-agent-run.sh`. This module owns the Rust mirror of that
//! schema, the deterministic validator that rejects malformed records, the
//! per-configuration aggregator used for the `M4` comparison tables, and the
//! GitHub-Flavored Markdown renderer.
//!
//! Categorical values recorded by hand or by different harness versions are
//! deliberately kept as [`String`] rather than closed enums (for example
//! `billing_mode` may be `api_metered`, `subscription_included`, or
//! `subscription`). Telemetry that fails to parse is lost forever, so the
//! reader tolerates unknown spellings and reports them verbatim instead of
//! rejecting the run.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Version of the agent run record schema understood by this module.
///
/// Bump this whenever the JSON shape changes in a backwards-incompatible way so
/// that aggregation tools can reject records they cannot interpret.
pub const AGENT_RUN_SCHEMA_VERSION: u32 = 1;

/// One recorded agent task execution (`docs/experiments/runs/<run_id>.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRunRecord {
    /// Schema version of this record; must equal [`AGENT_RUN_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Unique run identifier, usually `<timestamp>_<issue>_<harness>_<model>`.
    pub run_id: String,
    /// Backlog issue the run was assigned, e.g. `DG-006`.
    pub issue_id: String,
    /// Milestone label the issue belongs to, e.g. `M1`.
    pub milestone: String,
    /// Commit SHA the run started from in its isolated worktree.
    pub starting_commit_sha: String,
    /// Commit SHA the run finished on.
    pub final_commit_sha: String,
    /// SHA-256 of `AGENTS.md` as read by the agent at run start.
    pub agents_md_sha256: String,
    /// SHA-256 of the assigned issue specification.
    pub issue_spec_sha256: String,
    /// Harness that executed the task.
    pub harness: HarnessInfo,
    /// Foundation model and billing mode used by the harness.
    pub model: ModelInfo,
    /// Wall-clock and rate-limit timing for the run.
    pub timing: TimingMetrics,
    /// Token counts and cost for the run.
    pub usage: UsageMetrics,
    /// Harness-level execution counters.
    pub execution_metrics: ExecutionMetrics,
    /// Quality-gate results and independent review findings.
    pub quality_and_review: QualityReviewMetrics,
    /// Human attention spent on the run.
    pub human_interventions: HumanInterventions,
    /// Final acceptance status and pull request number.
    pub outcome: RunOutcome,
}

impl AgentRunRecord {
    /// Input, output, and reasoning tokens consumed by this run.
    pub fn total_tokens(&self) -> u64 {
        self.usage
            .input_tokens
            .saturating_add(self.usage.output_tokens)
            .saturating_add(self.usage.reasoning_tokens)
    }

    /// Dollar cost attributed to this run.
    ///
    /// Metered API spend wins when it was actually measured; subscription and
    /// otherwise unmetered runs fall back to the imputed list-price cost.
    pub fn cost_usd(&self) -> f64 {
        if self.usage.measured_api_cost_usd > 0.0 {
            self.usage.measured_api_cost_usd
        } else {
            self.usage.imputed_list_cost_usd
        }
    }

    /// Active execution time in minutes, excluding rate-limit pauses.
    pub fn active_minutes(&self) -> f64 {
        // u64 -> f64 is lossy only above 2^53 seconds (285 million years).
        self.timing.active_execution_seconds as f64 / 60.0
    }
}

/// Harness that executed the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessInfo {
    /// Harness name, e.g. `opencode` or `codex`.
    pub name: String,
    /// Harness version string as reported by the harness itself.
    pub version: String,
    /// Sandbox policy the harness ran under, e.g. `workspace-write`.
    pub sandbox_mode: String,
}

/// Foundation model and billing mode used by the harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model provider, e.g. `anthropic`, `openai`, or `deepseek`.
    pub provider: String,
    /// Exact model identifier reported by the provider.
    pub model_id: String,
    /// Billing mode: `api_metered`, `subscription_included`, or `subscription`.
    pub billing_mode: String,
}

/// Wall-clock and rate-limit timing for a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingMetrics {
    /// RFC 3339 timestamp for run start, or empty when unknown.
    pub started_at: String,
    /// RFC 3339 timestamp for run completion, or empty when unknown.
    pub completed_at: String,
    /// Total elapsed seconds from start to completion.
    pub wall_clock_seconds: u64,
    /// Seconds spent paused on provider rate limits.
    pub rate_limit_pause_seconds: u64,
    /// Wall-clock seconds minus rate-limit pauses.
    pub active_execution_seconds: u64,
}

/// Token counts and cost for a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageMetrics {
    /// Prompt tokens consumed.
    pub input_tokens: u64,
    /// Prompt tokens served from the provider's prompt cache.
    pub cached_input_tokens: u64,
    /// Completion tokens produced.
    pub output_tokens: u64,
    /// Reasoning tokens produced, where the provider reports them separately.
    pub reasoning_tokens: u64,
    /// Measured spend in USD, or `0.0` when the run was not API-metered.
    pub measured_api_cost_usd: f64,
    /// Spend imputed at public list prices, always populated.
    pub imputed_list_cost_usd: f64,
    /// Number of provider rate-limit events observed during the run.
    pub rate_limit_events: u64,
}

/// Harness-level execution counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Total tool calls issued by the agent.
    pub tool_calls_total: u64,
    /// Tool calls that were retried after a transient failure.
    pub tool_call_retries: u64,
    /// Compile or test command invocations that failed.
    pub compile_or_test_failures: u64,
    /// Repair loops entered to reach a green quality gate.
    pub repair_attempts: u64,
}

/// Quality-gate results and independent review findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityReviewMetrics {
    /// `cargo fmt --all -- --check` passed on the final submission.
    pub cargo_fmt_passed: bool,
    /// `cargo clippy --workspace --all-targets -- -D warnings` passed.
    pub cargo_clippy_passed: bool,
    /// `cargo test --workspace --all-targets` passed.
    pub cargo_test_passed: bool,
    /// Scope containment check passed.
    pub scope_check_passed: bool,
    /// Reviewer verdict, e.g. `APPROVE` or `REQUEST_CHANGES`.
    pub reviewer_agent_verdict: String,
    /// Number of blocking review findings.
    pub reviewer_blocking_findings: u64,
    /// Number of non-blocking review findings.
    pub reviewer_non_blocking_findings: u64,
}

/// Human attention spent on a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanInterventions {
    /// Number of human interventions required.
    pub count: u64,
    /// Total human minutes spent on the run.
    pub total_human_minutes: f64,
    /// Free-form intervention categories, e.g. `spec_clarification`.
    pub categories: Vec<String>,
}

/// Final acceptance status and pull request number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunOutcome {
    /// One of `accepted_first_pr`, `accepted_after_repair`,
    /// `accepted_after_spec_clarification`, `rejected`, or `blocked`.
    ///
    /// Kept as a string so unfamiliar statuses recorded by future runs are
    /// still parsed and reported rather than silently dropped.
    pub status: String,
    /// Pull request number, absent for non-merged comparison arms.
    #[serde(default)]
    pub pr_number: Option<u64>,
}

impl RunOutcome {
    /// Whether the run was eventually accepted, in any acceptance mode.
    pub fn is_accepted(&self) -> bool {
        self.status.trim().starts_with("accepted_")
    }

    /// Whether the run was accepted on its first pull request submission.
    pub fn is_first_pass_accepted(&self) -> bool {
        self.status.trim() == "accepted_first_pr"
    }
}

/// Validates one run record against the invariants this module relies on.
///
/// Checks the schema version, the presence of the identifiers used for grouping
/// and reporting, and that `active_execution_seconds` is exactly the wall-clock
/// time minus rate-limit pauses. Returns a human-readable, field-named error on
/// the first violation.
pub fn validate_run_record(record: &AgentRunRecord) -> Result<(), String> {
    if record.schema_version != AGENT_RUN_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version {} (expected {AGENT_RUN_SCHEMA_VERSION})",
            record.schema_version
        ));
    }
    for (field, value) in [
        ("run_id", record.run_id.as_str()),
        ("issue_id", record.issue_id.as_str()),
        ("milestone", record.milestone.as_str()),
        ("harness.name", record.harness.name.as_str()),
        ("model.model_id", record.model.model_id.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{field} must not be empty"));
        }
    }
    let expected_active = record
        .timing
        .wall_clock_seconds
        .saturating_sub(record.timing.rate_limit_pause_seconds);
    if record.timing.active_execution_seconds != expected_active {
        return Err(format!(
            "timing.active_execution_seconds {} does not equal wall_clock_seconds {} minus rate_limit_pause_seconds {} (expected {expected_active})",
            record.timing.active_execution_seconds,
            record.timing.wall_clock_seconds,
            record.timing.rate_limit_pause_seconds
        ));
    }
    Ok(())
}

/// Loads, parses, and validates every `*.json` run record in `dir`.
///
/// Files are visited in filename order so that callers get a deterministic
/// vector regardless of directory iteration order. Any unreadable file,
/// malformed JSON document, or invalid record fails the whole load with an error
/// naming the offending path.
pub fn load_run_records_from_dir(dir: &Path) -> Result<Vec<AgentRunRecord>, String> {
    let entries = std::fs::read_dir(dir).map_err(|err| {
        format!(
            "failed to read run record directory {}: {err}",
            dir.display()
        )
    })?;

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|err| format!("failed to read entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        let is_json = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
        if is_json && path.is_file() {
            files.push(path);
        }
    }
    files.sort_by_key(|path| path.file_name().map(std::ffi::OsStr::to_os_string));

    let mut records = Vec::with_capacity(files.len());
    for path in files {
        let contents = std::fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let record: AgentRunRecord = serde_json::from_str(&contents)
            .map_err(|err| format!("{}: invalid run record JSON: {err}", path.display()))?;
        validate_run_record(&record).map_err(|err| format!("{}: {err}", path.display()))?;
        records.push(record);
    }
    Ok(records)
}

/// Aggregated metrics for one `(harness, provider, model)` configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelConfigSummary {
    /// Harness name shared by every run in this configuration.
    pub harness_name: String,
    /// Model provider shared by every run in this configuration.
    pub provider: String,
    /// Model identifier shared by every run in this configuration.
    pub model_id: String,
    /// Number of runs in this configuration.
    pub tasks_attempted: usize,
    /// Runs accepted on the first pull request submission.
    pub accepted_first_pass: usize,
    /// Runs accepted in any acceptance mode.
    pub accepted_eventual: usize,
    /// `accepted_first_pass / tasks_attempted`, as a percentage.
    pub first_pass_acceptance_rate_pct: f64,
    /// `accepted_eventual / tasks_attempted`, as a percentage.
    pub eventual_acceptance_rate_pct: f64,
    /// Median active minutes across accepted runs, `0.0` when none accepted.
    pub median_active_minutes_per_accepted: f64,
    /// Mean active minutes across accepted runs, `0.0` when none accepted.
    pub mean_active_minutes_per_accepted: f64,
    /// Median cost in USD across accepted runs, `0.0` when none accepted.
    pub median_cost_usd_per_accepted: f64,
    /// Mean cost in USD across accepted runs, `0.0` when none accepted.
    pub mean_cost_usd_per_accepted: f64,
    /// Input + output + reasoning tokens across every run.
    pub total_tokens: u64,
    /// Mean repair attempts per attempted run.
    pub mean_repair_attempts: f64,
    /// Human interventions recorded across every run.
    pub total_human_interventions: u64,
    /// Human minutes across accepted runs, per accepted run, `0.0` when none.
    pub human_minutes_per_accepted: f64,
}

/// Aggregates run records by `(harness.name, model.provider, model.model_id)`.
///
/// The returned vector is ordered by that tuple, so repeated invocations over
/// the same records always produce byte-identical reports.
pub fn summarize_by_configuration(records: &[AgentRunRecord]) -> Vec<ModelConfigSummary> {
    let mut groups: BTreeMap<(&str, &str, &str), Vec<&AgentRunRecord>> = BTreeMap::new();
    for record in records {
        groups
            .entry((
                record.harness.name.as_str(),
                record.model.provider.as_str(),
                record.model.model_id.as_str(),
            ))
            .or_default()
            .push(record);
    }

    groups
        .into_iter()
        .map(|((harness_name, provider, model_id), runs)| {
            let tasks_attempted = runs.len();
            let accepted: Vec<&AgentRunRecord> = runs
                .iter()
                .copied()
                .filter(|record| record.outcome.is_accepted())
                .collect();
            let accepted_first_pass = runs
                .iter()
                .filter(|record| record.outcome.is_first_pass_accepted())
                .count();
            let accepted_eventual = accepted.len();

            let active_minutes: Vec<f64> = accepted
                .iter()
                .map(|record| record.active_minutes())
                .collect();
            let costs: Vec<f64> = accepted.iter().map(|record| record.cost_usd()).collect();

            let total_tokens = runs
                .iter()
                .map(|record| record.total_tokens())
                .fold(0u64, u64::saturating_add);
            let total_repair_attempts = runs
                .iter()
                .map(|record| record.execution_metrics.repair_attempts)
                .fold(0u64, u64::saturating_add);
            let total_human_interventions = runs
                .iter()
                .map(|record| record.human_interventions.count)
                .fold(0u64, u64::saturating_add);
            let accepted_human_minutes: Vec<f64> = accepted
                .iter()
                .map(|record| record.human_interventions.total_human_minutes)
                .collect();

            ModelConfigSummary {
                harness_name: harness_name.to_owned(),
                provider: provider.to_owned(),
                model_id: model_id.to_owned(),
                tasks_attempted,
                accepted_first_pass,
                accepted_eventual,
                first_pass_acceptance_rate_pct: percentage(accepted_first_pass, tasks_attempted),
                eventual_acceptance_rate_pct: percentage(accepted_eventual, tasks_attempted),
                median_active_minutes_per_accepted: median(&active_minutes),
                mean_active_minutes_per_accepted: mean(&active_minutes),
                median_cost_usd_per_accepted: median(&costs),
                mean_cost_usd_per_accepted: mean(&costs),
                total_tokens,
                mean_repair_attempts: if tasks_attempted == 0 {
                    0.0
                } else {
                    total_repair_attempts as f64 / tasks_attempted as f64
                },
                total_human_interventions,
                human_minutes_per_accepted: mean(&accepted_human_minutes),
            }
        })
        .collect()
}

/// Renders the `M4` experiment report as GitHub-Flavored Markdown.
///
/// The report has two tables: a per-configuration comparison table ordered by
/// `(harness, provider, model)` and a per-run ledger ordered by
/// `(issue_id, run_id)`. Floats are rounded for display only; aggregation always
/// uses full precision. The returned string always ends with a newline.
pub fn render_experiment_summary_markdown(records: &[AgentRunRecord]) -> String {
    let mut output = String::new();
    output.push_str("# DiskGoblin Factory Experiment Summary\n\n");
    output.push_str(&format!("Run records: {}\n\n", records.len()));

    if records.is_empty() {
        output.push_str("_No run records found._\n");
        return output;
    }

    output.push_str("## Summary Comparison\n\n");
    push_table_header(
        &mut output,
        &[
            "Harness",
            "Provider / Model",
            "Attempted",
            "First-Pass %",
            "Eventual %",
            "Median Active Min",
            "Mean Active Min",
            "Median Cost ($)",
            "Mean Cost ($)",
            "Total Tokens",
            "Mean Repairs",
            "Human Interventions",
        ],
    );
    for summary in summarize_by_configuration(records) {
        push_table_row(
            &mut output,
            &[
                escape_cell(&summary.harness_name),
                format!(
                    "{} / {}",
                    escape_cell(&summary.provider),
                    escape_cell(&summary.model_id)
                ),
                summary.tasks_attempted.to_string(),
                format_decimal(summary.first_pass_acceptance_rate_pct, 1),
                format_decimal(summary.eventual_acceptance_rate_pct, 1),
                format_decimal(summary.median_active_minutes_per_accepted, 1),
                format_decimal(summary.mean_active_minutes_per_accepted, 1),
                format_decimal(summary.median_cost_usd_per_accepted, 2),
                format_decimal(summary.mean_cost_usd_per_accepted, 2),
                summary.total_tokens.to_string(),
                format_decimal(summary.mean_repair_attempts, 2),
                summary.total_human_interventions.to_string(),
            ],
        );
    }

    output.push_str("\n## Per-Run Ledger\n\n");
    push_table_header(
        &mut output,
        &[
            "Issue",
            "Milestone",
            "Harness",
            "Model",
            "Active Sec",
            "Tool Calls",
            "Repairs",
            "Cost ($)",
            "Outcome",
            "PR",
        ],
    );
    let mut ledger: Vec<&AgentRunRecord> = records.iter().collect();
    ledger.sort_by(|left, right| {
        left.issue_id
            .cmp(&right.issue_id)
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    for record in ledger {
        push_table_row(
            &mut output,
            &[
                escape_cell(&record.issue_id),
                escape_cell(&record.milestone),
                escape_cell(&record.harness.name),
                escape_cell(&record.model.model_id),
                record.timing.active_execution_seconds.to_string(),
                record.execution_metrics.tool_calls_total.to_string(),
                record.execution_metrics.repair_attempts.to_string(),
                format_decimal(record.cost_usd(), 2),
                escape_cell(&record.outcome.status),
                match record.outcome.pr_number {
                    Some(number) if number > 0 => format!("#{number}"),
                    _ => "-".to_owned(),
                },
            ],
        );
    }

    output
}

/// Percentage of `part` in `whole`, `0.0` when `whole` is zero.
fn percentage(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64 * 100.0
    }
}

/// Arithmetic mean of `values`, `0.0` when empty.
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Median of `values`, averaging the two middle samples for even lengths.
/// Returns `0.0` when empty. Uses [`f64::total_cmp`] so the ordering is total
/// and never panics on a NaN sample.
fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len() & 1 == 0 {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

/// Formats `value` with a fixed number of decimal places for report output.
fn format_decimal(value: f64, places: usize) -> String {
    format!("{value:.places$}")
}

/// Escapes table-cell content so a stray pipe or newline cannot break a table.
fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// Appends a GitHub-Flavored Markdown table header and separator row.
fn push_table_header(output: &mut String, columns: &[&str]) {
    output.push('|');
    for column in columns {
        output.push(' ');
        output.push_str(column);
        output.push_str(" |");
    }
    output.push('\n');
    output.push('|');
    for _ in columns {
        output.push_str(" --- |");
    }
    output.push('\n');
}

/// Appends one GitHub-Flavored Markdown table row.
fn push_table_row(output: &mut String, cells: &[String]) {
    output.push('|');
    for cell in cells {
        output.push(' ');
        output.push_str(cell);
        output.push_str(" |");
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The exact record shape documented in `docs/experiments/experiment-plan.md` §4.
    const PLAN_EXAMPLE: &str = r#"{
      "schema_version": 1,
      "run_id": "2026-10-31T14-20-00Z_DG-006_opencode_claude-sonnet-4",
      "issue_id": "DG-006",
      "milestone": "M1",
      "starting_commit_sha": "a1b2c3d4e5f6",
      "final_commit_sha": "f6e5d4c3b2a1",
      "agents_md_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "issue_spec_sha256": "8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4",
      "harness": {
        "name": "opencode",
        "version": "0.4.2",
        "sandbox_mode": "workspace-write"
      },
      "model": {
        "provider": "anthropic",
        "model_id": "claude-sonnet-4-20250514",
        "billing_mode": "api_metered"
      },
      "timing": {
        "started_at": "2026-10-31T14:20:00Z",
        "completed_at": "2026-10-31T14:38:12Z",
        "wall_clock_seconds": 1092,
        "rate_limit_pause_seconds": 0,
        "active_execution_seconds": 1092
      },
      "usage": {
        "input_tokens": 142500,
        "cached_input_tokens": 98000,
        "output_tokens": 8420,
        "reasoning_tokens": 0,
        "measured_api_cost_usd": 0.34,
        "imputed_list_cost_usd": 0.34,
        "rate_limit_events": 0
      },
      "execution_metrics": {
        "tool_calls_total": 28,
        "tool_call_retries": 1,
        "compile_or_test_failures": 2,
        "repair_attempts": 2
      },
      "quality_and_review": {
        "cargo_fmt_passed": true,
        "cargo_clippy_passed": true,
        "cargo_test_passed": true,
        "scope_check_passed": true,
        "reviewer_agent_verdict": "APPROVE",
        "reviewer_blocking_findings": 0,
        "reviewer_non_blocking_findings": 1
      },
      "human_interventions": {
        "count": 0,
        "total_human_minutes": 0.0,
        "categories": []
      },
      "outcome": {
        "status": "accepted_first_pr",
        "pr_number": 6
      }
    }"#;

    /// A valid baseline record; tests override the fields they exercise.
    fn base_record(run_id: &str, issue_id: &str) -> AgentRunRecord {
        AgentRunRecord {
            schema_version: AGENT_RUN_SCHEMA_VERSION,
            run_id: run_id.to_owned(),
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
                model_id: "claude-sonnet-4".to_owned(),
                billing_mode: "subscription".to_owned(),
            },
            timing: TimingMetrics {
                started_at: "2026-10-31T10:00:00Z".to_owned(),
                completed_at: "2026-10-31T10:10:00Z".to_owned(),
                wall_clock_seconds: 600,
                rate_limit_pause_seconds: 0,
                active_execution_seconds: 600,
            },
            usage: UsageMetrics {
                input_tokens: 100,
                cached_input_tokens: 0,
                output_tokens: 10,
                reasoning_tokens: 1,
                measured_api_cost_usd: 0.0,
                imputed_list_cost_usd: 0.0,
                rate_limit_events: 0,
            },
            execution_metrics: ExecutionMetrics {
                tool_calls_total: 10,
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
                status: "accepted_first_pr".to_owned(),
                pr_number: Some(1),
            },
        }
    }

    /// Sets active execution seconds consistently with wall clock minus pause.
    fn with_timing(mut record: AgentRunRecord, wall: u64, pause: u64) -> AgentRunRecord {
        record.timing.wall_clock_seconds = wall;
        record.timing.rate_limit_pause_seconds = pause;
        record.timing.active_execution_seconds = wall.saturating_sub(pause);
        record
    }

    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    #[test]
    fn deserializes_and_round_trips_the_documented_schema() {
        let record: AgentRunRecord = serde_json::from_str(PLAN_EXAMPLE).expect("plan example");
        assert_eq!(record.schema_version, AGENT_RUN_SCHEMA_VERSION);
        assert_eq!(
            record.run_id,
            "2026-10-31T14-20-00Z_DG-006_opencode_claude-sonnet-4"
        );
        assert_eq!(record.model.billing_mode, "api_metered");
        assert!(record.outcome.is_accepted());
        assert!(record.outcome.is_first_pass_accepted());
        assert_eq!(record.outcome.pr_number, Some(6));
        assert_eq!(record.total_tokens(), 150_920);
        assert!(close(record.cost_usd(), 0.34));
        assert!(close(record.active_minutes(), 18.2));
        validate_run_record(&record).expect("plan example is valid");

        let serialized = serde_json::to_string(&record).expect("serialize");
        let reparsed: AgentRunRecord = serde_json::from_str(&serialized).expect("reparse");
        assert_eq!(record, reparsed);
    }

    #[test]
    fn accepts_every_recorded_categorical_variant() {
        for billing_mode in ["api_metered", "subscription_included", "subscription"] {
            let mut value: serde_json::Value =
                serde_json::from_str(PLAN_EXAMPLE).expect("plan example");
            value["model"]["billing_mode"] = json!(billing_mode);
            let record: AgentRunRecord =
                serde_json::from_value(value).expect("billing mode must parse");
            assert_eq!(record.model.billing_mode, billing_mode);
        }

        for status in [
            "accepted_first_pr",
            "accepted_after_repair",
            "accepted_after_spec_clarification",
            "rejected",
            "blocked",
        ] {
            let mut value: serde_json::Value =
                serde_json::from_str(PLAN_EXAMPLE).expect("plan example");
            value["outcome"]["status"] = json!(status);
            value["outcome"]["pr_number"] = json!(null);
            let record: AgentRunRecord = serde_json::from_value(value).expect("status parses");
            assert_eq!(record.outcome.status, status);
            assert_eq!(record.outcome.pr_number, None);
            assert_eq!(
                record.outcome.is_accepted(),
                status.starts_with("accepted_")
            );
            assert_eq!(
                record.outcome.is_first_pass_accepted(),
                status == "accepted_first_pr"
            );
        }
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut record = base_record("run-1", "DG-001");
        record.schema_version = 2;
        let error = validate_run_record(&record).expect_err("schema 2 must be rejected");
        assert!(error.contains("schema_version"), "{error}");
    }

    #[test]
    fn rejects_empty_identifiers() {
        let mut cleared_run_id = base_record("run-1", "DG-001");
        cleared_run_id.run_id.clear();
        assert_validation_error(cleared_run_id, "run_id");

        let mut blank_issue = base_record("run-1", "DG-001");
        blank_issue.issue_id = "   ".to_owned();
        assert_validation_error(blank_issue, "issue_id");

        let mut blank_milestone = base_record("run-1", "DG-001");
        blank_milestone.milestone.clear();
        assert_validation_error(blank_milestone, "milestone");

        let mut blank_harness = base_record("run-1", "DG-001");
        blank_harness.harness.name.clear();
        assert_validation_error(blank_harness, "harness.name");

        let mut blank_model = base_record("run-1", "DG-001");
        blank_model.model.model_id.clear();
        assert_validation_error(blank_model, "model.model_id");
    }

    /// Asserts that validation fails and names the offending field.
    fn assert_validation_error(record: AgentRunRecord, field: &str) {
        let error = validate_run_record(&record).expect_err("record must be rejected");
        assert!(error.contains(field), "expected {field} in {error}");
    }

    #[test]
    fn validates_rate_limit_pause_subtraction() {
        let paused = with_timing(base_record("run-1", "DG-001"), 1000, 250);
        assert_eq!(paused.timing.active_execution_seconds, 750);
        validate_run_record(&paused).expect("active time equals wall minus pause");

        let mut wrong = paused.clone();
        wrong.timing.active_execution_seconds = 1000;
        let error = validate_run_record(&wrong).expect_err("unsubtracted pause must be rejected");
        assert!(error.contains("rate_limit_pause_seconds"), "{error}");

        // A pause longer than the wall clock saturates at zero active seconds.
        let saturated = with_timing(base_record("run-2", "DG-001"), 100, 500);
        assert_eq!(saturated.timing.active_execution_seconds, 0);
        validate_run_record(&saturated).expect("saturating subtraction is valid");
    }

    #[test]
    fn loads_json_records_in_filename_order_and_skips_other_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut third = base_record("c", "DG-003");
        third.outcome.status = "rejected".to_owned();
        third.outcome.pr_number = None;
        let records = [
            ("b-run.json", base_record("b", "DG-002")),
            ("a-run.json", base_record("a", "DG-001")),
            ("c-run.json", third),
        ];
        for (name, record) in &records {
            let json = serde_json::to_string_pretty(record).expect("serialize");
            std::fs::write(dir.path().join(name), json).expect("write fixture");
        }
        std::fs::write(dir.path().join("notes.txt"), "ignored").expect("write non-json");

        let loaded = load_run_records_from_dir(dir.path()).expect("load records");
        assert_eq!(
            loaded.iter().map(|r| r.run_id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
        assert_eq!(loaded[2].outcome.status, "rejected");
    }

    #[test]
    fn surfaces_the_offending_file_on_invalid_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("broken.json"), "{ not json").expect("write invalid json");
        let error = load_run_records_from_dir(dir.path()).expect_err("malformed JSON must fail");
        assert!(error.contains("broken.json"), "{error}");

        let mut invalid = with_timing(base_record("run-1", "DG-001"), 100, 0);
        invalid.timing.active_execution_seconds = 42;
        let invalid_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            invalid_dir.path().join("invalid.json"),
            serde_json::to_string(&invalid).expect("serialize"),
        )
        .expect("write invalid record");
        let error =
            load_run_records_from_dir(invalid_dir.path()).expect_err("invalid record fails");
        assert!(error.contains("invalid.json"), "{error}");
        assert!(error.contains("active_execution_seconds"), "{error}");
    }

    #[test]
    fn reports_unreadable_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("does-not-exist");
        let error = load_run_records_from_dir(&missing).expect_err("missing dir must fail");
        assert!(error.contains("does-not-exist"), "{error}");
    }

    #[test]
    fn aggregates_accepted_runs_per_configuration() {
        let mut slow = base_record("run-1", "DG-001");
        slow = with_timing(slow, 300, 0);
        slow.usage.imputed_list_cost_usd = 0.30;
        slow.usage.measured_api_cost_usd = 0.0;
        slow.human_interventions.count = 1;
        slow.human_interventions.total_human_minutes = 2.0;

        let mut medium = base_record("run-2", "DG-002");
        medium = with_timing(medium, 600, 0);
        medium.usage.imputed_list_cost_usd = 0.10;
        medium.usage.measured_api_cost_usd = 0.50;
        medium.execution_metrics.repair_attempts = 2;

        let mut fast = base_record("run-3", "DG-003");
        fast = with_timing(fast, 1800, 0);
        fast.usage.imputed_list_cost_usd = 0.70;
        fast.outcome.status = "accepted_after_repair".to_owned();
        fast.execution_metrics.repair_attempts = 1;
        fast.human_interventions.count = 3;
        fast.human_interventions.total_human_minutes = 4.0;

        let summaries = summarize_by_configuration(&[slow, medium, fast]);
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.harness_name, "opencode");
        assert_eq!(summary.provider, "anthropic");
        assert_eq!(summary.model_id, "claude-sonnet-4");
        assert_eq!(summary.tasks_attempted, 3);
        assert_eq!(summary.accepted_first_pass, 2);
        assert_eq!(summary.accepted_eventual, 3);
        assert!(close(summary.first_pass_acceptance_rate_pct, 200.0 / 3.0));
        assert!(close(summary.eventual_acceptance_rate_pct, 100.0));
        // Active minutes are 5, 10, and 30; cost samples are 0.30, 0.50, 0.70.
        assert!(close(summary.median_active_minutes_per_accepted, 10.0));
        assert!(close(summary.mean_active_minutes_per_accepted, 15.0));
        assert!(close(summary.median_cost_usd_per_accepted, 0.50));
        assert!(close(summary.mean_cost_usd_per_accepted, 0.50));
        assert_eq!(summary.total_tokens, 333);
        assert!(close(summary.mean_repair_attempts, 1.0));
        assert_eq!(summary.total_human_interventions, 4);
        assert!(close(summary.human_minutes_per_accepted, 2.0));
    }

    #[test]
    fn groups_and_orders_configurations_deterministically() {
        let mut codex = base_record("run-b", "DG-002");
        codex.harness.name = "codex".to_owned();
        codex.model.provider = "openai".to_owned();
        codex.model.model_id = "gpt-5-codex".to_owned();

        let mut opencode_b = base_record("run-c", "DG-003");
        opencode_b.model.model_id = "claude-opus-4".to_owned();

        let opencode_a = base_record("run-a", "DG-001");

        let summaries =
            summarize_by_configuration(&[codex.clone(), opencode_b.clone(), opencode_a.clone()]);
        let keys: Vec<(&str, &str, &str)> = summaries
            .iter()
            .map(|s| {
                (
                    s.harness_name.as_str(),
                    s.provider.as_str(),
                    s.model_id.as_str(),
                )
            })
            .collect();
        assert_eq!(
            keys,
            vec![
                ("codex", "openai", "gpt-5-codex"),
                ("opencode", "anthropic", "claude-opus-4"),
                ("opencode", "anthropic", "claude-sonnet-4"),
            ]
        );

        let reversed: Vec<AgentRunRecord> = [opencode_a, opencode_b, codex]
            .iter()
            .cloned()
            .rev()
            .collect();
        assert_eq!(
            summarize_by_configuration(&reversed),
            summaries,
            "input order must not affect the summary"
        );
    }

    #[test]
    fn zero_accepted_runs_report_zero_rates_and_costs() {
        let mut rejected = with_timing(base_record("run-1", "DG-001"), 900, 300);
        rejected.outcome.status = "rejected".to_owned();
        rejected.outcome.pr_number = None;
        rejected.usage.measured_api_cost_usd = 1.25;
        rejected.human_interventions.count = 2;
        rejected.human_interventions.total_human_minutes = 7.5;

        let summaries = summarize_by_configuration(&[rejected]);
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.tasks_attempted, 1);
        assert_eq!(summary.accepted_first_pass, 0);
        assert_eq!(summary.accepted_eventual, 0);
        assert!(close(summary.first_pass_acceptance_rate_pct, 0.0));
        assert!(close(summary.eventual_acceptance_rate_pct, 0.0));
        assert!(close(summary.median_active_minutes_per_accepted, 0.0));
        assert!(close(summary.mean_active_minutes_per_accepted, 0.0));
        assert!(close(summary.median_cost_usd_per_accepted, 0.0));
        assert!(close(summary.mean_cost_usd_per_accepted, 0.0));
        assert!(close(summary.human_minutes_per_accepted, 0.0));
        // Totals still cover every attempted run.
        assert_eq!(summary.total_tokens, 111);
        assert_eq!(summary.total_human_interventions, 2);
    }

    #[test]
    fn median_and_mean_handle_even_lists_and_empty_input() {
        assert!(close(median(&[40.0, 10.0, 30.0, 20.0]), 25.0));
        assert!(close(median(&[5.0, 1.0, 3.0]), 3.0));
        assert!(close(median(&[]), 0.0));
        assert!(close(mean(&[2.0, 4.0, 9.0]), 5.0));
        assert!(close(mean(&[]), 0.0));
    }

    #[test]
    fn renders_summary_and_ledger_tables() {
        let mut first = with_timing(base_record("run-b", "DG-002"), 600, 0);
        first.outcome.status = "accepted_after_repair".to_owned();
        first.outcome.pr_number = Some(21);
        first.execution_metrics.repair_attempts = 2;
        first.execution_metrics.tool_calls_total = 14;
        first.usage.measured_api_cost_usd = 1.5;

        let mut second = with_timing(base_record("run-a", "DG-001"), 300, 0);
        second.human_interventions.count = 1;

        let markdown = render_experiment_summary_markdown(&[first, second]);
        assert!(markdown.starts_with("# DiskGoblin Factory Experiment Summary\n"));
        assert!(markdown.contains("Run records: 2"));
        assert!(markdown.contains("## Summary Comparison"));
        assert!(markdown.contains(
            "| Harness | Provider / Model | Attempted | First-Pass % | Eventual % | Median Active Min | Mean Active Min | Median Cost ($) | Mean Cost ($) | Total Tokens | Mean Repairs | Human Interventions |"
        ));
        assert!(markdown.contains(
            "| opencode | anthropic / claude-sonnet-4 | 2 | 50.0 | 100.0 | 7.5 | 7.5 | 0.75 | 0.75 | 222 | 1.00 | 1 |"
        ));
        assert!(markdown.contains("## Per-Run Ledger"));
        assert!(markdown.contains(
            "| Issue | Milestone | Harness | Model | Active Sec | Tool Calls | Repairs | Cost ($) | Outcome | PR |"
        ));

        let ledger_rows: Vec<&str> = markdown
            .lines()
            .skip_while(|line| !line.contains("| Issue |"))
            .filter(|line| line.starts_with("| DG-"))
            .collect();
        assert_eq!(ledger_rows.len(), 2);
        // Ledger is ordered by (issue_id, run_id), not by input order.
        assert!(ledger_rows[0].starts_with("| DG-001 | M4 | opencode | claude-sonnet-4 | 300 |"));
        assert!(ledger_rows[0].ends_with("| accepted_first_pr | #1 |"));
        assert!(ledger_rows[1]
            .starts_with("| DG-002 | M4 | opencode | claude-sonnet-4 | 600 | 14 | 2 | 1.50 |"));
        assert!(ledger_rows[1].ends_with("| accepted_after_repair | #21 |"));
        assert!(markdown.ends_with('\n'));
    }

    #[test]
    fn renders_absent_pr_numbers_and_escapes_pipes() {
        let mut record = base_record("run-1", "DG-001");
        record.outcome.status = "blocked".to_owned();
        record.outcome.pr_number = None;
        record.model.model_id = "claude|sonnet".to_owned();
        let markdown = render_experiment_summary_markdown(&[record]);
        assert!(markdown.contains(
            "| DG-001 | M4 | opencode | claude\\|sonnet | 600 | 10 | 0 | 0.00 | blocked | - |"
        ));
        assert!(markdown.contains("| opencode | anthropic / claude\\|sonnet |"));
        assert!(!markdown.contains("| claude|sonnet |"));
    }

    #[test]
    fn renders_empty_dataset_without_tables() {
        let markdown = render_experiment_summary_markdown(&[]);
        assert_eq!(
            markdown,
            "# DiskGoblin Factory Experiment Summary\n\nRun records: 0\n\n_No run records found._\n"
        );
        assert!(summarize_by_configuration(&[]).is_empty());
    }
}
