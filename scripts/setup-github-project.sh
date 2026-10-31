#!/usr/bin/env bash
set -euo pipefail

REPO="RimCorp/DiskGoblin"
OWNER="RimCorp"

echo "=== 1. Verifying GitHub CLI authentication & project scopes ==="
gh auth status

echo "=== 2. Creating Labels in $REPO ==="
create_label() {
  local name="$1"
  local color="$2"
  local desc="$3"
  gh label create "$name" --repo "$REPO" --color "$color" --description "$desc" --force
}

create_label "type:feature"    "1D76DB" "New capability or module implementation"
create_label "type:bug"        "D73A4A" "Defect or regression fix"
create_label "type:chore"      "FEF2C0" "Build, CI, workspace, or test infrastructure"
create_label "type:research"   "D4C5F9" "Benchmarking, profiling, or technical spike"
create_label "type:experiment" "0E8A16" "Software factory evaluation and telemetry"

create_label "priority:P0"     "B60205" "Critical path blocker"
create_label "priority:P1"     "D93F0B" "High priority deliverable"
create_label "priority:P2"     "FBCA04" "Normal priority deliverable"

create_label "risk:low"        "C2E0C6" "Well-bounded task with low risk"
create_label "risk:medium"     "FEF2C0" "Moderate algorithmic or OS integration complexity"
create_label "risk:high"       "F9D0C4" "Concurrency, performance, or cross-layer risk"

create_label "status:ready"    "0E8A16" "All dependencies met; ready for coding agent"
create_label "status:blocked"  "B60205" "Blocked by ambiguity, failure, or upstream issue"
create_label "agent:escalated" "E99695" "Requires human review or decision"

echo "=== 3. Creating Milestones (M0..M4) in $REPO ==="
ensure_milestone() {
  local title="$1"
  local desc="$2"
  local existing
  existing=$(gh api "repos/$REPO/milestones" --jq ".[] | select(.title == \"$title\") | .number" || true)
  if [[ -z "$existing" ]]; then
    gh api "repos/$REPO/milestones" -f title="$title" -f description="$desc" > /dev/null
    echo "Created milestone: $title"
  else
    echo "Milestone already exists: $title (#$existing)"
  fi
}

ensure_milestone "M0 — Foundation" "Repository, workspace, CI, conventions, test infrastructure."
ensure_milestone "M1 — CLI MVP" "Filesystem scanning, aggregation, error handling, output, tests."
ensure_milestone "M2 — Performance" "Benchmarks, profiling, optimization, regression tests."
ensure_milestone "M3 — Desktop MVP" "Tauri integration, visualization, interaction."
ensure_milestone "M4 — Factory Evaluation" "Comparative experiments, metrics, findings."

echo "=== 4. Creating or Locating GitHub Project V2 ==="
PROJECT_TITLE="DiskGoblin Autonomous Factory"
PROJECT_NUM=$(gh project list --owner "$OWNER" --format json --jq ".projects[] | select(.title == \"$PROJECT_TITLE\") | .number" || true)

if [[ -z "$PROJECT_NUM" ]]; then
  PROJECT_NUM=$(gh project create --owner "$OWNER" --title "$PROJECT_TITLE" --format json --jq '.number')
  echo "Created GitHub Project #$PROJECT_NUM: $PROJECT_TITLE"
else
  echo "Found existing GitHub Project #$PROJECT_NUM: $PROJECT_TITLE"
fi

# Link project to repository
gh project link "$PROJECT_NUM" --owner "$OWNER" --repo "$REPO" || true

# Ensure custom fields exist on the Project
create_select_field() {
  local name="$1"
  local opts="$2"
  local exists
  exists=$(gh project field-list "$PROJECT_NUM" --owner "$OWNER" --format json --jq ".fields[] | select(.name == \"$name\") | .id" || true)
  if [[ -z "$exists" ]]; then
    gh project field-create "$PROJECT_NUM" --owner "$OWNER" --name "$name" --data-type "SINGLE_SELECT" --single-select-options "$opts" > /dev/null
    echo "Created project field: $name"
  fi
}

create_text_field() {
  local name="$1"
  local exists
  exists=$(gh project field-list "$PROJECT_NUM" --owner "$OWNER" --format json --jq ".fields[] | select(.name == \"$name\") | .id" || true)
  if [[ -z "$exists" ]]; then
    gh project field-create "$PROJECT_NUM" --owner "$OWNER" --name "$name" --data-type "TEXT" > /dev/null
    echo "Created project field: $name"
  fi
}

create_select_field "Task Type" "feature,bug,chore,research,experiment"
create_select_field "Priority" "P0,P1,P2"
create_select_field "Complexity" "S (30-45m),M (45-75m),L (75-120m)"
create_select_field "Risk" "Low,Medium,High"
create_select_field "Agent Harness" "OpenCode,Codex CLI,Human"
create_text_field "Dependencies"
create_text_field "Agent Model"

