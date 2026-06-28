//! Data-driven workspace CONFORMANCE runner.
//!
//! Mirrors the Python (`test_workspace.py::TestWorkspace`) and TS
//! (`test-workspace.ts`) 5-aspect data-driven suite over the shared QMD-5
//! fixtures, so the `workspace` row of the unified report reaches parity by
//! construction (same fixtures × same 5 aspects in every language). Each fixture
//! contributes exactly 5 cases: workspace_id, files, objects_by_kind, errors
//! (skipped-but-counted when the fixture declares none), objects_have_metadata.
//!
//! Drives the real `qmdc workspace parse` CLI (same source the other workspace
//! tests use). Impl-specific workspace tests live in `workspace.rs` (→ unit-rs).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Once;

mod common;

fn get_qmdc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/qmdc")
}

fn get_workspace_tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/workspace")
}

static BUILD_ONCE: Once = Once::new();

fn ensure_qmdc_built() {
    BUILD_ONCE.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "--bin", "qmdc"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("Failed to build qmdc");
        assert!(status.success(), "Failed to build qmdc binary");
    });
}

fn find_workspace_tests() -> Vec<(String, PathBuf, serde_json::Value)> {
    let root = get_workspace_tests_dir();
    let mut tests = Vec::new();

    fn scan(dir: &PathBuf, prefix: &str, out: &mut Vec<(String, PathBuf, serde_json::Value)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut dirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for path in dirs {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let expected_file = path.join("_expected.json");
            let readme_file = path.join("readme.qmd.md");
            if expected_file.exists() && readme_file.exists() {
                let test_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}{}", prefix, name)
                };
                if let Ok(content) = fs::read_to_string(&expected_file) {
                    if let Ok(expected) = serde_json::from_str(&content) {
                        out.push((test_name, path.clone(), expected));
                    }
                }
            } else {
                let new_prefix = if prefix.is_empty() {
                    format!("{}/", name)
                } else {
                    format!("{}{}/", prefix, name)
                };
                scan(&path, &new_prefix, out);
            }
        }
    }

    if root.exists() {
        scan(&root, "", &mut tests);
    }
    tests
}

fn parse_workspace_json(path: &std::path::Path) -> serde_json::Value {
    let output = Command::new(get_qmdc_path())
        .args(["workspace", "parse", path.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run qmdc workspace parse");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("Failed to parse JSON for {}: {}", path.display(), e))
}

fn sorted_strings(v: Option<&serde_json::Value>) -> Vec<String> {
    let mut out: Vec<String> = v
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Group object ids by __kind, sorted — mirrors the Python `objects_by_kind`.
fn objects_by_kind(result: &serde_json::Value) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(objects) = result.get("objects").and_then(|v| v.as_array()) {
        for obj in objects {
            let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
            let id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
            // Match Python: group every object (incl. empty kind/id) so a
            // malformed result fails identically across parsers.
            map.entry(kind.to_string())
                .or_default()
                .push(id.to_string());
        }
    }
    for ids in map.values_mut() {
        ids.sort();
    }
    map
}

fn expected_by_kind(expected: &serde_json::Value) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(objs) = expected.get("objects").and_then(|v| v.as_object()) {
        for (kind, ids) in objs {
            let mut v = sorted_strings(Some(ids));
            v.sort();
            map.insert(kind.clone(), v);
        }
    }
    map
}

/// Normalize one error to the comparable subset (type/object/reference/file/line/
/// candidates), dropping empty fields — mirrors the Python error construction.
fn norm_error(e: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    let mut m = BTreeMap::new();
    for key in ["type", "object", "reference", "file", "line", "candidates"] {
        if let Some(v) = e.get(key) {
            let empty = v.is_null()
                || v.as_str() == Some("")
                || v.as_array().map(|a| a.is_empty()).unwrap_or(false);
            if !empty {
                m.insert(key.to_string(), v.clone());
            }
        }
    }
    m
}

fn error_sort_key(e: &BTreeMap<String, serde_json::Value>) -> String {
    serde_json::to_string(e).unwrap_or_default()
}

fn norm_errors(arr: Option<&serde_json::Value>) -> Vec<BTreeMap<String, serde_json::Value>> {
    let mut out: Vec<_> = arr
        .and_then(|v| v.as_array())
        .map(|a| a.iter().map(norm_error).collect())
        .unwrap_or_default();
    out.sort_by_key(error_sort_key);
    out
}

#[test]
fn test_workspace_conformance() {
    ensure_qmdc_built();
    let tests = find_workspace_tests();
    let mut report = common::CaseReport::new("workspace", "rs-workspace");

    for (name, path, expected) in tests {
        let parse_t = std::time::Instant::now();
        let result = parse_workspace_json(&path);
        let parse_secs = parse_t.elapsed().as_secs_f64();

        // 1. workspace_id (the per-fixture parse cost is attributed to this case)
        let t = std::time::Instant::now();
        let case = format!("{}/workspace_id", name);
        let actual_id = result
            .get("workspace")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let expected_id = expected
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let secs = parse_secs + t.elapsed().as_secs_f64();
        if actual_id == expected_id {
            report.pass(&case, secs);
        } else {
            report.fail(
                &case,
                &format!("expected '{}', got '{}'", expected_id, actual_id),
                secs,
            );
        }

        // 2. files
        let t = std::time::Instant::now();
        let case = format!("{}/files", name);
        let ok = sorted_strings(result.get("files")) == sorted_strings(expected.get("files"));
        let secs = t.elapsed().as_secs_f64();
        if ok {
            report.pass(&case, secs);
        } else {
            report.fail(&case, "files mismatch", secs);
        }

        // 3. objects_by_kind
        let t = std::time::Instant::now();
        let case = format!("{}/objects_by_kind", name);
        let ok = objects_by_kind(&result) == expected_by_kind(&expected);
        let secs = t.elapsed().as_secs_f64();
        if ok {
            report.pass(&case, secs);
        } else {
            report.fail(&case, "objects_by_kind mismatch", secs);
        }

        // 4. errors (skipped-but-counted when the fixture declares none; null == absent)
        let t = std::time::Instant::now();
        let case = format!("{}/errors", name);
        if expected.get("errors").is_none_or(|v| v.is_null()) {
            report.skip(&case, t.elapsed().as_secs_f64());
        } else {
            let ok = norm_errors(result.get("errors")) == norm_errors(expected.get("errors"));
            let secs = t.elapsed().as_secs_f64();
            if ok {
                report.pass(&case, secs);
            } else {
                report.fail(&case, "errors mismatch", secs);
            }
        }

        // 5. objects_have_metadata
        let t = std::time::Instant::now();
        let case = format!("{}/objects_have_metadata", name);
        let mut ok = true;
        if let Some(objects) = result.get("objects").and_then(|v| v.as_array()) {
            for obj in objects {
                let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
                if kind.starts_with("__") && kind != "__Workspace" && kind != "__Namespace" {
                    continue;
                }
                if obj.get("__file").is_none() || obj.get("__line").is_none() {
                    ok = false;
                    break;
                }
            }
        }
        let secs = t.elapsed().as_secs_f64();
        if ok {
            report.pass(&case, secs);
        } else {
            report.fail(&case, "object missing __file/__line", secs);
        }
    }

    report.finish();
}
