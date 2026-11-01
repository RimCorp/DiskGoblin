//! Integration tests for the versioned JSON report schema (DG-008).

use std::path::PathBuf;

use diskgoblin_core::{
    report_to_json_pretty, report_to_json_writer, scan_directory, test_fixtures::FixtureBuilder,
    DirNode, DirSummary, FileEntry, ScanErrorKind, ScanErrorRecord, ScanOptions, ScanReport,
    SizeMetric, REPORT_SCHEMA_VERSION,
};
use serde_json::Value;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn summary(path: &str, depth: usize) -> DirSummary {
    DirSummary {
        path: PathBuf::from(path),
        depth,
        apparent_size: 4096,
        allocated_size: 8192,
        file_count: 1,
        dir_count: 1,
        symlink_count: 2,
        error_count: 3,
    }
}

fn sample_report() -> ScanReport {
    ScanReport {
        schema_version: REPORT_SCHEMA_VERSION,
        root_path: PathBuf::from("/fixture/root"),
        size_metric: SizeMetric::Allocated,
        duration_ms: 42,
        root: DirNode {
            summary: summary("/fixture/root", 0),
            children: vec![DirNode {
                summary: summary("/fixture/root/build", 1),
                children: Vec::new(),
            }],
        },
        top_directories: vec![summary("/fixture/root/build", 1)],
        top_files: vec![FileEntry {
            path: PathBuf::from("/fixture/root/build/artifact.bin"),
            apparent_size: 4096,
            allocated_size: 8192,
            is_deduplicated_hardlink: false,
        }],
        errors: vec![
            ScanErrorRecord {
                path: PathBuf::from("/fixture/root/private"),
                kind: ScanErrorKind::PermissionDenied,
                message: "Permission denied (os error 13)".to_string(),
            },
            ScanErrorRecord {
                path: PathBuf::from("/fixture/root/vanished"),
                kind: ScanErrorKind::NotFound,
                message: "No such file or directory (os error 2)".to_string(),
            },
        ],
    }
}

fn object_keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .expect("expected a JSON object")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn pretty_json_has_stable_versioned_structure() -> Result {
    let report = sample_report();
    let json = report_to_json_pretty(&report)?;
    assert!(!json.ends_with('\n'));
    // Serialization is deterministic: the same report always yields the same bytes.
    assert_eq!(json, report_to_json_pretty(&report)?);

    let value: Value = serde_json::from_str(&json)?;

    assert_eq!(
        object_keys(&value),
        [
            "duration_ms",
            "errors",
            "root",
            "root_path",
            "schema_version",
            "size_metric",
            "top_directories",
            "top_files",
        ]
    );
    assert_eq!(value["schema_version"].as_u64(), Some(1));
    assert_eq!(value["schema_version"], REPORT_SCHEMA_VERSION);
    assert_eq!(value["size_metric"].as_str(), Some("allocated"));
    assert_eq!(value["duration_ms"].as_u64(), Some(42));
    assert_eq!(
        value["root_path"].as_str(),
        Some(report.root_path.to_string_lossy().as_ref())
    );

    let root = &value["root"];
    assert_eq!(object_keys(root), ["children", "summary"]);
    assert_eq!(
        object_keys(&root["summary"]),
        [
            "allocated_size",
            "apparent_size",
            "depth",
            "dir_count",
            "error_count",
            "file_count",
            "path",
            "symlink_count",
        ]
    );
    assert_eq!(
        root["summary"]["path"].as_str(),
        Some(report.root.summary.path.to_string_lossy().as_ref())
    );
    assert_eq!(root["summary"]["depth"].as_u64(), Some(0));
    assert_eq!(root["summary"]["apparent_size"].as_u64(), Some(4096));
    assert_eq!(root["summary"]["allocated_size"].as_u64(), Some(8192));
    assert_eq!(root["summary"]["file_count"].as_u64(), Some(1));
    assert_eq!(root["summary"]["dir_count"].as_u64(), Some(1));
    assert_eq!(root["summary"]["symlink_count"].as_u64(), Some(2));
    assert_eq!(root["summary"]["error_count"].as_u64(), Some(3));

    let children = root["children"].as_array().expect("children is an array");
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0]["summary"]["path"].as_str(),
        Some("/fixture/root/build")
    );
    assert!(children[0]["children"]
        .as_array()
        .is_some_and(Vec::is_empty));

    let top_directories = value["top_directories"]
        .as_array()
        .expect("top_directories is an array");
    assert_eq!(top_directories.len(), 1);
    assert_eq!(
        object_keys(&top_directories[0]),
        [
            "allocated_size",
            "apparent_size",
            "depth",
            "dir_count",
            "error_count",
            "file_count",
            "path",
            "symlink_count",
        ]
    );

    let top_files = value["top_files"]
        .as_array()
        .expect("top_files is an array");
    assert_eq!(top_files.len(), 1);
    assert_eq!(
        object_keys(&top_files[0]),
        [
            "allocated_size",
            "apparent_size",
            "is_deduplicated_hardlink",
            "path",
        ]
    );
    assert_eq!(
        top_files[0]["is_deduplicated_hardlink"].as_bool(),
        Some(false)
    );
    assert_eq!(top_files[0]["apparent_size"].as_u64(), Some(4096));
    assert_eq!(top_files[0]["allocated_size"].as_u64(), Some(8192));

    let errors = value["errors"].as_array().expect("errors is an array");
    assert_eq!(errors.len(), 2);
    assert_eq!(object_keys(&errors[0]), ["kind", "message", "path"]);
    assert_eq!(errors[0]["kind"].as_str(), Some("permission_denied"));
    assert_eq!(errors[1]["kind"].as_str(), Some("not_found"));
    assert_eq!(
        errors[0]["message"].as_str(),
        Some("Permission denied (os error 13)")
    );

    Ok(())
}

