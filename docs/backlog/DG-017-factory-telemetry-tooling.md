# DG-017: Implement software factory run-log schema, validator, and metrics summary tool

- **Milestone:** M4 — Factory Evaluation
- **Type:** experiment
- **Priority:** P1
- **Complexity Estimate:** M (45–75 minutes)
- **Risk:** Low

## Objective
Implement a deterministic telemetry validation and aggregation module (in Rust or a self-contained CLI subcommand/binary `diskgoblin-factory-eval` / workspace tool) that validates agent run logs (`docs/experiments/runs/*.json`) against the schema in [`docs/experiments/experiment-plan.md`](../experiments/experiment-plan.md) and generates Markdown comparison tables for Milestone `M4`.

## Context
Per [`PLANNING_CHARTER.md` §8](../../PLANNING_CHARTER.md) and [`docs/experiments/experiment-plan.md`](../experiments/experiment-plan.md), DiskGoblin's secondary objective is evaluating autonomous software factory configurations using **accepted tasks per unit of time, cost, and human attention**.

## Scope
- `crates/diskgoblin-core/src/factory_metrics.rs` (or a small helper binary in `crates/diskgoblin-cli/src/bin/factory_eval.rs`):
  - Serde types matching the exact JSON schema in `docs/experiments/experiment-plan.md` §4 (`AgentRunRecord`, `HarnessInfo`, `ModelInfo`, `TimingMetrics`, `UsageMetrics`, `ExecutionMetrics`, `QualityReviewMetrics`, `HumanInterventions`, `RunOutcome`).
  - Aggregation logic computing per-(Harness, Model) summary metrics:
    - Total tasks attempted, First-Pass Acceptance Rate (%), Eventual Acceptance Rate (%)
    - Median & Mean Active Execution Time per accepted task (minutes)
    - Median & Mean API/Imputed Cost per accepted task ($ USD) and Total Tokens
    - Total Human Interventions & Human Minutes per accepted task
    - Mean Repair Attempts per task
  - Markdown table generator (`render_experiment_summary_markdown(&[AgentRunRecord]) -> String`).
- Sample fixture run records and unit tests verifying validation and aggregation math.

## Out of scope
- Running the live multi-model comparative experiment itself (`DG-018`).

## Dependencies
- `DG-001` (Workspace scaffold; can be developed in parallel with `M1`–`M3`).

## Implementation guidance
- Implementing `factory_eval` in Rust inside the workspace (`crates/diskgoblin-cli/src/bin/factory_eval.rs`) means it gets compiled, linted (`clippy`), and tested (`cargo test`) automatically by our existing CI without needing external scripting runtimes.

## Acceptance criteria
- [ ] `AgentRunRecord` deserializes and validates run JSON files conforming to `docs/experiments/experiment-plan.md` §4.
- [ ] Aggregator accurately computes First-Pass Acceptance Rate, Eventual Acceptance Rate, Active Time per Accepted Task, Cost per Accepted Task, and Human Attention per Accepted Task.
- [ ] `cargo run -p diskgoblin-cli --bin factory_eval -- docs/experiments/runs/` outputs a clean GitHub-Flavored Markdown comparison table.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

## Definition of done
- Unit tests verify schema parsing, edge cases (0 accepted tasks, rate-limit pause subtraction), and Markdown table rendering.
- PR opened linking `DG-017` with Agent Implementation Report.
