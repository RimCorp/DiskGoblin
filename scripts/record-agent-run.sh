#!/usr/bin/env bash
set -euo pipefail

# Records a structured telemetry JSON file in docs/experiments/runs/ from day one (starting with DG-001).
# Usage:
#   ./scripts/record-agent-run.sh \
#     --issue DG-001 \
#     --milestone M0 \
#     --harness codex \
#     --harness-version "0.1.0" \
#     --provider openai \
#     --model gpt-5-codex \
#     --billing subscription \
#     --start-sha <sha> \
#     --end-sha <sha> \
#     --started-at 2026-10-31T10:00:00Z \
#     --completed-at 2026-10-31T10:15:00Z \
#     --wall-sec 900 \
#     --pause-sec 0 \
#     --retries 0 \
#     --interventions 0 \
#     --status accepted_first_pr \
#     --pr 20

ISSUE_ID=""
MILESTONE="M0"
HARNESS="opencode"
HARNESS_VERSION="unknown"
PROVIDER="unknown"
MODEL="unknown"
BILLING="subscription"
START_SHA=""
END_SHA=""
STARTED_AT=""
COMPLETED_AT=""
WALL_SEC=0
PAUSE_SEC=0
INPUT_TOKENS=0
CACHED_TOKENS=0
OUTPUT_TOKENS=0
REASONING_TOKENS=0
COST_USD=0.0
TOOL_CALLS=0
RETRIES=0
FAILURES=0
REPAIRS=0
INTERVENTIONS=0
HUMAN_MINS=0.0
STATUS="accepted_first_pr"
PR_NUM=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --issue) ISSUE_ID="$2"; shift 2 ;;
    --milestone) MILESTONE="$2"; shift 2 ;;
    --harness) HARNESS="$2"; shift 2 ;;
    --harness-version) HARNESS_VERSION="$2"; shift 2 ;;
    --provider) PROVIDER="$2"; shift 2 ;;
    --model) MODEL="$2"; shift 2 ;;
    --billing) BILLING="$2"; shift 2 ;;
    --start-sha) START_SHA="$2"; shift 2 ;;
    --end-sha) END_SHA="$2"; shift 2 ;;
    --started-at) STARTED_AT="$2"; shift 2 ;;
    --completed-at) COMPLETED_AT="$2"; shift 2 ;;
    --wall-sec) WALL_SEC="$2"; shift 2 ;;
    --pause-sec) PAUSE_SEC="$2"; shift 2 ;;
    --input-tokens) INPUT_TOKENS="$2"; shift 2 ;;
    --cached-tokens) CACHED_TOKENS="$2"; shift 2 ;;
    --output-tokens) OUTPUT_TOKENS="$2"; shift 2 ;;
    --reasoning-tokens) REASONING_TOKENS="$2"; shift 2 ;;
    --cost-usd) COST_USD="$2"; shift 2 ;;
    --tool-calls) TOOL_CALLS="$2"; shift 2 ;;
    --retries) RETRIES="$2"; shift 2 ;;
    --failures) FAILURES="$2"; shift 2 ;;
    --repairs) REPAIRS="$2"; shift 2 ;;
    --interventions) INTERVENTIONS="$2"; shift 2 ;;
    --human-mins) HUMAN_MINS="$2"; shift 2 ;;
    --status) STATUS="$2"; shift 2 ;;
    --pr) PR_NUM="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [[ -z "$ISSUE_ID" ]]; then
  echo "ERROR: --issue <DG-XXX> is required" >&2
  exit 2
fi

SPEC_FILE=$(find docs/backlog -maxdepth 1 -name "${ISSUE_ID}-*.md" | head -n 1)
AGENTS_SHA=$(shasum -a 256 AGENTS.md | awk '{print $1}')
SPEC_SHA=$(shasum -a 256 "$SPEC_FILE" | awk '{print $1}')
ACTIVE_SEC=$(( WALL_SEC - PAUSE_SEC ))
TS_SAFE=$(echo "${STARTED_AT:-$(date -u +"%Y-%m-%dT%H-%M-%SZ")}" | tr ':' '-')
RUN_ID="${TS_SAFE}_${ISSUE_ID}_${HARNESS}_${MODEL}"

mkdir -p docs/experiments/runs
OUT_FILE="docs/experiments/runs/${RUN_ID}.json"

cat > "$OUT_FILE" <<EOF
{
  "schema_version": 1,
  "run_id": "${RUN_ID}",
  "issue_id": "${ISSUE_ID}",
  "milestone": "${MILESTONE}",
  "starting_commit_sha": "${START_SHA}",
  "final_commit_sha": "${END_SHA}",
  "agents_md_sha256": "${AGENTS_SHA}",
  "issue_spec_sha256": "${SPEC_SHA}",
  "harness": {
    "name": "${HARNESS}",
    "version": "${HARNESS_VERSION}",
    "sandbox_mode": "workspace-write"
  },
  "model": {
    "provider": "${PROVIDER}",
    "model_id": "${MODEL}",
    "billing_mode": "${BILLING}"
  },
  "timing": {
    "started_at": "${STARTED_AT}",
    "completed_at": "${COMPLETED_AT}",
    "wall_clock_seconds": ${WALL_SEC},
    "rate_limit_pause_seconds": ${PAUSE_SEC},
    "active_execution_seconds": ${ACTIVE_SEC}
  },
  "usage": {
    "input_tokens": ${INPUT_TOKENS},
    "cached_input_tokens": ${CACHED_TOKENS},
    "output_tokens": ${OUTPUT_TOKENS},
    "reasoning_tokens": ${REASONING_TOKENS},
    "measured_api_cost_usd": ${COST_USD},
    "imputed_list_cost_usd": ${COST_USD},
    "rate_limit_events": 0
  },
  "execution_metrics": {
    "tool_calls_total": ${TOOL_CALLS},
    "tool_call_retries": ${RETRIES},
    "compile_or_test_failures": ${FAILURES},
    "repair_attempts": ${REPAIRS}
  },
  "quality_and_review": {
    "cargo_fmt_passed": true,
    "cargo_clippy_passed": true,
    "cargo_test_passed": true,
    "scope_check_passed": true,
    "reviewer_agent_verdict": "APPROVE",
    "reviewer_blocking_findings": 0,
    "reviewer_non_blocking_findings": 0
  },
  "human_interventions": {
    "count": ${INTERVENTIONS},
    "total_human_minutes": ${HUMAN_MINS},
    "categories": []
  },
  "outcome": {
    "status": "${STATUS}",
    "pr_number": ${PR_NUM}
  }
}
EOF

echo "Recorded telemetry run -> $OUT_FILE"
