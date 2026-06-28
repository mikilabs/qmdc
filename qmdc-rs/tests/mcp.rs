//! MCP fixture-driven integration tests (rmcp-based).
//!
//! Reuses the SAME fixture directories as lsp-microtests. Each fixture dir can optionally
//! contain:
//!   - `mcp-request.json`  — `{ "tool": "...", "arguments": {...} }`
//!   - `mcp-expected.json` — expected inner JSON of the tool's text content block
//!
//! and/or a resource pair:
//!   - `mcp-resource-request.json`  — `{ "uri": "qmdc://...", "path": "{{workspace}}" }`
//!   - `mcp-resource-expected.json` — `{ "uri", "mimeType", "content": {...} }`
//!
//! The runner drives a REAL `rmcp` client against a REAL `QmdcServer`, connected over an
//! in-memory `tokio::io::duplex` transport (no child process). This exercises the full stack:
//! the macro-generated tool router, schema-checked argument deserialization, the tool body,
//! and result serialization.
//!
//! Special matchers in expected JSON: `"__array_nonempty__"`, `"__positive_integer__"`, `"__any__"`.

use std::fs;
use std::path::{Path, PathBuf};

use rmcp::model::{CallToolRequestParam, ReadResourceRequestParam};
use rmcp::service::{RoleClient, RunningService};
use rmcp::ServiceExt;
use serde_json::{json, Value};

use qmdc::mcp::tools::QmdcServer;

mod common;

// ---------------------------------------------------------------------------
// In-memory rmcp client/server harness
// ---------------------------------------------------------------------------

/// A connected rmcp client peer talking to an in-process `QmdcServer` over a duplex stream.
type Client = RunningService<RoleClient, ()>;

/// Spin up a `QmdcServer` on one end of an in-memory duplex and return a connected client peer.
/// The server task runs detached and ends when the client is cancelled/dropped.
async fn connect() -> Client {
    let (client_io, server_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        if let Ok(server) = QmdcServer::new().serve(server_io).await {
            let _ = server.waiting().await;
        }
    });
    ().serve(client_io).await.expect("client failed to connect")
}

/// Call a tool and return `(inner_text, is_error)`. The QMD tools always return a single text
/// content block carrying the serialized envelope JSON.
async fn call_tool_text(
    client: &Client,
    name: &str,
    args: Value,
) -> Result<(String, bool), String> {
    let arguments = args.as_object().cloned();
    let res = client
        .call_tool(CallToolRequestParam {
            name: name.to_string().into(),
            arguments,
        })
        .await
        .map_err(|e| format!("call_tool {}: {}", name, e))?;
    let is_error = res.is_error.unwrap_or(false);
    let text = res
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .ok_or_else(|| "tool result has no text content".to_string())?;
    Ok((text, is_error))
}

// ---------------------------------------------------------------------------
// Fixture discovery
// ---------------------------------------------------------------------------

fn get_lsp_microtests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/lsp/microtests")
}

fn get_lsp_sql_tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/lsp/sql")
}

/// MCP-specific envelope/pagination/filter fixtures (QMD-62). Data-driven: each dir has a
/// `workspace/` subdir plus `mcp-request.json` + `mcp-expected.json`.
fn get_mcp_envelope_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/mcp")
}

fn discover_mcp_tests() -> Vec<(String, PathBuf)> {
    let mut tests = Vec::new();
    for (label, base_dir) in [
        ("lsp-microtests", get_lsp_microtests_dir()),
        ("lsp-sql-tests", get_lsp_sql_tests_dir()),
        ("envelope-tests", get_mcp_envelope_dir()),
    ] {
        if base_dir.exists() {
            collect_pairs(
                &base_dir,
                label,
                "mcp-request.json",
                "mcp-expected.json",
                &mut tests,
            );
        }
    }
    tests.sort_by(|a, b| a.0.cmp(&b.0));
    tests
}

fn discover_mcp_resource_tests() -> Vec<(String, PathBuf)> {
    let mut tests = Vec::new();
    for (label, base_dir) in [
        ("lsp-microtests", get_lsp_microtests_dir()),
        ("lsp-sql-tests", get_lsp_sql_tests_dir()),
    ] {
        if base_dir.exists() {
            collect_pairs(
                &base_dir,
                label,
                "mcp-resource-request.json",
                "mcp-resource-expected.json",
                &mut tests,
            );
        }
    }
    tests.sort_by(|a, b| a.0.cmp(&b.0));
    tests
}

fn collect_pairs(
    dir: &Path,
    prefix: &str,
    request_name: &str,
    expected_name: &str,
    tests: &mut Vec<(String, PathBuf)>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for sub in &dirs {
        if sub.join(request_name).exists() && sub.join(expected_name).exists() {
            let rel = sub
                .strip_prefix(dir.parent().unwrap_or(dir))
                .unwrap_or(sub.as_path());
            tests.push((format!("{}/{}", prefix, rel.display()), sub.clone()));
        }
        collect_pairs(sub, prefix, request_name, expected_name, tests);
    }
}