echo "=== 5. Creating Issues DG-001 .. DG-018 and Adding to Project ==="
create_backlog_issue() {
  local file="$1"
  local title="$2"
  local milestone="$3"
  local labels="$4"

  local existing_url
  existing_url=$(gh issue list --repo "$REPO" --search "\"$title\" in:title" --state all --json url,title --jq ".[] | select(.title == \"$title\") | .url" | head -n 1 || true)

  local issue_url
  if [[ -n "$existing_url" ]]; then
    echo "Issue already exists: $title ($existing_url)"
    issue_url="$existing_url"
  else
    issue_url=$(gh issue create \
      --repo "$REPO" \
      --title "$title" \
      --body-file "$file" \
      --milestone "$milestone" \
      --label "$labels")
    echo "Created issue: $title -> $issue_url"
  fi

  gh project item-add "$PROJECT_NUM" --owner "$OWNER" --url "$issue_url" > /dev/null
}

create_backlog_issue "docs/backlog/DG-001-workspace-and-ci.md" \
  "DG-001: Initialize Cargo workspace, crates, lint rules, and GitHub Actions CI" \
  "M0 — Foundation" "type:chore,priority:P0,risk:low,status:ready"

create_backlog_issue "docs/backlog/DG-002-core-domain-and-platform.md" \
  "DG-002: Implement core domain models, error types, and platform metadata adapter" \
  "M0 — Foundation" "type:feature,priority:P0,risk:low"

create_backlog_issue "docs/backlog/DG-003-synthetic-test-fixtures.md" \
  "DG-003: Build deterministic synthetic filesystem fixture generator for tests" \
  "M0 — Foundation" "type:feature,priority:P0,risk:low"

create_backlog_issue "docs/backlog/DG-004-byte-formatting.md" \
  "DG-004: Implement human-readable byte formatting (IEC binary & SI decimal)" \
  "M1 — CLI MVP" "type:feature,priority:P1,risk:low"

create_backlog_issue "docs/backlog/DG-005-sequential-scanner.md" \
  "DG-005: Implement sequential directory traversal with symlink, mount, and error handling" \
  "M1 — CLI MVP" "type:feature,priority:P0,risk:medium"

create_backlog_issue "docs/backlog/DG-006-tree-aggregation-and-hardlinks.md" \
  "DG-006: Implement bottom-up directory aggregation, hard-link deduplication, and top-N selection" \
  "M1 — CLI MVP" "type:feature,priority:P0,risk:medium"

create_backlog_issue "docs/backlog/DG-007-cancellation-and-progress.md" \
  "DG-007: Implement cooperative cancellation token and throttled progress reporting" \
  "M1 — CLI MVP" "type:feature,priority:P1,risk:low"

create_backlog_issue "docs/backlog/DG-008-json-serialization.md" \
  "DG-008: Implement versioned JSON output schema and deterministic serialization" \
  "M1 — CLI MVP" "type:feature,priority:P1,risk:low"

create_backlog_issue "docs/backlog/DG-009-cli-interface-and-renderer.md" \
  "DG-009: Implement diskgoblin-cli argument parser, terminal table renderer, and SIGINT handler" \
  "M1 — CLI MVP" "type:feature,priority:P0,risk:low"

create_backlog_issue "docs/backlog/DG-010-cli-integration-tests.md" \
  "DG-010: Add end-to-end CLI integration test suite and exit-code verification" \
  "M1 — CLI MVP" "type:chore,priority:P0,risk:low"

create_backlog_issue "docs/backlog/DG-011-criterion-benchmarks.md" \
  "DG-011: Create Criterion benchmark suite with synthetic directory topologies" \
  "M2 — Performance" "type:research,priority:P1,risk:low"

create_backlog_issue "docs/backlog/DG-012-parallel-scanner.md" \
  "DG-012: Implement parallel traversal engine and evaluate SSD thread scaling" \
  "M2 — Performance" "type:feature,priority:P1,risk:high"

create_backlog_issue "docs/backlog/DG-013-proptest-and-equivalence.md" \
  "DG-013: Add property-based equivalence tests (sequential vs. parallel) and memory regression checks" \
  "M2 — Performance" "type:chore,priority:P1,risk:medium"

create_backlog_issue "docs/backlog/DG-014-tauri-desktop-backend.md" \
  "DG-014: Scaffold Tauri 2 desktop app (diskgoblin-desktop) and Rust IPC commands" \
  "M3 — Desktop MVP" "type:feature,priority:P2,risk:medium"

create_backlog_issue "docs/backlog/DG-015-desktop-treemap-ui.md" \
  "DG-015: Implement React + d3-hierarchy interactive treemap and breadcrumb drill-down" \
  "M3 — Desktop MVP" "type:feature,priority:P2,risk:medium"

create_backlog_issue "docs/backlog/DG-016-desktop-tables-and-tests.md" \
  "DG-016: Add desktop top-items table, error drawer, and Vitest UI component tests" \
  "M3 — Desktop MVP" "type:feature,priority:P2,risk:low"

create_backlog_issue "docs/backlog/DG-017-factory-telemetry-tooling.md" \
  "DG-017: Implement software factory run-log schema, validator, and metrics summary tool" \
  "M4 — Factory Evaluation" "type:experiment,priority:P1,risk:low"

create_backlog_issue "docs/backlog/DG-018-factory-comparative-study.md" \
  "DG-018: Execute paired model/harness comparison study on benchmark tasks and publish findings" \
  "M4 — Factory Evaluation" "type:experiment,priority:P2,risk:medium"

echo "=== Done! GitHub Project #$PROJECT_NUM is populated with all 18 issues. ==="