#[test]
fn enum_values_serialize_as_snake_case_strings() -> Result {
    let json = report_to_json_pretty(&sample_report())?;
    let value: Value = serde_json::from_str(&json)?;

    assert_eq!(value["size_metric"], "allocated");
    assert_eq!(value["errors"][0]["kind"], "permission_denied");

    let mut apparent = sample_report();
    apparent.size_metric = SizeMetric::Apparent;
    let value: Value = serde_json::from_str(&report_to_json_pretty(&apparent)?)?;
    assert_eq!(value["size_metric"], "apparent");

    Ok(())
}

#[test]
fn pretty_and_writer_output_round_trip() -> Result {
    let report = sample_report();

    let pretty = report_to_json_pretty(&report)?;
    let decoded: ScanReport = serde_json::from_str(&pretty)?;
    assert_eq!(report, decoded);

    let mut buffer = Vec::new();
    report_to_json_writer(&report, &mut buffer)?;
    let compact = String::from_utf8(buffer)?;
    // The writer emits the compact form with no formatting whitespace.
    assert!(!compact.contains('\n'));
    assert!(!compact.contains("  "));

    let decoded: ScanReport = serde_json::from_str(&compact)?;
    assert_eq!(report, decoded);

    // Both representations describe exactly the same document.
    let pretty_value: Value = serde_json::from_str(&pretty)?;
    let compact_value: Value = serde_json::from_str(&compact)?;
    assert_eq!(pretty_value, compact_value);

    Ok(())
}

#[test]
fn scanned_reports_are_versioned_and_round_trip() -> Result {
    let fixture = FixtureBuilder::new()
        .add_file("a/data.bin", 128)
        .add_file("b/data.bin", 64)
        .build()?;
    let options = ScanOptions {
        root_path: fixture.path().into(),
        size_metric: SizeMetric::Allocated,
        ..ScanOptions::default()
    };

    let report = scan_directory(&options)?;
    assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
    assert_eq!(report.schema_version, 1);

    let json = report_to_json_pretty(&report)?;
    let value: Value = serde_json::from_str(&json)?;
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["size_metric"], "allocated");

    let decoded: ScanReport = serde_json::from_str(&json)?;
    assert_eq!(report, decoded);

    Ok(())
}
