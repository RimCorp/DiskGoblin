# DG-018: Execute paired model/harness comparison study on benchmark tasks and publish findings

- **Milestone:** M4 — Factory Evaluation
- **Type:** experiment
- **Priority:** P2
- **Complexity Estimate:** L (90–120 minutes)
- **Risk:** Medium

## Objective
Execute the paired comparative benchmark protocol defined in [`docs/experiments/experiment-plan.md`](../experiments/experiment-plan.md) across the canonical task subset (`DG-004`, `DG-005`, `DG-006`, `DG-009`, `DG-012`), commit all structured run telemetry JSON files under `docs/experiments/runs/`, and publish the empirical synthesis in `docs/experiments/findings.md`.

## Context
Completes Milestone `M4` (Factory Evaluation). Synthesizes all telemetry gathered during `M0`–`M3` plus paired comparison runs in isolated Git worktrees to answer the charter's core questions about autonomous coding agent reliability, speed, cost, and failure modes.

## Scope
- `docs/experiments/runs/*.json`: Validated `AgentRunRecord` JSON files for all executed factory tasks and paired comparison runs.
- `docs/experiments/findings.md`:
  1. Executive Summary of Software Factory Performance.
  2. Quantitative Comparison Table generated via `factory_eval` (`DG-017`): Accepted tasks per unit of time, cost, and human attention across harnesses (`OpenCode`, `Codex CLI`) and models.
  3. Confounder Analysis (harness context compaction differences, rate-limit pauses, cache effects).
  4. Failure Mode Taxonomy (compile/borrow-checker repair loops, Unix filesystem edge cases, scope violations).
  5. Actionable Recommendations for next-generation `AGENTS.md` and issue decomposition.

## Out of scope
- Modifying DiskGoblin application features.

## Dependencies
- `DG-013` (Completed `M0`–`M2` benchmark tasks & property verification oracles)
- `DG-017` (`factory_eval` telemetry validator and summary generator)

## Implementation guidance
- Ensure every `.json` file in `docs/experiments/runs/` passes validation via `cargo run -p diskgoblin-cli --bin factory_eval -- docs/experiments/runs/`.
- Clearly separate measured API costs from imputed list-price costs for subscription-backed runs.

## Acceptance criteria
- [ ] All run telemetry files in `docs/experiments/runs/*.json` validate cleanly against `AgentRunRecord` schema v1.
- [ ] `docs/experiments/findings.md` includes the generated quantitative summary table, failure taxonomy, confounder analysis, and concrete lessons learned.
- [ ] All workspace tests and linters pass.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p diskgoblin-cli --bin factory_eval -- docs/experiments/runs/
```

## Definition of done
- All telemetry logs and `docs/experiments/findings.md` committed and verified; Milestone `M4` is complete.
- PR opened linking `DG-018` with Agent Implementation Report.
