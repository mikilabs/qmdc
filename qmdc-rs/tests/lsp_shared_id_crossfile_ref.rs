//! Regression test: a cross-file reference must resolve even when the OPEN file
//! shares an object id with another file.
//!
//! Bug (QMD-64 follow-up): `compute_diagnostics` builds its resolution index as
//! `(whole workspace − open file's own objects) ∪ (freshly parsed open doc)`.
//! It identified "the open file's own objects" by looking the open file's ids up
//! in `ws.objects` and reading `.first().__file`. But ids are NOT unique across
//! files — child objects like `### Description [[description: text]]` appear in
//! many files — so for a shared id the lookup returned some OTHER file's copy and
//! `open_file` resolved to the WRONG file. The filter then dropped that other
//! file's objects from the index, so every reference to an object defined there
//! reported a false QMDC001 "not found" (while hover and `qmdc workspace
//! validate` resolved it fine, since they don't use this filter).
//!
//! Repro shape (mirrors the real workspace that surfaced it). `a_core.qmd.md`
//! defines `core_widget` plus a shared-id heading `[[shared: Topic]]`.
//! `b_open.qmd.md` (the open file) also defines `[[shared: Topic]]` as its first
//! object and then references `[[#core_widget]]`. `a_core` sorts/indexes before
//! `b_open`, so the shared id's first copy belongs to `a_core`; the old code
//! therefore mis-identified `b_open`'s file as `a_core.qmd.md` and dropped
//! `core_widget` from the index, yielding a false QMDC001. The fix derives
//! `open_file` from the URI, so `core_widget` stays resolvable.

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

fn latest_broken_link_count(captured: &Arc<Mutex<Vec<Value>>>, suffix: &str) -> Option<usize> {
    let diags = captured.lock().unwrap();
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
                    let code = d.get("code").and_then(|c| c.as_str()).unwrap_or("");
                    code == "QMDC001" || msg.contains("not found")
                })
                .count();
            return Some(broken);
        }
    }
    None
}

async fn wait_for_broken_count(
    captured: &Arc<Mutex<Vec<Value>>>,
    suffix: &str,
    expected: usize,
    timeout: Duration,
) -> Option<usize> {
    let start = std::time::Instant::now();
    loop {
        let cur = latest_broken_link_count(captured, suffix);
        if cur == Some(expected) || start.elapsed() >= timeout {
            return cur;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn crossfile_ref_resolves_when_open_file_shares_an_id() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();

    fs::write(
        root.join("readme.qmd.md"),
        "# Demo Workspace [[demo_ws:__Workspace]]\n",
    )
    .unwrap();

    // a_core.qmd.md — indexed first (sorts before b_open). Defines the shared id
    // `shared` and the cross-file target `core_widget`.
    fs::write(
        root.join("a_core.qmd.md"),
        "## Shared Heading [[shared: Topic]]\n\nintro text\n\n## Core Widget [[core_widget: Widget]]\n\n- note: hi\n",
    )
    .unwrap();

    // b_open.qmd.md — the OPEN file. Its FIRST object reuses the shared id
    // `shared`, then it references the cross-file target `core_widget`.
    let b_path = root.join("b_open.qmd.md");
    fs::write(
        &b_path,
        "## Shared Heading [[shared: Topic]]\n\nother text\n\n## Open Widget [[open_widget: Widget]]\n\n- depends: [[#core_widget]]\n",
    )
    .unwrap();
    let b_uri = Url::from_file_path(&b_path).unwrap().to_string();

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
    let _ = service
        .call(build_request(
            "initialize",
            json!({
                "capabilities": {},
                "workspaceFolders": [{ "uri": ws_uri, "name": "demo" }]
            }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;

    // Open b_open.qmd.md; its `[[#core_widget]]` targets an object defined in
    // a_core.qmd.md. `qmdc workspace validate` on this workspace is clean.
    let b_content = fs::read_to_string(&b_path).unwrap();
    let _ = service
        .call(build_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": b_uri,
                    "languageId": "qmd",
                    "version": 1,
                    "text": b_content,
                }
            }),
        ))
        .await;

    let broken = wait_for_broken_count(&captured, "b_open.qmd.md", 0, Duration::from_secs(3)).await;
    assert_eq!(
        broken,
        Some(0),
        "cross-file ref [[#core_widget]] must resolve even though b_open shares the \
         id `shared` with a_core; got {:?} broken link(s). A shared id must not \
         cause the target file's objects to be dropped from the resolution index.",
        broken
    );
}
