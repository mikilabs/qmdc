//! QMD-64 red test: LSP must not report a false QMDC001 for a cross-file
//! reference whose target was created on disk **without a delivered
//! `workspace/didChangeWatchedFiles` event** (external tool write, folder
//! `mv`, etc.).
//!
//! This is the sibling of `lsp_stale_diagnostics.rs` (QMD-58) but with the
//! crucial difference: QMD-58 *sends* the CREATED watched-file event and asserts
//! the reverse-invalidation clears the broken link. QMD-64 deliberately does
//! **not** send that event — the whole bug is that the editor/client fails to
//! deliver it for programmatic/batch file operations, so the LSP's cached
//! workspace index goes stale and diagnostics stay red until a server restart,
//! even though `qmdc workspace validate` reports 0 issues for the same state.
//!
//! Contract encoded here (mechanism-agnostic): once the target object exists on
//! disk, a natural re-evaluation of the already-open referencing file (a
//! `didChange`/`didSave` the author would trigger) must resolve the reference —
//! WITHOUT a watched-file notification and WITHOUT a restart.
//!
//! Expected to be RED against pre-fix code (the reference stays QMDC001 because
//! the target file was never indexed), and GREEN once the index is a
//! trustworthy projection of disk state.

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

/// Poll `latest_broken_link_count` until it equals `expected` or `timeout`
/// elapses, then return the last observed value. The native fs watcher + rescan
/// are asynchronous and, under parallel test load, can take noticeably longer
/// than a fixed sleep — polling keeps the test robust instead of flaky.
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

/// Latest QMDC001 ("not found") count published for the file whose URI ends with
/// `suffix`. Returns `None` if no diagnostics were ever published for that file.
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

#[tokio::test]
async fn qmd64_external_target_resolves_without_watched_file_event() {
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

    // Precondition: the bug's starting state — A has exactly one broken link.
    let before = wait_for_broken_count(&captured, "a.qmd.md", 1, Duration::from_secs(3)).await;
    assert_eq!(
        before,
        Some(1),
        "precondition: A should have 1 broken_link before b.qmd.md exists, got {:?}",
        before
    );

    // Step 2: create b.qmd.md on disk defining b_obj — the target now EXISTS on
    // disk (so `qmdc workspace validate` would return []). Crucially, DO NOT send
    // `workspace/didChangeWatchedFiles`: this models the missed-event reality of
    // an external tool write / folder `mv` that the client never forwards.
    let b_path = root.join("b.qmd.md");
    fs::write(&b_path, "## B [[b_obj: Thing]]\n").unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Step 3: the author naturally re-touches the open referencing file (a
    // keystroke / save). This re-evaluates A's diagnostics. Because b_obj now
    // exists on disk, A's [[#b_obj]] must resolve — no watched-file event, no
    // restart.
    let _ = service
        .call(build_notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": a_uri, "version": 2 },
                "contentChanges": [{
                    "text": "## A [[a_obj: Thing]]\n\n- depends: [[#b_obj]]\n\n<!-- touched -->\n"
                }]
            }),
        ))
        .await;

    // Poll (not a fixed sleep): the native watcher + rescan are async and slower
    // under parallel test load.
    let after = wait_for_broken_count(&captured, "a.qmd.md", 0, Duration::from_secs(10)).await;
    assert_eq!(
        after,
        Some(0),
        "QMD-64: after b.qmd.md is created on disk (no didChangeWatchedFiles \
         delivered), a natural re-edit of A must clear the stale QMDC001 \
         (got {:?}). The cached workspace index must reflect on-disk state \
         without depending on a client-delivered watch event.",
        after
    );
}
