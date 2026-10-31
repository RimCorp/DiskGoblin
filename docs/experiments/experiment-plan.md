# DiskGoblin — Software Factory Experiment & Evaluation Plan

## 1. Experimental Objective

Beyond delivering DiskGoblin itself, our secondary objective (`PLANNING_CHARTER.md` §1 & §8) is to empirically evaluate the **reliability, cost, speed, and autonomy** of AI coding agents across different **agent harnesses** (`OpenCode` vs. `Codex CLI`) and **foundation models**.

### Primary Evaluation Metric
We evaluate configurations using:
> **Accepted Tasks per Unit of Time, Cost, and Human Attention**

Specifically:
1. **First-Pass Acceptance Rate (%):** Percentage of assigned tasks that pass all CI checks, verification tests, and independent code review on the first PR submission without human intervention.
2. **Eventual Acceptance Rate (%):** Percentage of assigned tasks accepted within the 3-repair-loop budget.
3. **Active Wall-Clock Time per Accepted Task (minutes):** Elapsed execution time excluding provider rate-limit pauses.
4. **Cost per Accepted Task ($ USD / Token equivalent):** Total input, cached-input, reasoning, and output tokens consumed across implementation and repair loops.
5. **Human Attention Cost (minutes & intervention count):** Number of human interventions required (spec clarifications, manual fixups, review overrides) and total human minutes spent per task.

We explicitly **reject** lines of code added, commit count, or self-reported agent confidence as quality metrics.

---

## 2. Controlled Experimental Design (A/B Paired Execution)

To compare two configurations (e.g., **Config A:** `OpenCode + Model X` vs. **Config B:** `Codex CLI + Model Y`, or same harness across two models) without sequential state drift:

1. **Identical Starting Commit:** Both configurations start from the exact same Git commit SHA on `main` in separate, isolated Git worktrees:
   - `../worktrees/exp-DG-006-config-A`
   - `../worktrees/exp-DG-006-config-B`
2. **Identical Task Prompt & Repository Context:** Both receive the exact same issue markdown (`docs/backlog/DG-XXX-*.md`) and read the exact same `AGENTS.md` and `docs/architecture.md` (recorded by SHA-256 hash).
3. **Identical Verification Oracle:** Both are graded against the exact same automated verification command suite (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) PLUS a held-out verification test suite from the downstream integration test.
4. **Canonical Benchmark Task Subset:**
   While all 18 tasks (`DG-001`–`DG-018`) record telemetry, we designate 5 tasks spanning varying complexity and risk levels as our **Paired Comparison Benchmark Suite**:
   - **Task 1 (Small / Pure Logic):** `DG-004` — Byte formatting (IEC & SI).
   - **Task 2 (Medium / Systems I/O & Edge Cases):** `DG-005` — Sequential traversal, symlink safety, and error collection.
   - **Task 3 (Large / Algorithmic & Data Structures):** `DG-006` — Bottom-up tree aggregation, top-K heaps, and hard-link deduplication.
   - **Task 4 (Medium / CLI & Signal Integration):** `DG-009` — Clap CLI, table rendering, and SIGINT handling.
   - **Task 5 (Large / Concurrency & Performance):** `DG-012` — Parallel scanner & thread scaling.

---

## 3. Confounders & Mitigation Controls

| Potential Confounder | How It Distorts Comparisons | Control / Mitigation |
|---|---|---|
| **Harness vs. Model Coupling** | Comparing `OpenCode + Claude` against `Codex CLI + GPT` conflates harness tool definitions/compaction with model capability. | Run a 2×2 matrix where feasible (or hold harness constant via `OpenCode` when comparing models, and hold model constant when comparing `OpenCode` vs. `Codex CLI`). |
| **Filesystem Cache Warmth** | The second agent running tests/benchmarks on the same machine benefits from warm OS page cache or pre-compiled Cargo artifacts. | Each worktree uses an isolated `CARGO_TARGET_DIR` (or clean build baseline) when measuring build/test timing; Criterion benchmarks run a dedicated warm-up phase before measurement. |
| **Subscription Rate Limits vs. Pay-Per-Token API** | Subscription tiers (ChatGPT Plus/Pro, Claude Max) may throttle or pause without reporting dollar cost, whereas API keys report exact token spend. | Record both raw token counts (`input_tokens`, `cached_tokens`, `output_tokens`, `reasoning_tokens`) AND compute imputed API cost at standard public list prices alongside `rate_limit_events`. |
| **Prompt / `AGENTS.md` Drift** | Updating `AGENTS.md` mid-milestone changes the instructions given to later runs. | Log `agents_md_sha256` and `issue_spec_sha256` in every run record; freeze `AGENTS.md` during paired runs. |

---

## 4. Telemetry Data Schema (`docs/experiments/runs/<run_id>.json`)

Every agent task execution produces a structured JSON record conforming to the following schema (validated and aggregated by `DG-017`):

```json
{
  "schema_version": 1,
  "run_id": "2026-10-31T14-20-00Z_DG-006_opencode_claude-sonnet-4",
  "issue_id": "DG-006",
  "milestone": "M1",
  "starting_commit_sha": "a1b2c3d4e5f6...",
  "final_commit_sha": "f6e5d4c3b2a1...",
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
}
```

---

## 5. Reporting Deliverables (`M4`)

At Milestone `M4` (`DG-018`), the factory telemetry CLI (`DG-017`) will process `docs/experiments/runs/*.json` and generate `docs/experiments/findings.md` containing:
1. Summary comparison table by Harness × Model (`Acceptance %`, `Median Active Time`, `Median Cost $`, `Mean Repair Loops`, `Human Interventions`).
2. Failure taxonomy breakdown (e.g., Unix metadata edge cases, borrow-checker errors in tree rollups, Tauri IPC serialization errors).
3. Concrete recommendations for improving `AGENTS.md` and task granularity.
