//! Repro: a namespaced reference to a hierarchical child object — `[[#lsp:completion]]` →
//! `lsp_completion.completion` (local id `completion`, namespace `lsp`) — must NOT be flagged
//! as a broken link by the LSP. `qmdc workspace validate` returns [] for the same workspace,
//! so the LSP diagnostics must agree.
//!
//! Mirrors the directory layout of `docs/lsp/`: a namespace is declared in a subdir's
//! `readme.qmd.md` (`[[lsp:__Namespace]]`) and the referenced object lives in a sibling file
//! in that same namespaced directory.

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

fn broken_links(captured: &Arc<Mutex<Vec<Value>>>, suffix: &str) -> Option<Vec<String>> {
    let diags = captured.lock().unwrap();
    for notif in diags.iter().rev() {
        let uri = notif.get("uri").and_then(|u| u.as_str()).unwrap_or("");
        if uri.ends_with(suffix) {
            let arr = notif
                .get("diagnostics")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            let msgs = arr
                .iter()
                .filter(|d| {
                    let code = d.get("code").and_then(|c| c.as_str()).unwrap_or("");
                    let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    code == "QMDC001" || msg.contains("not found")
                })
                .filter_map(|d| d.get("message").and_then(|m| m.as_str()).map(String::from))
                .collect();
            return Some(msgs);
        }
    }
    None
}

#[tokio::test]
async fn namespaced_localid_reference_is_not_flagged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().to_path_buf();

    // Workspace root.
    fs::write(root.join("readme.qmd.md"), "# WS [[ws:__Workspace]]\n").unwrap();

    // Namespaced subdir `lsp/`.
    let lsp_dir = root.join("lsp");
    fs::create_dir_all(&lsp_dir).unwrap();

    // The referencing file: declares the `lsp` namespace and references a child by local id.
    let readme = "# LSP Server [[lsp:__Namespace]]\n\n- features: [[#lsp:completion]]\n";
    fs::write(lsp_dir.join("readme.qmd.md"), readme).unwrap();

    // The target lives in a sibling file in the same namespaced directory: a Category with a
    // child whose local id is `completion` (hierarchical id `lsp_completion.completion`).
    let completion = "# Completion [[lsp_completion: Category]]\n\n\
                      ## Completion [[completion: LSPFeature]]\n\n- status: implemented\n";
    fs::write(lsp_dir.join("completion.qmd.md"), completion).unwrap();

    let (mut service, socket) = LspService::new(Backend::new);
    let captured: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_task = captured.clone();
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(msg) = socket.next().await {
            if let Ok(s) = serde_json::to_string(&msg) {
                if s.contains("publishDiagnostics") {
                    if let Ok(p) = serde_json::from_str::<Value>(&s) {
                        if let Some(params) = p.get("params") {
                            captured_task.lock().unwrap().push(params.clone());
                        }
                    }
                }
            }
        }
    });

    let ws_uri = Url::from_file_path(&root).unwrap().to_string();
    let _ = service
        .call(build_request(
            "initialize",
            json!({ "capabilities": {}, "workspaceFolders": [{ "uri": ws_uri, "name": "ws" }] }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    let readme_path = lsp_dir.join("readme.qmd.md");
    let readme_uri = Url::from_file_path(&readme_path).unwrap().to_string();
    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": readme_uri, "languageId": "qmd", "version": 1,
                    "text": fs::read_to_string(&readme_path).unwrap(),
                }
            }),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(400)).await;

    let flagged = broken_links(&captured, "lsp/readme.qmd.md").unwrap_or_default();
    assert!(
        flagged.is_empty(),
        "[[#lsp:completion]] resolves to lsp_completion.completion and must not be flagged; got: {flagged:?}"
    );
}
