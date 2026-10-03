//! GitHub #6: a change to `.qmdcignore` must reach the running LSP. Before the fix
//! both event paths — the client's `workspace/didChangeWatchedFiles` and the
//! server's own fs watcher — dropped every path that is not `*.qmd.md`, so the
//! ignore set read at start-up survived until a restart.

use std::fs;
use std::time::{Duration, Instant};

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

async fn workspace_ids(service: &mut LspService<Backend>, id: i64) -> Vec<String> {
    let response = service
        .call(build_request(
            "workspace/executeCommand",
            json!({ "command": "qmdc.getWorkspaceTree", "arguments": [".", "namespace"] }),
            id,
        ))
        .await
        .expect("executeCommand")
        .expect("response");
    let mut ids: Vec<String> = response.result().cloned().expect("result")["workspaces"]
        .as_array()
        .expect("workspaces")
        .iter()
        .filter_map(|w| w["id"].as_str().map(str::to_string))
        .collect();
    ids.sort();
    ids
}

/// A project with a real workspace in `docs/` and a fixture workspace in `tests/`,
/// served by a fresh Backend. Returns the temp dir (keep it alive), the root and the service.
async fn start() -> (TempDir, std::path::PathBuf, LspService<Backend>) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join("tests/fixture")).unwrap();
    fs::write(
        root.join("docs/readme.qmd.md"),
        "# Docs [[docs_ws: __Workspace]]\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/fixture/readme.qmd.md"),
        "# Fixture [[fixture_ws: __Workspace]]\n",
    )
    .unwrap();

    let (mut service, socket) = LspService::new(Backend::new);
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(_msg) = socket.next().await {}
    });
    let uri = Url::from_file_path(&root).unwrap().to_string();
    let _ = service
        .call(build_request(
            "initialize",
            json!({ "capabilities": {}, "workspaceFolders": [{ "uri": uri, "name": "p" }] }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    (temp, root, service)
}

#[tokio::test]
async fn client_notification_for_qmdcignore_rescans() {
    let (_temp, root, mut service) = start().await;
    assert_eq!(
        workspace_ids(&mut service, 2).await,
        ["docs_ws", "fixture_ws"]
    );

    fs::write(root.join(".qmdcignore"), "tests/**\n").unwrap();
    let ignore_uri = Url::from_file_path(root.join(".qmdcignore")).unwrap();
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": [{ "uri": ignore_uri.to_string(), "type": 1 }] }),
        ))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(workspace_ids(&mut service, 3).await, ["docs_ws"]);

    // Editing it back (CHANGED) brings the fixture back.
    fs::write(root.join(".qmdcignore"), "# nothing ignored\n").unwrap();
    let _ = service
        .call(build_notification(
            "workspace/didChangeWatchedFiles",
            json!({ "changes": [{ "uri": ignore_uri.to_string(), "type": 2 }] }),
        ))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        workspace_ids(&mut service, 4).await,
        ["docs_ws", "fixture_ws"]
    );
}

#[tokio::test]
async fn native_watcher_picks_up_qmdcignore_without_a_client_event() {
    // The server's own watcher must react too: clients drop file events for
    // programmatic writes (QMD-64), and this one sends no notification at all.
    let (_temp, root, mut service) = start().await;
    assert_eq!(
        workspace_ids(&mut service, 2).await,
        ["docs_ws", "fixture_ws"]
    );

    fs::write(root.join(".qmdcignore"), "tests/**\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut id = 3;
    loop {
        let ids = workspace_ids(&mut service, id).await;
        if ids == ["docs_ws"] {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "fixture_ws still listed 10 s after .qmdcignore was written: {ids:?}"
        );
        id += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
