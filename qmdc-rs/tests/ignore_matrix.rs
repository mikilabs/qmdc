//! Data-driven check of the `.qmdcignore` matcher against git's own answers (QMD-73).
//!
//! Reads `tests/ignore/gitignore-matrix.json`, whose `ignored` lists were produced by git
//! (`tests/ignore/gen_matrix.py`). The Python and TypeScript parsers run the same cases
//! through their ports of the same matcher, so the `ignore` suite has one case per matrix
//! entry in every language.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use qmdc::ignore::{is_ignored_relative, parse_qmdcignore};

mod common;

#[test]
fn test_ignore_matrix() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/ignore/gitignore-matrix.json");
    let doc: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read the ignore matrix"))
            .expect("parse the ignore matrix");
    let tree: Vec<String> = doc["tree"]
        .as_array()
        .expect("tree")
        .iter()
        .map(|p| p.as_str().expect("tree path").to_string())
        .collect();
    let mut report = common::CaseReport::new("ignore", "rs-ignore");

    for case in doc["cases"].as_array().expect("cases") {
        let t = Instant::now();
        let name = case["name"].as_str().expect("case name");
        let rules = parse_qmdcignore(case["content"].as_str().expect("content").as_bytes());
        let mut got: Vec<&str> = tree
            .iter()
            .filter(|p| is_ignored_relative(&rules, p, false))
            .map(|p| p.as_str())
            .collect();
        got.sort_unstable();
        let mut want: Vec<&str> = case["ignored"]
            .as_array()
            .expect("ignored")
            .iter()
            .map(|p| p.as_str().expect("ignored path"))
            .collect();
        want.sort_unstable();
        if got == want {
            report.pass(name, t.elapsed().as_secs_f64());
        } else {
            report.fail(
                name,
                &format!("ignored {:?}, git ignores {:?}", got, want),
                t.elapsed().as_secs_f64(),
            );
        }
    }

    report.finish();
}
