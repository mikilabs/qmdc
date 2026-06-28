//! Microtest runner - loads test cases dynamically from the microtests directory.

use std::fs;
use std::path::PathBuf;

use qmdc::{parse, rebuild, OutputFormat, ParseOptions};

mod common;

fn get_microtests_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("tests/parser")
}

/// Deep comparison of JSON values with order sensitivity for both arrays AND object keys.
/// Order matters for rebuild to preserve field order in documents.
fn deep_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Object(a_map), Value::Object(b_map)) => {
            if a_map.len() != b_map.len() {
                return false;
            }
            // Check keys are in same order
            let a_keys: Vec<_> = a_map.keys().collect();
            let b_keys: Vec<_> = b_map.keys().collect();
            if a_keys != b_keys {
                return false;
            }
            // Check values recursively
            for key in a_keys {
                if !deep_equal(&a_map[key], &b_map[key]) {
                    return false;
                }
            }
            true
        }
        (Value::Array(a_arr), Value::Array(b_arr)) => {
            if a_arr.len() != b_arr.len() {
                return false;
            }
            a_arr
                .iter()
                .zip(b_arr.iter())
                .all(|(a, b)| deep_equal(a, b))
        }
        _ => a == b,
    }
}

/// Get all microtest files with their format info
fn get_microtest_files() -> Vec<(String, PathBuf, PathBuf, OutputFormat)> {
    let dir = get_microtests_dir();
    let mut tests = Vec::new();

    let mut qmdc_files: Vec<_> = fs::read_dir(&dir)
        .expect("Failed to read microtests directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
        .filter(|p| p.to_string_lossy().ends_with(".qmd.md"))
        .collect();

    qmdc_files.sort();

    for qmdc_file in qmdc_files {
        let file_name = qmdc_file.file_name().unwrap().to_string_lossy();
        // e.g., "031-array-of-objects.qmd.md" -> "031-array-of-objects"
        let base_name = file_name
            .strip_suffix(".qmd.md")
            .unwrap_or(&file_name)
            .to_string();

        // Check for format-specific expected files
        let formats = [
            (".expected.json", OutputFormat::Standard),
            (".expected.minimal.json", OutputFormat::Minimal),
            (".expected.full.json", OutputFormat::Full),
        ];

        for (suffix, format) in formats {
            let expected_file = dir.join(format!("{}{}", base_name, suffix));
            if expected_file.exists() {
                let test_id = if format == OutputFormat::Standard {
                    base_name.clone()
                } else {
                    format!("{}[{:?}]", base_name, format)
                };
                tests.push((test_id, qmdc_file.clone(), expected_file, format));
            }
        }
    }

    tests
}

/// Get all .qmd.md microtest files (no expected JSON needed).
/// Used by text round-trip test which only needs the source file.
fn get_all_qmdc_files() -> Vec<(String, PathBuf)> {
    let dir = get_microtests_dir();
    let mut qmdc_files: Vec<_> = fs::read_dir(&dir)
        .expect("Failed to read microtests directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".qmd.md"))
        .collect();
    qmdc_files.sort();

    qmdc_files
        .into_iter()
        .map(|p| {
            let name = p
                .file_name()
                .unwrap()
                .to_string_lossy()
                .strip_suffix(".qmd.md")
                .unwrap()
                .to_string();
            (name, p)
        })
        .collect()
}

#[test]
fn test_all_microtests() {
    let mut tests = get_microtest_files();

    // Filter by MICROTEST_FILTER env var if set
    if let Ok(filter) = std::env::var("MICROTEST_FILTER") {
        tests.retain(|(name, _, _, _)| name.contains(&filter));
        eprintln!("Filtering tests by \"{}\": {} tests", filter, tests.len());
    }

    let mut failures = Vec::new();
    let mut report = common::CaseReport::new("parser", "rs-microtests");

    for (test_name, qmdc_file, expected_file, format) in &tests {
        let case_t = std::time::Instant::now();
        let markdown = fs::read_to_string(qmdc_file)
            .unwrap_or_else(|_| panic!("Failed to read {}", qmdc_file.display()));

        let expected_str = fs::read_to_string(expected_file)
            .unwrap_or_else(|_| panic!("Failed to read {}", expected_file.display()));

        let expected: serde_json::Value = serde_json::from_str(&expected_str)
            .unwrap_or_else(|e| panic!("Failed to parse expected JSON for {}: {}", test_name, e));

        let options = ParseOptions {
            random_seed: Some(666),
            format: *format,
        };

        let result = parse(&markdown, options);
        let result_json = serde_json::to_value(&result).unwrap();
        let secs = case_t.elapsed().as_secs_f64();

        if !deep_equal(&result_json, &expected) {
            report.fail(test_name, "parse output mismatch", secs);
            failures.push((
                test_name.clone(),
                format!("{:?}", format),
                serde_json::to_string_pretty(&result_json).unwrap(),
                serde_json::to_string_pretty(&expected).unwrap(),
            ));
        } else {
            report.pass(test_name, secs);
        }
    }

    if !failures.is_empty() {
        let mut msg = format!("\n\n{} test(s) failed:\n", failures.len());
        // Summary of failed test names first
        for (name, format, _, _) in &failures {
            msg.push_str(&format!("  ✗ {} (format: {})\n", name, format));
        }
        // Then details
        for (name, format, actual, expected) in &failures {
            msg.push_str(&format!("\n=== {} (format: {}) ===\n", name, format));
            msg.push_str("ACTUAL:\n");
            msg.push_str(actual);
            msg.push_str("\n\nEXPECTED:\n");
            msg.push_str(expected);
            msg.push('\n');
        }
        eprintln!("{}", msg);
    }

    report.finish();
    eprintln!("\n✓ All {} microtests passed!", tests.len());
}

#[test]
fn test_all_microtests_rebuild() {
    let mut tests = get_microtest_files();

    // Filter by MICROTEST_FILTER env var if set
    if let Ok(filter) = std::env::var("MICROTEST_FILTER") {
        tests.retain(|(name, _, _, _)| name.contains(&filter));
        eprintln!(
            "Filtering rebuild tests by \"{}\": {} tests",
            filter,
            tests.len()
        );
    }

    let mut failures = Vec::new();
    let mut passed = 0;
    let mut report = common::CaseReport::new("parser", "rs-microtests-rebuild");

    for (test_name, qmdc_file, _expected_file, format) in &tests {
        // Only test standard format for rebuild (full has extra metadata)
        if *format != OutputFormat::Standard {
            continue;
        }

        let case_t = std::time::Instant::now();
        let markdown = fs::read_to_string(qmdc_file)
            .unwrap_or_else(|_| panic!("Failed to read {}", qmdc_file.display()));

        let options = ParseOptions {
            random_seed: Some(666),
            format: *format,
        };

        // Parse original
        let parsed1 = parse(&markdown, options.clone());
        let json1 = serde_json::to_value(&parsed1).unwrap();

        // Skip rebuild test if there are parsing errors - rebuild of invalid docs is undefined
        let has_parsing_errors = json1
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|obj| obj.get("__kind").and_then(|k| k.as_str()) == Some("__ParsingError"))
            })
            .unwrap_or(false);

        if has_parsing_errors {
            eprintln!(
                "Skipping rebuild test for {} (has parsing errors)",
                test_name
            );
            report.skip(test_name, case_t.elapsed().as_secs_f64());
            continue;
        }

        // Rebuild
        let rebuilt = rebuild(&parsed1);

        // Parse rebuilt
        let parsed2 = parse(&rebuilt, options);
        let json2 = serde_json::to_value(&parsed2).unwrap();

        // Compare (order-sensitive — rebuild must preserve field order)
        let secs = case_t.elapsed().as_secs_f64();
        if !deep_equal(&json1, &json2) {
            report.fail(test_name, "rebuild round-trip mismatch", secs);
            failures.push((
                test_name.clone(),
                markdown.clone(),
                rebuilt.clone(),
                serde_json::to_string_pretty(&json1).unwrap(),
                serde_json::to_string_pretty(&json2).unwrap(),
            ));
        } else {
            report.pass(test_name, secs);
            passed += 1;
        }
    }

    if !failures.is_empty() {
        let mut msg = format!("\n\n{} rebuild test(s) failed:\n", failures.len());
        for (name, original, rebuilt, json1, json2) in &failures {
            msg.push_str(&format!("\n=== {} ===\n", name));
            msg.push_str("ORIGINAL MD:\n");
            msg.push_str(original);
            msg.push_str("\n\nREBUILT MD:\n");
            msg.push_str(rebuilt);
            msg.push_str("\n\nJSON1 (from original):\n");
            msg.push_str(json1);
            msg.push_str("\n\nJSON2 (from rebuilt):\n");
            msg.push_str(json2);
            msg.push('\n');
        }
        eprintln!("{}", msg);
    }

    report.finish();
    eprintln!("\n✓ All {} rebuild round-trip tests passed!", passed);
}

