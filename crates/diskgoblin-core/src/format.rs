//! Deterministic human-readable formatting helpers and the versioned JSON
//! report serializer.
//!
//! All human-readable arithmetic is performed with integer types so that output
//! is stable across platforms and no floating-point rounding surprises can
//! occur. JSON output preserves struct field order and uses `snake_case` names,
//! making reports byte-for-byte deterministic for a given [`ScanReport`].

use crate::model::ScanReport;

/// Version of the machine-readable JSON report schema.
///
/// Bump this whenever the JSON shape changes in a backwards-incompatible way so
/// that consumers (CLI scripts, benchmark harnesses, and the desktop app) can
/// reject or migrate reports they do not understand.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

/// Serializes a scan report to pretty-printed, deterministic JSON.
///
/// Field names are `snake_case` and fields are emitted in declaration order.
/// The returned string is valid UTF-8 and always ends without a trailing
/// newline.
pub fn report_to_json_pretty(report: &ScanReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

/// Writes a scan report as compact, deterministic JSON to `writer`.
///
/// This is the streaming counterpart of [`report_to_json_pretty`] and is suited
/// to writing directly to `stdout` without buffering the entire document in
/// memory. The caller is responsible for flushing `writer`.
pub fn report_to_json_writer<W: std::io::Write>(
    report: &ScanReport,
    writer: W,
) -> Result<(), serde_json::Error> {
    serde_json::to_writer(writer, report)
}

/// Unit system used by [`format_bytes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitSystem {
    /// Binary IEC units (base 1024): `B`, `KiB`, `MiB`, `GiB`, `TiB`, `PiB`, `EiB`.
    IecBinary,
    /// Decimal SI units (base 1000): `B`, `KB`, `MB`, `GB`, `TB`, `PB`, `EB`.
    SiDecimal,
}

impl UnitSystem {
    /// The multiplicative step between consecutive units.
    pub const fn base(self) -> u64 {
        match self {
            Self::IecBinary => 1024,
            Self::SiDecimal => 1000,
        }
    }

    /// Unit labels ordered from bytes to the largest supported unit.
    pub const fn units(self) -> &'static [&'static str] {
        match self {
            Self::IecBinary => &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"],
            Self::SiDecimal => &["B", "KB", "MB", "GB", "TB", "PB", "EB"],
        }
    }
}

/// Formats a byte count using one decimal place for scaled units.
///
/// Counts below the unit system's base are rendered as exact integers, e.g.
/// `"0 B"`, `"512 B"`, `"1023 B"`. Larger counts are rendered with one decimal
/// place, e.g. `"1.0 KiB"`, `"12.5 MiB"`.
///
/// Rounding is half-up and may promote to the next unit, so
/// `format_bytes(1024 * 1024 - 1, UnitSystem::IecBinary)` returns `"1.0 MiB"`.
pub fn format_bytes(bytes: u64, system: UnitSystem) -> String {
    let base = u128::from(system.base());
    let units = system.units();
    let max_index = units.len() - 1;

    if u128::from(bytes) < base {
        return format!("{bytes} B");
    }

    // Find the largest unit whose divisor is <= bytes. `divisor` starts at 1
    // (plain bytes) and is multiplied by `base` at each step.
    let mut index = 0;
    let mut divisor: u128 = 1;
    while index < max_index && u128::from(bytes) >= divisor * base {
        divisor *= base;
        index += 1;
    }

    // Work in tenths of the selected unit, rounding half-up.
    let mut tenths = scaled_tenths(bytes, divisor);

    // A rounded value such as 1024.0 KiB must be shown as 1.0 MiB.
    while tenths >= base * 10 && index < max_index {
        divisor *= base;
        index += 1;
        tenths = scaled_tenths(bytes, divisor);
    }

    format!("{}.{} {}", tenths / 10, tenths % 10, units[index])
}

/// Returns `bytes` in tenths of `divisor`, rounded half-up.
fn scaled_tenths(bytes: u64, divisor: u128) -> u128 {
    (u128::from(bytes) * 10 + divisor / 2) / divisor
}

/// Formats a relative usage percentage with one decimal place.
///
/// A zero `total` yields `"0.0%"`, and `part > total` is clamped to
/// `"100.0%"`.
pub fn format_percentage(part: u64, total: u64) -> String {
    if total == 0 {
        return "0.0%".to_string();
    }

    let total = u128::from(total);
    let tenths = (u128::from(part) * 1000 + total / 2) / total;
    let tenths = tenths.min(1000);

    format!("{}.{}%", tenths / 10, tenths % 10)
}

