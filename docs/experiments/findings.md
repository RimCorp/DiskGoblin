# DiskGoblin software factory comparative findings — DG-018

## 1. Executive Summary of Software Factory Performance

This retrospective covers 20 recorded agent runs, representing 16 distinct issues, from 2026-10-31 through 2026-11-02. Evidence consists of the [schema-v1 run records](runs/), the [experiment plan](experiment-plan.md), the [SSD study](ssd-parallel-traversal-study.md), and the checked-in [metrics implementation](../../crates/diskgoblin-core/src/factory_metrics.rs). Historical run facts supplied with DG-018 are identified where the structured records lack causal detail. This report adds no new benchmark executions or application behavior.

The factory progressed through M0 (workspace, domain contracts, fixtures: 3 runs), M1 (sequential engine, aggregation, cancellation, serialization, CLI and integration verification: 11 runs including comparison arms), M2 (benchmarks, parallel traversal and property oracles: 3 runs), M3 (desktop IPC and React/D3 presentation: 2 runs), and M4 (telemetry tooling: 1 run). DG-018 is the synthesis, outside this 20-run cohort; DG-016 has no record here. Milestone coverage is not proof that every planned milestone deliverable was executed.

| Configuration | Harness × model | Assigned runs | Observed outcome |
| --- | --- | --- | --- |
| A | Codex CLI 0.166.0 × `openai/gpt-6.1-sol` | DG-001, DG-002, DG-004 Arm A, DG-005 Arm A, DG-006, DG-007, DG-009, DG-012, DG-014 (9) | 8/9 first-pass; 9/9 eventual; one specification clarification |
| B | OpenCode v2.2.5 × `openrouter/deepseek/deepseek-v4.1-flash-1017` | DG-003, DG-004 Arm B, DG-005 Arm B, DG-008, DG-010, DG-011, DG-013, DG-015, DG-017 (9) | 8/9 first-pass; 9/9 eventual; one recorded repair |
| C | OpenCode v2.2.5 × `openrouter/z-ai/glm-5.3-flash` | DG-004 Arm C, DG-005 Arm C (2) | 0/2 first-pass; 2/2 eventual; one repair per run |

A handled architecture, systems integration, concurrency and IPC; B handled fixtures, formatting, serialization, integration/property tests, benchmarks, telemetry and UI. That assignment is a confounder, not randomized evidence that either configuration is universally better. Overall, 16/20 runs (80%) are labeled first-pass and 20/20 eventually accepted. Acceptance includes non-merged experimental arms with null PR numbers; it does not mean 20 merged PRs or 20 independently audited CI passes.

C was retired after DG-005 under the study's routing decision: both comparisons required repair, with slower execution and higher API spend. The broad description “2.4–3.6× slower” spans comparisons against different opponents; the exact ratios below distinguish those denominators. Against B specifically, C was 2.45–3.08× slower and 1.95–2.80× as expensive (approximately 2.0–2.8×). Two observations justify a local allocation decision, not a general model capability claim.

## 2. Quantitative Comparison Table & Per-Run Ledger

The following fenced Markdown is the exact stdout of:

```bash
CARGO_HOME=/tmp/dg009-cargo-home cargo run --offline -p diskgoblin-cli --bin factory_eval -- docs/experiments/runs/
```

Cargo build messages go to stderr and are excluded. Keeping stdout verbatim preserves generator headings, ordering, rounding and the original ledger; interpretation follows separately.