fn workspace_for(fixture_dir: &Path) -> Result<PathBuf, String> {
    let p = if fixture_dir.join("workspace").is_dir() {
        fixture_dir.join("workspace")
    } else {
        fixture_dir.to_path_buf()
    };
    p.canonicalize()
        .map_err(|e| format!("canonicalize workspace: {}", e))
}

// ---------------------------------------------------------------------------
// Test execution
// ---------------------------------------------------------------------------

async fn run_mcp_test(client: &Client, fixture_dir: &Path) -> Result<(), String> {
    let request: Value = serde_json::from_str(
        &fs::read_to_string(fixture_dir.join("mcp-request.json"))
            .map_err(|e| format!("read mcp-request.json: {}", e))?,
    )
    .map_err(|e| format!("parse mcp-request.json: {}", e))?;
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(fixture_dir.join("mcp-expected.json"))
            .map_err(|e| format!("read mcp-expected.json: {}", e))?,
    )
    .map_err(|e| format!("parse mcp-expected.json: {}", e))?;

    let workspace_path = workspace_for(fixture_dir)?;
    let tool_name = request
        .get("tool")
        .and_then(|t| t.as_str())
        .ok_or("mcp-request.json missing 'tool' field")?;
    let mut arguments = request.get("arguments").cloned().unwrap_or(json!({}));
    replace_workspace_placeholder(&mut arguments, &workspace_path);

    let (content_text, _is_error) = call_tool_text(client, tool_name, arguments).await?;
    let actual: Value =
        serde_json::from_str(&content_text).map_err(|e| format!("parse content text: {}", e))?;

    match_value(&expected, &actual, "$").map_err(|e| {
        format!(
            "{}\n  actual: {}",
            e,
            serde_json::to_string_pretty(&actual).unwrap_or_default()
        )
    })
}

