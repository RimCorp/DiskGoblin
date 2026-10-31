#!/usr/bin/env bash
set -euo pipefail

# Verifies that a backlog issue (e.g. DG-003) has all upstream dependencies
# closed/merged on GitHub before dispatching an autonomous coding agent.

REPO="RimCorp/DiskGoblin"
ISSUE_ID="${1:-}"

if [[ -z "$ISSUE_ID" ]]; then
  echo "Usage: $0 <DG-XXX>" >&2
  exit 2
fi

SPEC_FILE=$(find docs/backlog -maxdepth 1 -name "${ISSUE_ID}-*.md" | head -n 1)
if [[ -z "$SPEC_FILE" || ! -f "$SPEC_FILE" ]]; then
  echo "ERROR: Could not find backlog spec for $ISSUE_ID in docs/backlog/" >&2
  exit 1
fi

echo "Checking dispatch readiness for $ISSUE_ID ($SPEC_FILE)..."

# Extract DG-XXX identifiers from the ## Dependencies section
DEPS=$(awk '
  /^## Dependencies/ { in_deps=1; next }
  /^## / { if (in_deps) exit }
  in_deps { print }
' "$SPEC_FILE" | grep -oE 'DG-[0-9]{3}' | sort -u || true)

if [[ -z "$DEPS" ]]; then
  echo "✓ $ISSUE_ID has no upstream dependencies. READY FOR DISPATCH."
  exit 0
fi

BLOCKED=0
for dep in $DEPS; do
  STATE=$(gh issue list --repo "$REPO" --search "\"$dep:\" in:title" --state all --json title,state --jq ".[] | select(.title | startswith(\"$dep:\")) | .state" | head -n 1 || true)
  if [[ "$STATE" != "CLOSED" ]]; then
    echo "✗ Dependency $dep is not CLOSED (current state: ${STATE:-NOT_FOUND})."
    BLOCKED=1
  else
    echo "✓ Dependency $dep is CLOSED."
  fi
done

if [[ "$BLOCKED" -ne 0 ]]; then
  echo "BLOCKED: $ISSUE_ID cannot be dispatched until all dependencies are CLOSED and merged to main." >&2
  exit 1
fi

echo "✓ All dependencies for $ISSUE_ID are satisfied. READY FOR DISPATCH."