```markdown
# DiskGoblin Factory Experiment Summary

Run records: 20

## Summary Comparison

| Harness | Provider / Model | Attempted | First-Pass % | Eventual % | Median Active Min | Mean Active Min | Median Cost ($) | Mean Cost ($) | Total Tokens | Mean Repairs | Human Interventions |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| codex | openai / gpt-6.1-sol | 9 | 88.9 | 100.0 | 1.8 | 2.4 | 0.00 | 0.00 | 4390023 | 0.00 | 1 |
| opencode | openrouter / deepseek/deepseek-v4.1-flash-1017 | 9 | 88.9 | 100.0 | 2.6 | 2.6 | 0.03 | 0.03 | 764569 | 0.11 | 0 |
| opencode | openrouter / z-ai/glm-5.3-flash | 2 | 0.0 | 100.0 | 4.8 | 4.8 | 0.06 | 0.06 | 373618 | 1.00 | 0 |

## Per-Run Ledger

| Issue | Milestone | Harness | Model | Active Sec | Tool Calls | Repairs | Cost ($) | Outcome | PR |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| DG-001 | M0 | codex | gpt-6.1-sol | 32 | 4 | 0 | 0.00 | accepted_first_pr | #20 |
| DG-002 | M0 | codex | gpt-6.1-sol | 75 | 14 | 0 | 0.00 | accepted_after_spec_clarification | #21 |
| DG-003 | M0 | opencode | deepseek/deepseek-v4.1-flash-1017 | 89 | 23 | 0 | 0.02 | accepted_first_pr | #22 |
| DG-004 | M1 | codex | gpt-6.1-sol | 73 | 10 | 0 | 0.00 | accepted_first_pr | - |
| DG-004 | M1 | opencode | deepseek/deepseek-v4.1-flash-1017 | 63 | 18 | 0 | 0.01 | accepted_first_pr | #23 |
| DG-004 | M1 | opencode | z-ai/glm-5.3-flash | 194 | 26 | 1 | 0.04 | accepted_after_repair | - |
| DG-005 | M1 | codex | gpt-6.1-sol | 106 | 9 | 0 | 0.00 | accepted_first_pr | #24 |
| DG-005 | M1 | opencode | deepseek/deepseek-v4.1-flash-1017 | 156 | 50 | 0 | 0.04 | accepted_first_pr | - |
| DG-005 | M1 | opencode | z-ai/glm-5.3-flash | 382 | 40 | 1 | 0.09 | accepted_after_repair | - |
| DG-006 | M1 | codex | gpt-6.1-sol | 108 | 9 | 0 | 0.00 | accepted_first_pr | #25 |
| DG-007 | M1 | codex | gpt-6.1-sol | 163 | 9 | 0 | 0.00 | accepted_first_pr | #26 |
| DG-008 | M1 | opencode | deepseek/deepseek-v4.1-flash-1017 | 58 | 38 | 0 | 0.02 | accepted_first_pr | #27 |
| DG-009 | M1 | codex | gpt-6.1-sol | 103 | 12 | 0 | 0.00 | accepted_first_pr | #28 |
| DG-010 | M1 | opencode | deepseek/deepseek-v4.1-flash-1017 | 266 | 39 | 0 | 0.03 | accepted_first_pr | #29 |
| DG-011 | M2 | opencode | deepseek/deepseek-v4.1-flash-1017 | 117 | 33 | 0 | 0.04 | accepted_first_pr | #31 |
| DG-012 | M2 | codex | gpt-6.1-sol | 417 | 31 | 0 | 0.00 | accepted_first_pr | #32 |
| DG-013 | M2 | opencode | deepseek/deepseek-v4.1-flash-1017 | 182 | 58 | 0 | 0.05 | accepted_first_pr | #33 |
| DG-014 | M3 | codex | gpt-6.1-sol | 237 | 21 | 0 | 0.00 | accepted_first_pr | #34 |
| DG-015 | M3 | opencode | deepseek/deepseek-v4.1-flash-1017 | 258 | 31 | 0 | 0.03 | accepted_first_pr | #36 |
| DG-017 | M4 | opencode | deepseek/deepseek-v4.1-flash-1017 | 205 | 44 | 1 | 0.03 | accepted_after_repair | #35 |
```

### Metric interpretation and accepted-work throughput

`factory_eval` groups by harness/provider/model, without stratifying task difficulty or billing mode. First-pass acceptance means the exact status `accepted_first_pr`; eventual acceptance means any `accepted_` status. Thus DG-002 reduces A's first-pass rate despite no code repair. Compile/test failures during implementation can coexist with first-PR acceptance. All 20 records report passing final fmt, Clippy, tests, scope and reviewer approval, with zero recorded reviewer findings; these are recorded outcomes, not independent reproduction of historical checks.

| Configuration | Summed active time | Accepted runs / active hour | Cost basis and total | Accepted runs / measured API dollar | Recorded human minutes / accepted run |
| --- | ---: | ---: | --- | ---: | ---: |
| A | 1,314 s (21.90 min) | 24.66 | Subscription; $0 measured and $0 imputed placeholders | Undefined | 1/9 = 0.111 |
| B | 1,394 s (23.23 min) | 23.24 | API-metered: $0.274361 | 32.80 | 0 |
| C | 576 s (9.60 min) | 12.50 | API-metered: $0.121539 | 16.46 | 0 |