async fn run_mcp_resource_test(client: &Client, fixture_dir: &Path) -> Result<(), String> {
    let request: Value = serde_json::from_str(
        &fs::read_to_string(fixture_dir.join("mcp-resource-request.json"))
            .map_err(|e| format!("read mcp-resource-request.json: {}", e))?,
    )
    .map_err(|e| format!("parse mcp-resource-request.json: {}", e))?;
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(fixture_dir.join("mcp-resource-expected.json"))
            .map_err(|e| format!("read mcp-resource-expected.json: {}", e))?,
    )
    .map_err(|e| format!("parse mcp-resource-expected.json: {}", e))?;

    let workspace_path = workspace_for(fixture_dir)?;
    let base_uri = request
        .get("uri")
        .and_then(|u| u.as_str())
        .ok_or("mcp-resource-request.json missing 'uri' field")?;

    // Standard MCP resources/read carries only a uri. Dynamic resources take the workspace
    // as a `?path=` query suffix (percent-encoded).
    let uri = match request.get("path").and_then(|p| p.as_str()) {
        Some(raw_path) => {
            let path = raw_path.replace("{{workspace}}", &workspace_path.to_string_lossy());
            format!("{}?path={}", base_uri, percent_encode(&path))
        }
        None => base_uri.to_string(),
    };

    let result = client
        .read_resource(ReadResourceRequestParam { uri: uri.clone() })
        .await
        .map_err(|e| format!("read_resource {}: {}", uri, e))?;

    let block = result
        .contents
        .first()
        .ok_or_else(|| "no resource contents in response".to_string())?;
    let (actual_uri, actual_mime, actual_text) = match block {
        rmcp::model::ResourceContents::TextResourceContents {
            uri,
            mime_type,
            text,
            ..
        } => (
            uri.clone(),
            mime_type.clone().unwrap_or_default(),
            text.clone(),
        ),
        _ => return Err("resource block is not text".to_string()),
    };

    // The served uri carries the `?path=` suffix; compare on the base.
    let actual_base = actual_uri.split('?').next().unwrap_or(&actual_uri);
    if actual_base != base_uri {
        return Err(format!(
            "uri: expected {:?}, got {:?}",
            base_uri, actual_base
        ));
    }
    if let Some(exp_mime) = expected.get("mimeType").and_then(|m| m.as_str()) {
        if actual_mime != exp_mime {
            return Err(format!(
                "mimeType: expected {:?}, got {:?}",
                exp_mime, actual_mime
            ));
        }
    }

    let actual_content: Value = serde_json::from_str(&actual_text)
        .map_err(|e| format!("parse resource content text: {}", e))?;
    let expected_content = expected
        .get("content")
        .ok_or("mcp-resource-expected.json missing 'content' field")?;
    match_value(expected_content, &actual_content, "$").map_err(|e| {
        format!(
            "{}\n  actual: {}",
            e,
            serde_json::to_string_pretty(&actual_content).unwrap_or_default()
        )
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn replace_workspace_placeholder(value: &mut Value, workspace_path: &Path) {
    let ws_str = workspace_path.to_string_lossy().to_string();
    match value {
        Value::String(s) => {
            if s.contains("{{workspace}}") {
                *s = s.replace("{{workspace}}", &ws_str);
            }
        }
        Value::Object(map) => map
            .values_mut()
            .for_each(|v| replace_workspace_placeholder(v, workspace_path)),
        Value::Array(arr) => arr
            .iter_mut()
            .for_each(|v| replace_workspace_placeholder(v, workspace_path)),
        _ => {}
    }
}

/// Minimal percent-encoding for a path placed in a `?path=` query value.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn match_value(expected: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match expected {
        Value::String(s) => match s.as_str() {
            "__array_nonempty__" => actual
                .as_array()
                .filter(|a| !a.is_empty())
                .map(|_| ())
                .ok_or_else(|| {
                    format!(
                        "{}: expected non-empty array, got {}",
                        path,
                        type_of(actual)
                    )
                }),
            "__positive_integer__" => actual
                .as_i64()
                .filter(|&n| n > 0)
                .map(|_| ())
                .or_else(|| actual.as_u64().filter(|&n| n > 0).map(|_| ()))
                .ok_or_else(|| format!("{}: expected positive integer, got {}", path, actual)),
            "__any__" => Ok(()),
            _ => actual
                .as_str()
                .filter(|a| *a == s.as_str())
                .map(|_| ())
                .ok_or_else(|| format!("{}: expected {:?}, got {:?}", path, s, actual)),
        },
        Value::Number(_) | Value::Bool(_) | Value::Null => {
            if expected == actual {
                Ok(())
            } else {
                Err(format!("{}: expected {}, got {}", path, expected, actual))
            }
        }
        Value::Array(exp_arr) => {
            let act_arr = actual
                .as_array()
                .ok_or_else(|| format!("{}: expected array, got {}", path, type_of(actual)))?;
            if exp_arr.len() != act_arr.len() {
                return Err(format!(
                    "{}: array len {} != {}",
                    path,
                    exp_arr.len(),
                    act_arr.len()
                ));
            }
            for (i, (e, a)) in exp_arr.iter().zip(act_arr).enumerate() {
                match_value(e, a, &format!("{}[{}]", path, i))?;
            }
            Ok(())
        }
        Value::Object(exp_map) => {
            let act_map = actual
                .as_object()
                .ok_or_else(|| format!("{}: expected object, got {}", path, type_of(actual)))?;
            for (key, exp_val) in exp_map {
                let act_val = act_map
                    .get(key)
                    .ok_or_else(|| format!("{}.{}: missing in actual", path, key))?;
                match_value(exp_val, act_val, &format!("{}.{}", path, key))?;
            }
            Ok(())
        }
    }
}

fn type_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ---------------------------------------------------------------------------
// Test entry points
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mcp_fixture_tests() {
    let tests = discover_mcp_tests();
    assert!(
        !tests.is_empty(),
        "No MCP tool fixtures found — fixture path likely broken"
    );
    let client = connect().await;
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut pass_count = 0;
    let mut report = common::CaseReport::new("mcp", "rs-mcp-fixtures");
    for (name, fixture_dir) in &tests {
        let case_t = std::time::Instant::now();
        let outcome = run_mcp_test(&client, fixture_dir).await;
        let secs = case_t.elapsed().as_secs_f64();
        match outcome {
            Ok(()) => {
                pass_count += 1;
                report.pass(name, secs);
                eprintln!("  PASS: {}", name);
            }
            Err(e) => {
                eprintln!("  FAIL: {} — {}", name, e);
                report.fail(name, &e, secs);
                failures.push((name.clone(), e));
            }
        }
    }
    eprintln!(
        "\nMCP fixture tests: {} passed, {} failed",
        pass_count,
        failures.len()
    );
    let _ = client.cancel().await;
    report.finish();
}

#[tokio::test]
async fn mcp_resource_fixture_tests() {
    let tests = discover_mcp_resource_tests();
    if tests.is_empty() {
        panic!("No MCP resource fixtures found.");
    }
    let client = connect().await;
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut pass_count = 0;
    let mut report = common::CaseReport::new("mcp", "rs-mcp-resources");
    for (name, fixture_dir) in &tests {
        let case_t = std::time::Instant::now();
        let outcome = run_mcp_resource_test(&client, fixture_dir).await;
        let secs = case_t.elapsed().as_secs_f64();
        match outcome {
            Ok(()) => {
                pass_count += 1;
                report.pass(name, secs);
                eprintln!("  PASS: {}", name);
            }
            Err(e) => {
                eprintln!("  FAIL: {} — {}", name, e);
                report.fail(name, &e, secs);
                failures.push((name.clone(), e));
            }
        }
    }
    eprintln!(
        "\nMCP resource fixture tests: {} passed, {} failed",
        pass_count,
        failures.len()
    );
    let _ = client.cancel().await;
    report.finish();
}

