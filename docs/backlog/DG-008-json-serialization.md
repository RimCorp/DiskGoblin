# DG-008: Implement versioned JSON output schema and deterministic serialization

- **Milestone:** M1 — CLI MVP
- **Type:** feature
- **Priority:** P1
- **Complexity Estimate:** S (30–60 minutes)
- **Risk:** Low

## Objective
Implement deterministic JSON serialization (`to_json_pretty` and `to_json_compact`) for `ScanReport` with `schema_version: 1`, ensuring stable field naming (`snake_case`) and round-trip deserialization for CLI scripting and automated benchmark/factory verification.

## Context
Per [`docs/product-spec.md` §3.3](../product-spec.md) and [`docs/architecture.md` §4.5](../architecture.md), `--json` output is a primary interface for both CLI automation and the software factory evaluation harness (`M4`).

## Scope
- `crates/diskgoblin-core/Cargo.toml`: Add `serde_json = "1"`.
- `crates/diskgoblin-core/src/format.rs` (or `src/json.rs`):
  - `pub const REPORT_SCHEMA_VERSION: u32 = 1;`
  - `pub fn report_to_json_pretty(report: &ScanReport) -> Result<String, serde_json::Error>`
  - `pub fn report_to_json_writer<W: std::io::Write>(report: &ScanReport, writer: W) -> Result<(), serde_json::Error>`
  - Ensure all `ScanReport`, `DirNode`, `DirSummary`, `FileEntry`, `ScanErrorRecord`, `ScanErrorKind`, and `SizeMetric` fields serialize cleanly in `snake_case`.
- Integration test in `crates/diskgoblin-core/tests/json_schema_test.rs` verifying exact JSON structure and round-trip `serde_json::from_str::<ScanReport>`.

## Out of scope
- Do not implement CLI flag handling (`DG-009`).

## Dependencies
- `DG-006` (`ScanReport` construction).

## Implementation guidance
- Verify that `ScanReport::schema_version` is always initialized to `1`.
- Write a golden/structural JSON assertion test parsing the output into `serde_json::Value` and asserting the presence and types of `schema_version`, `root_path`, `size_metric`, `duration_ms`, `root.summary`, `root.children`, `top_directories`, `top_files`, and `errors`.

## Acceptance criteria
- [ ] `ScanReport` serializes to valid JSON with `"schema_version": 1`.
- [ ] Enum values (`SizeMetric::Allocated`, `ScanErrorKind::PermissionDenied`) serialize as lowercase `snake_case` strings (`"allocated"`, `"permission_denied"`).
- [ ] A serialized `ScanReport` deserializes back via `serde_json::from_str::<ScanReport>` with full equality (`assert_eq!(original, decoded)`).

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-core --test json_schema_test
```

## Definition of done
- JSON schema tests pass with zero warnings.
- PR opened linking `DG-008` with Agent Implementation Report.