Rates equal accepted run count divided by summed active seconds/3,600 or measured dollars. They describe this task mix; they are not calendar delivery throughput, because runs overlap and omit orchestration, downstream review and merge waiting. Total active effort is 3,284 s (54.73 min). Zero recorded human minutes for B/C provides no finite “tasks per human hour” estimate; scheduling and review attention may be outside the measurement boundary.

### Head-to-Head Paired Benchmark

The only executed three-way pairs are DG-004 (Pure Logic: Byte Formatting) and DG-005 (Systems I/O: Sequential Traversal). Each triplet has identical `starting_commit_sha`, `agents_md_sha256` and `issue_spec_sha256`. DG-004 starts at `6acb159dfe441bcad541b7dc45e2b85b9d937cca`; DG-005 starts at `a32c94d26f9c927786791c11f67e4ee7d93efb07`. These fields support a common input baseline; the records do not independently attest identical full prompts, test-oracle hashes, cache isolation or randomized ordering.

| Task | A active / repairs | B active / repairs / measured API cost | C active / repairs / measured API cost | Comparison |
| --- | --- | --- | --- | --- |
| DG-004 | 73 s / 0 | 63 s / 0 / $0.012705 | 194 s / 1 / $0.035611 | B is 1.16× faster than A; C takes 3.08× B's time, 2.66× A's time and 2.80× B's dollars |
| DG-005 | 106 s / 0 | 156 s / 0 / $0.044026 | 382 s / 1 / $0.085928 | A is 1.47× faster than B; C takes 2.45× B's time, 3.60× A's time and 1.95× B's dollars |

A and B each accept both paired tasks on the first PR; C accepts both after repair. Across the two tasks, A takes 179 s, B 219 s and C 576 s; C costs $0.121539 versus B's $0.056731, a 2.14× paired API spend ratio. B wins the small logic case, A the traversal case. B-versus-C holds the harness constant, making it the stronger model comparison, although one sample per task per configuration cannot establish variance or statistical significance. A-versus-B changes both harness and model.

### Complexity-stratified performance beyond the pairs

| Canonical task / stratum | Observed configuration | Active time | Tool calls | Failed compile/test invocations | Repairs / outcome |
| --- | --- | ---: | ---: | ---: | --- |
| DG-006: large, aggregation/top-K/hard-link attribution | A only | 108 s (1.80 min) | 9 | 0 | 0 / first PR |
| DG-009: medium, CLI/rendering/signals | A only | 103 s (1.72 min) | 12 | 4 | 0 / first PR |
| DG-012: large, parallel traversal/performance | A only | 417 s (6.95 min) | 31 | 6 | 0 / first PR |

DG-012 takes 3.86× DG-006's time and 4.05× DG-009's time, consistent with substantial concurrency and empirical benchmarking work, but those causes cannot be partitioned from elapsed time alone. B and C were not run on these three canonical tasks: the planned five-task paired suite is only partially executed. No cross-model win can be inferred for these strata.

The SSD study provides a separate application-performance oracle: wide-tree speedup reaches 2.277× at eight workers, deep-tree only 1.392×, and the hard-link fixture peaks at 1.532× with two workers. No worker count clears the 1.5× gate across all fixtures, so the sequential CLI default remains. This evidence-based decision is a factory outcome; application milliseconds must not be confused with agent execution seconds.

### Measured billing and imputed token costs

B and C use OpenRouter `api_metered`; their combined recorded spend is $0.395900. B's mean is $0.030485 per accepted run; C's is $0.060770. These are historical record values, not current prices. API records' imputed fields equal their measured values and provide no separate tariff derivation.

A has seven `subscription_included` records and two legacy `subscription` records (DG-001/002). Both labels are accepted by schema v1 and denote subscription-backed execution here. Every A record has `measured_api_cost_usd = 0` and `imputed_list_cost_usd = 0`. These zeros establish neither zero economic cost nor an actual list-price estimate. The generator selects a positive measured cost, otherwise imputed cost; its $0.00 A cells faithfully reproduce missing imputation. Subscription fees cannot be allocated without usage/period data.