/// Strip all whitespace characters from a string.
fn strip_all_whitespace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Normalize a string for content-loss comparison:
/// - Remove all `[[...]]` bracket tokens (ID/Kind normalization is not content loss)
/// - Remove quotes around values (`"..."` → `...`)
/// - Remove HTML comments (`<!-- ... -->`)
/// - Remove heading markers (`#` prefixes) — heading level changes are detected separately
/// - Strip all whitespace
fn normalize_for_content_comparison(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip HTML comments <!-- ... -->
        if i + 3 < len
            && chars[i] == '<'
            && chars[i + 1] == '!'
            && chars[i + 2] == '-'
            && chars[i + 3] == '-'
        {
            // Find closing -->
            let mut j = i + 4;
            let mut found = false;
            while j + 2 < len {
                if chars[j] == '-' && chars[j + 1] == '-' && chars[j + 2] == '>' {
                    j += 3;
                    found = true;
                    break;
                }
                j += 1;
            }
            if !found {
                j = len;
            }
            i = j;
            continue;
        }

        // Skip [[...]] bracket tokens (handles nested single brackets inside)
        if i + 1 < len && chars[i] == '[' && chars[i + 1] == '[' {
            // Count bracket depth: [[ opens, ]] closes
            // But we also need to handle single [ inside (like [[members: [User]]])
            let mut j = i + 2;
            let mut depth = 1; // we consumed one [[
            while j < len && depth > 0 {
                if j + 1 < len && chars[j] == '[' && chars[j + 1] == '[' {
                    depth += 1;
                    j += 2;
                } else if j + 1 < len && chars[j] == ']' && chars[j + 1] == ']' {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            i = j;
            continue;
        }

        // Skip quotes (but keep the content inside)
        if chars[i] == '"' {
            i += 1;
            continue;
        }

        result.push(chars[i]);
        i += 1;
    }

    // Strip heading markers — we detect heading level changes separately
    let normalized = result
        .lines()
        .map(|line| line.trim_start_matches('#').to_string())
        .collect::<Vec<_>>()
        .join("\n");

    // Normalize table separator rows (|---|---|, |-------|-----|, etc.) to just pipes
    let re = regex::Regex::new(r"\|[-:]+(?:\|[-:]+)*\|").unwrap();
    let normalized = re
        .replace_all(&normalized, |caps: &regex::Captures| {
            "|".repeat(caps[0].matches('|').count())
        })
        .to_string();

    strip_all_whitespace(&normalized)
}

/// Check if the difference between original and rebuilt text is normalization-only.
/// Returns None if texts are equivalent, or Some(diff_description) if real content loss.
///
/// Detects two types of problems:
/// 1. Content loss/reordering: actual text content disappears or moves
/// 2. Heading level changes: `###` becomes `####` (structural bug)
fn check_content_loss(original: &str, rebuilt: &str) -> Option<String> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let rebuilt_lines: Vec<&str> = rebuilt.lines().collect();

    let mut problems = Vec::new();

    // Check 1: Heading level changes
    // Extract all headings from both, match by position (index), compare levels
    let orig_headings: Vec<(usize, &str)> = orig_lines
        .iter()
        .filter(|l| l.starts_with('#'))
        .map(|l| {
            let level = l.chars().take_while(|c| *c == '#').count();
            let text = l.trim_start_matches('#').trim();
            (level, text)
        })
        .collect();

    let rebuilt_headings: Vec<(usize, &str)> = rebuilt_lines
        .iter()
        .filter(|l| l.starts_with('#'))
        .map(|l| {
            let level = l.chars().take_while(|c| *c == '#').count();
            let text = l.trim_start_matches('#').trim();
            (level, text)
        })
        .collect();

    // Match headings positionally (by index) — same Nth heading in both
    let heading_count = orig_headings.len().min(rebuilt_headings.len());
    for idx in 0..heading_count {
        let orig_h = &orig_headings[idx];
        let rebuilt_h = &rebuilt_headings[idx];
        let orig_label = normalize_for_content_comparison(orig_h.1);
        let rebuilt_label = normalize_for_content_comparison(rebuilt_h.1);
        // Only compare levels if the labels match (same heading, different level)
        if orig_label == rebuilt_label && !orig_label.is_empty() && orig_h.0 != rebuilt_h.0 {
            problems.push(format!(
                "  HEADING LEVEL CHANGE: \"{}\" was h{}, now h{} (\"{}\")",
                orig_h.1, orig_h.0, rebuilt_h.0, rebuilt_h.1
            ));
        }
    }

    // Check 2: Content loss via LCS diff with normalization
    let n = orig_lines.len();
    let m = rebuilt_lines.len();

    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if orig_lines[i - 1] == rebuilt_lines[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    enum DiffOp {
        Equal,
        Removed(String),
        Added(String),
    }

    let mut ops = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && orig_lines[i - 1] == rebuilt_lines[j - 1] {
            ops.push(DiffOp::Equal);
            i -= 1;
            j -= 1;
        } else if i > 0 && (j == 0 || dp[i - 1][j] >= dp[i][j - 1]) {
            ops.push(DiffOp::Removed(orig_lines[i - 1].to_string()));
            i -= 1;
        } else {
            ops.push(DiffOp::Added(rebuilt_lines[j - 1].to_string()));
            j -= 1;
        }
    }
    ops.reverse();

    // Group consecutive removed/added into hunks
    let mut hunks: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    let mut current_removed: Vec<String> = Vec::new();
    let mut current_added: Vec<String> = Vec::new();

    for op in &ops {
        match op {
            DiffOp::Equal => {
                if !current_removed.is_empty() || !current_added.is_empty() {
                    hunks.push((
                        std::mem::take(&mut current_removed),
                        std::mem::take(&mut current_added),
                    ));
                }
            }
            DiffOp::Removed(line) => {
                current_removed.push(line.clone());
            }
            DiffOp::Added(line) => {
                current_added.push(line.clone());
            }
        }
    }
    if !current_removed.is_empty() || !current_added.is_empty() {
        hunks.push((current_removed, current_added));
    }

    // Check each hunk with normalization
    for (removed, added) in &hunks {
        let removed_text: String = removed.join("\n");
        let added_text: String = added.join("\n");
        let removed_normalized = normalize_for_content_comparison(&removed_text);
        let added_normalized = normalize_for_content_comparison(&added_text);
        if removed_normalized != added_normalized {
            problems.push(format!(
                "  CONTENT LOSS:\n    REMOVED: {:?}\n    ADDED:   {:?}\n    (normalized: {:?} vs {:?})",
                removed_text, added_text, removed_normalized, added_normalized
            ));
        }
    }

    if problems.is_empty() {
        None
    } else {
        Some(problems.join("\n\n"))
    }
}

