# AGENTS.md — DiskGoblin Coding-Agent Execution Contract

This document defines the mandatory operating contract for all autonomous coding agents (**OpenCode**, **Codex CLI**, and peer agents) working in the **DiskGoblin** repository.

> **Authoritative State Rule:** The assigned GitHub Issue (or `docs/backlog/DG-XXX-*.md`), `docs/architecture.md`, and the Git repository state are the sole authoritative sources of truth. Never rely on conversational memory across tasks or invent unapproved product behavior.

---

## 1. Mandatory Pre-Flight Checklist

Before editing any file, you **MUST**:
1. Read your assigned issue specification in full (`Objective`, `Scope`, `Out of scope`, `Acceptance criteria`, `Verification`).
2. Read [`docs/architecture.md`](docs/architecture.md) and any ADRs in [`docs/decisions/`](docs/decisions/) referenced by the issue.
3. Confirm you are operating inside an isolated branch or Git worktree (`feat/DG-XXX-<slug>`, `chore/DG-XXX-<slug>`, or `fix/DG-XXX-<slug>`) branched from the latest `main`. Never commit directly to `main`.

---

## 2. Architectural & Scope Rules

1. **Strict Scope Containment:**
   - Only create or modify files permitted by the assigned issue's `## Scope` section.
   - Do not refactor unrelated modules, reformat untouched files, or implement downstream features listed under `## Out of scope`.
2. **Crate Boundary Enforcement (`ADR-001`):**
   - `crates/diskgoblin-core` is a pure library crate. It must **never** depend on `clap`, read environment CLI args, or print directly to `stdout`/`stderr` (`println!`, `eprintln!`).
   - `crates/diskgoblin-cli` is a thin binary wrapper around `diskgoblin-core`.
3. **Safety by Default (`ADR-003`):**
   - DiskGoblin is strictly read-only. Never invoke `std::fs::remove_file`, `std::fs::remove_dir_all`, or file truncation on scanned user paths. (Removing temporary test fixtures managed by `tempfile::TempDir` inside tests is permitted.)
   - Always use `std::fs::symlink_metadata` (`lstat`) during traversal; never follow symbolic links into recursive loops.
4. **Dependency Discipline:**
   - **Never** add a new crate to `Cargo.toml` (or package to `package.json`) unless explicitly authorized by the issue specification.
   - If you believe a new dependency is essential, document its purpose and escalate rather than silently adding it.
5. **Error Handling & Panic Hygiene:**
   - Do not use `.unwrap()` or `.expect()` in production library code (`crates/diskgoblin-core/src/**`) unless mathematically proven infallible and accompanied by a comment explaining why. Prefer typed errors (`thiserror`) or graceful `ScanErrorRecord` collection.

---

## 3. Testing & Verification Requirements

Every task must be deterministically verified before completion:
1. **No Host Filesystem Assumptions:** All filesystem tests must use synthetic temporary directories (`tempfile::TempDir` / `FixtureBuilder`) and clean up permissions (`chmod 700`) before drop so cleanup never leaks.
2. **Mandatory Quality Gate Commands:**
   Run all three commands locally and ensure zero errors or warnings:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace --all-targets
   ```
3. **Bounded Repair Loops (Max 3 Attempts):**
   If compilation, Clippy, or tests fail, diagnose the root cause and fix it. If you cannot achieve a green build after **3 repair iterations**, stop, commit your branch, and post a `BLOCKER REPORT` detailing the failure.

---

## 4. Escalation Protocol

Stop and escalate (`status:blocked`) instead of guessing when:
- Requirements in the issue conflict with `docs/architecture.md` or existing public trait/struct signatures.
- Completing the issue requires modifying files explicitly listed in `## Out of scope`.
- A required upstream dependency issue has not been merged to `main`.

---

## 5. Pull Request & Completion Report Contract

When all acceptance criteria and verification commands pass:
1. Stage and commit your changes with a Conventional Commit message referencing the issue ID (e.g., `feat(core): implement sequential directory scanner (DG-005)`).
2. When authorized to open a Pull Request (`gh pr create`), link the issue and include the following structured summary in your PR body / completion report:

```markdown
## Agent Implementation Report — DG-XXX

### Summary of Changes
- Concise bullet list of modules and types implemented or modified.

### Verification Results
- `cargo fmt --all -- --check`: PASS
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS
- `cargo test --workspace --all-targets`: PASS (X unit, Y integration tests)
- Issue-specific verification commands: PASS

### Scope & Dependency Check
- Out-of-scope files modified: None
- New dependencies added: None (or list with justification per spec)

### Unresolved Risks or Follow-ups
- Explicit note of any edge cases or follow-up considerations for downstream issues.
```

3. **Never merge your own Pull Request.** All PRs require automated CI verification and independent review approval before merge.