Token interpretation also differs. A's cached tokens fit within its input totals; OpenCode cached counts exceed `input_tokens`, consistent with uncached input and cache-read tokens being exported separately. Preserve those source fields rather than silently normalizing unverified semantics. Assuming these interpretations, A has 311,260 uncached input, 4,013,056 cached input, 58,656 output and 7,051 separately recorded reasoning tokens. For rates U (uncached input), C (cached input) and O (output including separately counted reasoning), in USD per million tokens, an explicit imputation is:

```text
A total imputed dollars = 0.311260 U + 4.013056 C + 0.065707 O
A mean imputed dollars per accepted run = total / 9
```

As a sensitivity calculation only, hypothetical U=$1, C=$0.10, O=$5 yields $1.041101 total, $0.115678/run. With no cache discount (C=$1), the same assumptions yield $4.652851 total, $0.516983/run. These are illustrative rates, not asserted provider list prices or measured spend. A defensible actual list-price comparison requires an archived model tariff and confirmation that reasoning tokens are not already included in output. No tariff provenance is supplied, so a precise actual A-versus-B dollar ranking is unavailable.

The generator's “Total Tokens” is input + output + reasoning, without adding cached tokens. It reports A=4,390,023, B=764,569, C=373,618. If OpenCode input excludes cache reads, comparable cache-inclusive totals would instead be B=12,965,545 and C=2,664,114; A remains 4,390,023 under the inclusive-input assumption. Consequently the generated totals cannot support a direct token-efficiency ranking without harmonizing exports.

## 3. Confounder Analysis

- **Harness granularity and model coupling.** Codex records 4–31 tool calls per task (119 total, mean 13.22); B records 18–58 (334 total, mean 37.11); C records 26–40 (66 total, mean 33). These are ranges across tasks, not average command counts. The reported harness practice bundles shell commands in Codex and uses finer read/edit/bash calls in OpenCode. The schema counts tool invocations, not individual shell commands, so fewer calls do not establish less work or better reasoning. Context compaction policies, retained history and tool schemas remain coupled with model selection; no same-model cross-harness or full 2×2 experiment was executed.
- **Prompt caching and accounting.** Under the interpretations above, aggregate cache-hit input ratios are A=4,013,056/4,324,316=92.80% and B=12,200,976/(12,200,976+559,185)=95.62%. Thus both harnesses' principal configurations exceed 90% in aggregate, not on every run. C is 89.37%. Caching can lower latency and spend, but the dataset lacks uncached counterfactuals; cached token counts are not fresh semantic work. Unknown compaction events prevent assigning a causal savings amount to either harness.
- **Dependency, build and filesystem warmth.** Shared `CARGO_HOME=/tmp/dg009-cargo-home` enables offline dependency reuse and avoids repeated resolution/download friction. It does not itself prove shared compiled artifacts: `CARGO_TARGET_DIR`, target reuse, OS page-cache state and clean-build timing are not recorded. The plan calls for isolated targets, but compliance cannot be verified from schema v1. The SSD measurements explicitly use warm metadata and Criterion warm-up. Keep that application benchmark method distinct from uncontrolled build/test time within agent runs; future pairs should record targets and randomize order.
- **Rate limits and elapsed-time boundaries.** Every record reports zero rate-limit events and pause seconds, so active equals wall-clock duration numerically. This supplies no evidence about behavior under throttling. Context compaction, provider latency, CPU contention and review time have no separate fields. Timestamps and counters are not a complete transcript of factory effort.
- **Sandbox and platform constraints.** The SSD study documents macOS Seatbelt denying `UnixListener::bind` with `EPERM` in `platform::tests::unix_socket_is_other`. That blocks a full test command independently of scanner correctness. Sandbox strings alone do not capture syscall allowances, network access or external reruns; comparing a blocked environment with a permissive one would bias reliability measurements.
- **Selection, provenance and acceptance.** Task routing favored A for highly coupled systems tasks and B for bounded deliverables; C was stopped after two runs, causing survivor and sample-size effects. All records share an AGENTS hash, and paired spec/start hashes match, but no full session logs, review artifacts or cold-cache repetitions are included. Zero reviewer findings and universal eventual acceptance should be read within this selected cohort, not as population reliability estimates.

## 4. Failure Mode Taxonomy

The table separates known historical causes from counters whose causes are not recoverable. No new repair history is invented to reconcile disagreements.