/// Renders a fixed-width ASCII/Unicode usage bar using `█` and `░`.
///
/// The number of filled cells is proportional to `part / total`, rounded
/// half-up and clamped to `width`. A zero `total` produces an empty bar and
/// `part > total` produces a full bar. A zero `width` produces an empty string.
pub fn render_ascii_bar(part: u64, total: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let filled = if total == 0 {
        0
    } else {
        let scaled = (u128::from(part) * width as u128 + u128::from(total) / 2) / u128::from(total);
        scaled.min(width as u128) as usize
    };

    let mut bar = String::with_capacity(width.saturating_mul(3));
    for cell in 0..width {
        bar.push(if cell < filled { '█' } else { '░' });
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_exact_bytes_below_base() {
        assert_eq!(format_bytes(0, UnitSystem::IecBinary), "0 B");
        assert_eq!(format_bytes(1, UnitSystem::IecBinary), "1 B");
        assert_eq!(format_bytes(512, UnitSystem::IecBinary), "512 B");
        assert_eq!(format_bytes(1023, UnitSystem::IecBinary), "1023 B");
        assert_eq!(format_bytes(999, UnitSystem::SiDecimal), "999 B");
    }

    #[test]
    fn formats_iec_unit_boundaries() {
        assert_eq!(format_bytes(1024, UnitSystem::IecBinary), "1.0 KiB");
        assert_eq!(format_bytes(1024 * 1024, UnitSystem::IecBinary), "1.0 MiB");
        assert_eq!(
            format_bytes(1024 * 1024 * 1024, UnitSystem::IecBinary),
            "1.0 GiB"
        );
        assert_eq!(
            format_bytes(1024_u64.pow(4), UnitSystem::IecBinary),
            "1.0 TiB"
        );
        assert_eq!(
            format_bytes(1024_u64.pow(5), UnitSystem::IecBinary),
            "1.0 PiB"
        );
        assert_eq!(
            format_bytes(1024_u64.pow(6), UnitSystem::IecBinary),
            "1.0 EiB"
        );
    }

    #[test]
    fn formats_si_unit_boundaries() {
        assert_eq!(format_bytes(1000, UnitSystem::SiDecimal), "1.0 KB");
        assert_eq!(
            format_bytes(1000_u64.pow(2), UnitSystem::SiDecimal),
            "1.0 MB"
        );
        assert_eq!(
            format_bytes(1000_u64.pow(3), UnitSystem::SiDecimal),
            "1.0 GB"
        );
        assert_eq!(
            format_bytes(1000_u64.pow(4), UnitSystem::SiDecimal),
            "1.0 TB"
        );
        assert_eq!(
            format_bytes(1000_u64.pow(5), UnitSystem::SiDecimal),
            "1.0 PB"
        );
        assert_eq!(
            format_bytes(1000_u64.pow(6), UnitSystem::SiDecimal),
            "1.0 EB"
        );
    }

    #[test]
    fn formats_fractional_units() {
        assert_eq!(format_bytes(12_582_912, UnitSystem::IecBinary), "12.0 MiB");
        assert_eq!(format_bytes(13_107_200, UnitSystem::IecBinary), "12.5 MiB");
        assert_eq!(format_bytes(860_160, UnitSystem::IecBinary), "840.0 KiB");
        assert_eq!(format_bytes(1_500, UnitSystem::SiDecimal), "1.5 KB");
    }

    #[test]
    fn rounds_and_promotes_at_unit_ceiling() {
        // Just below one MiB, but rounds to 1024.0 KiB, which must promote.
        assert_eq!(
            format_bytes(1024 * 1024 - 1, UnitSystem::IecBinary),
            "1.0 MiB"
        );
        assert_eq!(format_bytes(999_950, UnitSystem::SiDecimal), "1.0 MB");
    }

    #[test]
    fn formats_u64_max_without_overflow() {
        assert_eq!(format_bytes(u64::MAX, UnitSystem::IecBinary), "16.0 EiB");
        assert_eq!(format_bytes(u64::MAX, UnitSystem::SiDecimal), "18.4 EB");
    }

    #[test]
    fn formats_percentage() {
        assert_eq!(format_percentage(0, 1000), "0.0%");
        assert_eq!(format_percentage(452, 1000), "45.2%");
        assert_eq!(format_percentage(1, 3), "33.3%");
        assert_eq!(format_percentage(2, 3), "66.7%");
        assert_eq!(format_percentage(999, 1000), "99.9%");
        assert_eq!(format_percentage(1000, 1000), "100.0%");
    }

    #[test]
    fn percentage_handles_zero_and_out_of_range_totals() {
        assert_eq!(format_percentage(0, 0), "0.0%");
        assert_eq!(format_percentage(42, 0), "0.0%");
        assert_eq!(format_percentage(101, 100), "100.0%");
        assert_eq!(format_percentage(u64::MAX, 1), "100.0%");
    }

    #[test]
    fn renders_ascii_bar() {
        assert_eq!(render_ascii_bar(8, 12, 12), "████████░░░░",);
        assert_eq!(render_ascii_bar(0, 12, 4), "░░░░");
        assert_eq!(render_ascii_bar(12, 12, 4), "████");
        assert_eq!(render_ascii_bar(1, 2, 10), "█████░░░░░");
        assert_eq!(render_ascii_bar(1, 3, 3), "█░░");
    }

    #[test]
    fn ascii_bar_handles_zero_and_out_of_range_inputs() {
        assert_eq!(render_ascii_bar(0, 0, 4), "░░░░");
        assert_eq!(render_ascii_bar(7, 0, 4), "░░░░");
        assert_eq!(render_ascii_bar(11, 10, 4), "████");
        assert_eq!(render_ascii_bar(u64::MAX, 1, 4), "████");
        assert_eq!(render_ascii_bar(1, 1, 0), "");
        assert_eq!(render_ascii_bar(0, 0, 0), "");
    }
}