/// `qmdc://guide` is static and build-embedded (FR-19): served text must equal the on-disk
/// guide byte-for-byte (proves it is compiled in via `include_str!`, not read at runtime).
#[tokio::test]
async fn mcp_guide_resource_matches_embedded_source() {
    const GUIDE_SRC: &str = include_str!("../../docs/guides/qmdc-guide.qmd.md");
    let client = connect().await;
    let result = client
        .read_resource(ReadResourceRequestParam {
            uri: "qmdc://guide".to_string(),
        })
        .await
        .expect("read qmdc://guide");
    match result.contents.first().expect("guide content") {
        rmcp::model::ResourceContents::TextResourceContents {
            uri,
            mime_type,
            text,
            ..
        } => {
            assert_eq!(uri, "qmdc://guide");
            assert_eq!(mime_type.as_deref(), Some("text/markdown"));
            assert_eq!(
                text, GUIDE_SRC,
                "qmdc://guide must equal the embedded source byte-for-byte"
            );
        }
        _ => panic!("guide content is not text"),
    }
    let _ = client.cancel().await;
}

/// `resources/list` must advertise exactly the four `qmdc://` resources (FR-19/FR-20).
#[tokio::test]
async fn mcp_resources_list_returns_all_resources() {
    let client = connect().await;
    let result = client
        .list_resources(Default::default())
        .await
        .expect("list_resources");
    let uris: Vec<String> = result.resources.iter().map(|r| r.uri.clone()).collect();
    assert_eq!(
        uris,
        vec![
            "qmdc://guide".to_string(),
            "qmdc://tree".to_string(),
            "qmdc://object/{id}".to_string(),
            "qmdc://diagnostics".to_string(),
        ],
        "resources/list must advertise exactly the four qmdc:// resources"
    );
    let _ = client.cancel().await;
}

/// The `qmdc_get_guide` tool is the tool-surface twin of the `qmdc://guide` resource: it must
/// serve the SAME build-embedded guide byte-for-byte for tools-only clients.
#[tokio::test]
async fn mcp_get_guide_tool_matches_embedded_source() {
    const GUIDE_SRC: &str = include_str!("../../docs/guides/qmdc-guide.qmd.md");
    let client = connect().await;
    let (text, is_error) = call_tool_text(&client, "qmdc_get_guide", json!({}))
        .await
        .unwrap();
    assert!(!is_error);
    assert_eq!(
        text, GUIDE_SRC,
        "get_guide tool text must equal the embedded source byte-for-byte"
    );
    let _ = client.cancel().await;
}

/// `tools/list` must return all 14 tools with the expected names.
#[tokio::test]
async fn mcp_tools_list_returns_all_tools() {
    let client = connect().await;
    let result = client
        .list_tools(Default::default())
        .await
        .expect("list_tools");
    let names: Vec<String> = result.tools.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(
        names.len(),
        14,
        "Expected 14 tools, got {}: {:?}",
        names.len(),
        names
    );
    for expected in [
        "qmdc_locate_object",
        "qmdc_find_references",
        "qmdc_rename_object",
        "qmdc_describe_object",
        "qmdc_outline_file",
        "qmdc_search_objects",
        "qmdc_validate_references",
        "qmdc_get_tree",
        "qmdc_query_sql",
        "qmdc_dump_index",
        "qmdc_traverse_graph",
        "qmdc_find_path",
        "qmdc_describe_metamodel",
        "qmdc_get_guide",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "Missing tool '{}'",
            expected
        );
    }
    let _ = client.cancel().await;
}

/// A tool schema must advertise its pagination/filter params (single source of truth: the
/// param struct). Check `qmdc_get_tree` exposes `limit`, `cursor`, `namespace`.
#[tokio::test]
async fn mcp_get_tree_schema_advertises_pagination_and_filters() {
    let client = connect().await;
    let result = client
        .list_tools(Default::default())
        .await
        .expect("list_tools");
    let tree = result
        .tools
        .iter()
        .find(|t| t.name == "qmdc_get_tree")
        .expect("qmdc_get_tree present");
    let schema = serde_json::to_value(&tree.input_schema).unwrap();
    let props = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .expect("input schema properties");
    for field in ["path", "namespace", "limit", "cursor"] {
        assert!(
            props.contains_key(field),
            "qmdc_get_tree schema missing '{}'",
            field
        );
    }
    let _ = client.cancel().await;
}