| Category | Runs and evidence | Consequence and interpretation |
| --- | --- | --- |
| Toolchain Version Skew | DG-007 and DG-017: DG-018's historical account identifies local `rustc 1.85.0` versus Actions `rustc 1.99.0`, with `clippy::manual_is_multiple_of` flagging `x % N == 0` | Power-of-two checks were expressed as `& 255 == 0` / `& 1 == 0`. Current source retains those masks. DG-017 records one repair and acceptance after repair; DG-007 records zero failures/repairs and first-PR acceptance, so its historical CI friction is absent from the counters. Exact CI version provenance is not stored in the JSON. |
| Compile / Borrow-Checker Repair Loops | DG-004 Arm C and DG-005 Arm C: each records one repair and `accepted_after_repair`; DG-004 C records two failed invocations, DG-005 C zero | Historical account describes internal compile/Clippy repair on both. No compiler diagnostics are included, so a particular borrow-checker error cannot be established. DG-005 C's zero failure counter is an instrumentation gap, not proof that no repair occurred. Both stayed within the three-loop budget. |
| Spec Ambiguity Escalation | DG-002: historical `PathBuf` versus `OsString` representation question before coding; status `accepted_after_spec_clarification`, one human minute/intervention, one tool retry, no repair | Clarification preserved the architecture contract. Its empty intervention categories omit the cause; outcome and attention counters capture the escalation without showing it as a model compilation defect. |
| Sandbox Syscall Restrictions | DG-012: SSD report identifies `UnixListener::bind` / `EPERM` under Seatbelt; record has six failed invocations and zero repairs | New parallel tests and a supplemental suite passed historically, but the full local test gate was blocked. The record's final `cargo_test_passed=true` and first-PR status do not prove the blocked command passed inside that sandbox; no external rerun provenance is present. |
| Other implementation-time command failures, cause unclassified | DG-004 A: 2; DG-004 B: 1; DG-005 B: 1; DG-009 A: 4; DG-010 B: 1; DG-014 A: 12; DG-015 B: 1 failed invocations | All retain zero repairs and first-PR status. Failures may precede submission; the records lack diagnostic causes. Do not relabel them as borrow errors, IPC failures or sandbox denials without logs. DG-014 is the largest failure-count entry despite eventual first-PR acceptance. |
| Scope / Unix edge-case outcomes | All 20 scope flags pass; no blocking/non-blocking review findings are recorded | No recorded scope violation, symlink loop, hard-link misattribution or Tauri serialization defect can be enumerated. The synthetic edge-case oracles and SSD study support tested behavior, not universal absence of defects. |

Across the cohort there are 30 recorded failed compile/test invocations, three repairs (C twice, B on DG-017 once), one tool retry (DG-002), and one human intervention. Nine runs have nonzero failure counters. The failure-count and repair-count concepts are demonstrably different. DG-001, DG-003, DG-006, DG-008, DG-011 and DG-013 have no recorded execution friction; DG-007's counters omit the supplied version-skew history, while DG-005 C and DG-017 demonstrate repairs with zero failed-invocation counts. Review outcomes, local sandbox gates and later CI must be recorded separately to avoid overstating first-pass autonomy.

## 5. Actionable Recommendations for Next-Generation AGENTS.md and Issue Decomposition