/// Text-level round-trip test: detects real content loss (not just whitespace/normalization).
/// This catches bugs that the JSON round-trip test misses — e.g., when rebuild loses
/// content but the truncated text re-parses to the same truncated JSON.
///
/// Allowed normalizations (not flagged):
/// - Whitespace changes (extra/missing blank lines, trailing spaces)
/// - `[[...]]` bracket content changes (ID/Kind normalization)
/// - Quote changes around field values
/// - HTML comment removal
///
/// Flagged as bugs:
/// - Content lines disappearing
/// - Content lines reordering
/// - Heading level changes (`###` → `####`)
#[test]
fn test_all_microtests_rebuild_text() {
    let mut tests = get_all_qmdc_files();

    if let Ok(filter) = std::env::var("MICROTEST_FILTER") {
        tests.retain(|(name, _)| name.contains(&filter));
        eprintln!(
            "Filtering text rebuild tests by \"{}\": {} tests",
            filter,
            tests.len()
        );
    }

    let mut failures = Vec::new();
    let mut passed = 0;
    let mut skipped = 0;
    let mut report = common::CaseReport::new("parser", "rs-microtests-text");

    for (test_name, qmdc_file) in &tests {
        let case_t = std::time::Instant::now();
        let markdown = fs::read_to_string(qmdc_file)
            .unwrap_or_else(|_| panic!("Failed to read {}", qmdc_file.display()));

        let options = ParseOptions {
            random_seed: Some(666),
            format: OutputFormat::Standard,
        };

        let parsed = parse(&markdown, options);
        let json = serde_json::to_value(&parsed).unwrap();

        // Skip if parsing errors
        let has_parsing_errors = json
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|obj| obj.get("__kind").and_then(|k| k.as_str()) == Some("__ParsingError"))
            })
            .unwrap_or(false);

        if has_parsing_errors {
            skipped += 1;
            report.skip(test_name, case_t.elapsed().as_secs_f64());
            continue;
        }

        let rebuilt = rebuild(&parsed);

        let secs = case_t.elapsed().as_secs_f64();
        if let Some(diff_desc) = check_content_loss(&markdown, &rebuilt) {
            report.fail(test_name, "text round-trip content loss", secs);
            failures.push((test_name.clone(), diff_desc));
        } else {
            passed += 1;
            report.pass(test_name, secs);
        }
    }

    if !failures.is_empty() {
        let mut msg = format!(
            "\n\n{} text round-trip test(s) failed (content loss detected):\n",
            failures.len()
        );
        // Summary of failed test names first
        for (name, _) in &failures {
            msg.push_str(&format!("  ✗ {}\n", name));
        }
        // Then details
        for (name, diff) in &failures {
            msg.push_str(&format!("\n=== {} ===\n{}\n", name, diff));
        }
        msg.push_str(&format!("\n({} passed, {} skipped)\n", passed, skipped));
        eprintln!("{}", msg);
    }

    report.finish();
    eprintln!(
        "\n✓ All {} text round-trip tests passed! ({} skipped)",
        passed, skipped
    );
}

