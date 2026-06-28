//! QMD-58 regression test: LSP must clear stale `broken_link` diagnostics for
//! forward references once the target file is created — without a restart.
//!
//! Originally written as a triage artifact that was intentionally RED against
//! the pre-fix code (see docs/tracking/.../QMD-58). Now green: the fix
//! re-publishes diagnostics for open documents when a new object id appears.
//!
//! Scenario (the minimal repro from the bug report):
//!   1. Open `a.qmd.md` which references `[[#b_obj]]` — `b_obj` does not exist
//!      yet. The LSP correctly publishes a `broken_link` diagnostic for A.
//!   2. Without restarting, create `b.qmd.md` on disk defining `b_obj` and
//!      notify the server via `workspace/didChangeWatchedFiles` (CREATED) —
//!      exactly what an editor sends when a new file appears.
//!   3. EXPECTED: the server re-resolves references to `b_obj` and clears A's
//!      broken-link diagnostic (matching `qmdc workspace validate` == []).
//!
//! The test drives the REAL `Backend` over `tower::Service` (same style as
//! `tests/lsp_microtests.rs`) and captures `textDocument/publishDiagnostics`
//! notifications, tracking the LATEST diagnostic set per file URI.

use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::Service;
use tower_lsp::lsp_types::Url;
use tower_lsp::LspService;

use qmdc::lsp::server::Backend;

fn build_request(method: &'static str, params: Value, id: i64) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .id(id)
        .finish()
}

fn build_notification(method: &'static str, params: Value) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .finish()
}

/// Latest broken-link count published for the file whose URI ends with `suffix`.
/// Returns `None` if no diagnostics were ever published for that file.
fn latest_broken_link_count(captured: &Arc<Mutex<Vec<Value>>>, suffix: &str) -> Option<usize> {
    let diags = captured.lock().unwrap();
    // Walk newest-first so we report the most recently published set.
    for notif in diags.iter().rev() {
        let uri = notif.get("uri").and_then(|u| u.as_str()).unwrap_or("");
        if uri.ends_with(suffix) {
            let arr = notif
                .get("diagnostics")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            let broken = arr
                .iter()
                .filter(|d| {
                    let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    // Broken-link diagnostics carry code QMDC001 / "not found".
                    let code = d.get("code").and_then(|c| c.as_str()).unwrap_or("");
                    code == "QMDC001" || msg.contains("not found")
                })
                .count();
            return Some(broken);
        }
    }
    None
}

#[tokio::test]
async fn qmd58_forward_reference_clears_after_target_created() {
    // --- workspace on disk: readme (__Workspace) + a.qmd.md with a forward ref ---
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();

    fs::write(
        root.join("readme.qmd.md"),
        "# Repro Workspace [[repro_ws:__Workspace]]\n",
    )
    .unwrap();

    let a_path = root.join("a.qmd.md");
    fs::write(&a_path, "## A [[a_obj: Thing]]\n\n- depends: [[#b_obj]]\n").unwrap();
    let a_uri = Url::from_file_path(&a_path).unwrap().to_string();

    // --- bring up the real LSP backend ---
    let (mut service, socket) = LspService::new(Backend::new);

    let captured: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_task = captured.clone();
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(msg) = socket.next().await {
            if let Ok(s) = serde_json::to_string(&msg) {
                if s.contains("publishDiagnostics") {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&s) {
                        if let Some(params) = parsed.get("params") {
                            captured_task.lock().unwrap().push(params.clone());
                        }
                    }
                }
            }
        }
    });

    let ws_uri = Url::from_file_path(&root).unwrap().to_string();
    let init = build_request(
        "initialize",
        json!({
            "capabilities": {},
            "workspaceFolders": [{ "uri": ws_uri, "name": "repro" }]
        }),
        1,
    );
    let _ = service.call(init).await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    // Step 1: open A. b_obj does not exist yet -> broken_link expected.
    let a_content = fs::read_to_string(&a_path).unwrap();
    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": a_uri,
                    "languageId": "qmd",
                    "version": 1,
                    "text": a_content,
                }
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Sanity: the bug precondition. A must currently show exactly one broken link.
    let before = latest_broken_link_count(&captured, "a.qmd.md");
    assert_eq!(
        before,
        Some(1),
        "precondition: A should have 1 broken_link before b.qmd.md exists, got {:?}",
        before
    );

    // Step 2: create b.qmd.md on disk defining b_obj, and notify the server
    // exactly as an editor would when a new file appears in the workspace.
    let b_path = root.join("b.qmd.md");
    fs::write(&b_path, "## B [[b_obj: Thing]]\n").unwrap();
    let b_uri = Url::from_file_path(&b_path).unwrap().to_string();

    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{ "uri": b_uri, "type": 1 }] // 1 == Created
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(400)).await;

    // Step 3: A's reference [[#b_obj]] now resolves on disk
    // (qmdc workspace validate would return []). The LSP must have re-published
    // A's diagnostics with zero broken links.
    let after = latest_broken_link_count(&captured, "a.qmd.md");
    assert_eq!(
        after,
        Some(0),
        "after b.qmd.md is created, A's stale broken_link must be cleared \
         without a restart (got {:?}). This is QMD-58.",
        after
    );
}