1. **Use provisional hybrid routing with explicit escalation.** Route highly coupled algorithms, shared state, cancellation, concurrency and IPC contracts to `gpt-6.1-sol`; route bounded tests, serialization, fixtures, UI and telemetry tooling to `deepseek-v4.1-flash`. Require typed inputs/outputs, allowed files and deterministic oracles in each issue. Reconsider routing when a task changes public interfaces or exceeds the three-repair budget. Keep GLM retired for this workload unless fresh controlled evidence warrants reevaluation. This policy follows the observed allocation and two small pairs, not a demonstrated model advantage on the unpaired large tasks.
2. **Pin the verification toolchain.** In a future scoped issue, add `rust-toolchain.toml` with an exact Rust release and fmt/Clippy components, and make CI consume that same pin. Record `rustc -Vv` and Clippy versions at run start. Upgrade deliberately with a dedicated lint migration; power-of-two masks solved these specific cases but are not a substitute for eliminating local/CI drift.
3. **Pre-warm offline dependency caches before dispatch.** Resolve/fetch the lockfile dependencies in a permitted setup environment, provision the shared `CARGO_HOME`, and have agents use offline verification. Record cache identity, target directory and build warmth. For timed comparisons, isolate `CARGO_TARGET_DIR` and standardize cold or prebuilt baselines; pre-warming downloads alone does not standardize compilation or OS metadata caches.
4. **Enforce DAG-based parallel issue dispatch.** Dispatch only when dependency issues are merged into the selected base SHA and allowed write sets are disjoint. Freeze public contracts before parallel implementation; serialize interface-changing producers ahead of consumers. Use isolated branches/worktrees, record base SHA and revalidate after integration. For example, domain/platform contracts precede traversal, traversal precedes aggregation/CLI oracles, and IPC contracts precede UI wiring. A scheduler should reject unmet prerequisites rather than relying on an agent to discover them mid-task.
5. **Make sandbox capabilities a pre-flight gate.** Probe required socket/filesystem/network capabilities before timing tasks; preserve failing syscall, errno and environment. Keep the required full suite visible as blocked when a supplemental filtered suite passes. Arrange an independent CI run in a capable environment and link its evidence; do not weaken tests to manufacture green local results.
6. **Define acceptance and instrumentation boundaries.** Separate pre-submission edits, failed command invocations, post-review repairs, spec clarifications, local/CI gate outcomes and human review minutes. Capture per-failure causes and tool retry reasons. Require cost-source metadata, archived tariffs, explicit inclusive/exclusive cached-token semantics and output/reasoning overlap rules. Preserve null PRs for comparison arms. These improvements need a later schema/tooling issue; this study preserves the supplied v1 records.
7. **Complete the experimental matrix before broad procurement claims.** Repeat and randomize DG-004/005, add B/C arms for DG-006/009/012 under identical held-out oracles, and include same-model cross-harness tests where supported. Archive full prompt/spec/toolchain hashes and logs, capture compaction/rate-limit events, and report variability. Evaluate accepted work per active time, measured versus imputed dollars, and full human attention independently. Extend SSD sampling beyond warm-cache quick mode before changing the traversal default.

### DG-018 verification and scope record

The report and the 20 existing JSON files are the only publication artifacts. JSON contents remain unchanged, including the legacy billing labels, zero imputed subscription costs and counter discrepancies; no dependencies or Rust/TypeScript files were changed. Validation establishes deserialization, schema version, grouping identifiers and active-time arithmetic as implemented by `factory_eval`; it does not establish historical truth or full schema semantic consistency.

Publication verification on the supplied isolated branch `chore/DG-018-factory-comparative-study`, based at `766187d60557d5689c7de8658aa4507b8ee474b1`:

- `CARGO_HOME=/tmp/dg009-cargo-home cargo fmt --all -- --check`: PASS.
- `CARGO_HOME=/tmp/dg009-cargo-home cargo clippy --offline --workspace --all-targets -- -D warnings`: PASS, no warnings.
- `CARGO_HOME=/tmp/dg009-cargo-home cargo run --offline -p diskgoblin-cli --bin factory_eval -- docs/experiments/runs/`: PASS, all 20 records; repeated stdout is byte-identical and embedded verbatim above.
- `CARGO_HOME=/tmp/dg009-cargo-home cargo test --offline --workspace --all-targets`: BLOCKED/FAIL (exit 101), existing `platform::tests::unix_socket_is_other` receives OS error 1 (`EPERM`). The core unit target reports 44 passed, 1 failed; later targets are not reached by that invocation. No source repair was attempted because the failure is a sandbox syscall restriction outside this documentation-only scope.
- Supplemental `CARGO_HOME=/tmp/dg009-cargo-home cargo test --offline --workspace --lib --bins --tests -- --skip platform::tests::unix_socket_is_other`: PASS, 56 unit and 38 integration tests, with one existing unit test filtered out. This does not replace the full gate or execute Criterion targets.
- Remote-main freshness could not be independently confirmed: this checkout has only the supplied task branch, and `git ls-remote origin refs/heads/main` failed because `github.com` could not resolve. The base contains the merged DG-013 and DG-017 prerequisites and the DG-015 merge. A PR and independent CI/review remain follow-ups; the full-test acceptance criterion remains unresolved until execution in a socket-capable environment.
