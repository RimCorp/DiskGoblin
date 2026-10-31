# DG-004: Implement human-readable byte formatting (IEC binary & SI decimal)

- **Milestone:** M1 — CLI MVP
- **Type:** feature
- **Priority:** P1
- **Complexity Estimate:** S (30–45 minutes)
- **Risk:** Low

## Objective
Implement deterministic, dependency-free byte-size formatting and percentage bar helpers in `diskgoblin-core::format` supporting both binary IEC units (`B`, `KiB`, `MiB`, `GiB`, `TiB`, `PiB`) and decimal SI units (`B`, `KB`, `MB`, `GB`, `TB`, `PB`).

## Context
Per [`docs/product-spec.md` §3.3](../product-spec.md) and [`docs/architecture.md` §3](../architecture.md), both the CLI table renderer (`DG-009`) and the desktop UI (`M3`) need consistent human-readable size formatting (`12.4 GiB`, `840.0 KiB`, `0 B`) and relative usage percentage calculations.

## Scope
- `crates/diskgoblin-core/src/format.rs`:
  - `UnitSystem` enum (`IecBinary` base 1024, `SiDecimal` base 1000).
  - `format_bytes(bytes: u64, system: UnitSystem) -> String`:
    - Exact bytes `< base` formatted as integers (e.g., `"0 B"`, `"512 B"`, `"1023 B"`).
    - Scaled units (`KiB`..`PiB` or `KB`..`PB`) formatted with 1 decimal place (e.g., `"1.0 KiB"`, `"12.5 MiB"`).
  - `format_percentage(part: u64, total: u64) -> String` (e.g., `"45.2%"`, or `"0.0%"` when `total == 0`).
  - `render_ascii_bar(part: u64, total: u64, width: usize) -> String` (e.g., `"████████░░░░"` for terminal progress/usage columns).
- Re-export in `crates/diskgoblin-core/src/lib.rs`.

## Out of scope
- Do not add external formatting crates (`humansize` or `byte-unit` are unnecessary; this is ~60 lines of pure, well-tested Rust).
- Do not implement JSON serialization (`DG-008`) or full CLI table rendering (`DG-009`).

## Dependencies
- `DG-002` (Core domain module layout).

## Implementation guidance
- Handle boundary cases explicitly: `0`, `999`, `1000`, `1023`, `1024`, `1024 * 1024 - 1`, `u64::MAX`.
- Ensure `format_percentage(part, 0)` never divides by zero and clamps `part > total` to `100.0%`.

## Acceptance criteria
- [ ] `format_bytes(0, UnitSystem::IecBinary)` returns `"0 B"`.
- [ ] `format_bytes(1024, UnitSystem::IecBinary)` returns `"1.0 KiB"`.
- [ ] `format_bytes(1000, UnitSystem::SiDecimal)` returns `"1.0 KB"`.
- [ ] `format_bytes(u64::MAX, UnitSystem::IecBinary)` formats cleanly in `EiB` or `PiB` without overflow or panic.
- [ ] `format_percentage` and `render_ascii_bar` handle `total == 0` and `part > total` safely without panicking.

## Verification
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p diskgoblin-core format
```

## Definition of done
- Unit tests cover all unit boundaries (`B` through `PiB`/`PB`), `u64::MAX`, and zero-total percentages.
- PR opened linking `DG-004` with Agent Implementation Report.