/// Run a single test by name (for debugging)
#[allow(dead_code)]
fn run_single_test(test_pattern: &str) {
    let tests = get_microtest_files();
    let matching: Vec<_> = tests
        .iter()
        .filter(|(name, _, _, _)| name.contains(test_pattern))
        .collect();

    if matching.is_empty() {
        println!("No tests matching '{}'", test_pattern);
        return;
    }

    for (test_name, qmdc_file, expected_file, format) in matching {
        println!("\n=== Testing {} ({:?}) ===", test_name, format);

        let markdown = fs::read_to_string(qmdc_file).unwrap();
        let expected_str = fs::read_to_string(expected_file).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&expected_str).unwrap();

        let options = ParseOptions {
            random_seed: Some(666),
            format: *format,
        };

        let result = parse(&markdown, options);
        let result_json = serde_json::to_value(&result).unwrap();

        println!("INPUT:\n{}", markdown);
        println!(
            "\nACTUAL:\n{}",
            serde_json::to_string_pretty(&result_json).unwrap()
        );
        println!(
            "\nEXPECTED:\n{}",
            serde_json::to_string_pretty(&expected).unwrap()
        );

        if deep_equal(&result_json, &expected) {
            println!("\n✓ PASS");
        } else {
            println!("\n✗ FAIL");
        }
    }
}
