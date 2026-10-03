//! GitHub #7: in a multi-root editor workspace every workspace in the
//! `qmdc.getWorkspaceTree` response must carry the absolute `projectRoot` of the
//! editor folder it was discovered in, so the client resolves its relative `file`
//! against the right folder instead of guessing folder 0.
//!
//! The QMD workspaces live one level below each editor folder (as in the bug
//! report), and the second folder's workspace is the one that broke.

use std::fs;
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

async fn tree(service: &mut LspService<Backend>, mode: &str, id: i64) -> Value {
    let response = service
        .call(build_request(
            "workspace/executeCommand",
            json!({ "command": "qmdc.getWorkspaceTree", "arguments": [".", mode] }),
            id,
        ))
        .await
        .expect("executeCommand")
        .expect("response");
    response.result().cloned().expect("result")
}

#[tokio::test]
async fn duplicate_workspace_id_entry_still_gets_a_project_root() {
    // The bug report's project held copies of the same experiment under `.tmp/`,
    // so one workspace id was declared in several places. A lookup by id alone is
    // ambiguous there, and an entry without `projectRoot` sends the client back
    // to folder 0.
    let temp = TempDir::new().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let first = base.join("first");
    let second = base.join("second");
    fs::create_dir_all(first.join("copy_a")).unwrap();
    fs::create_dir_all(second.join("copy_b")).unwrap();
    for dir in [first.join("copy_a"), second.join("copy_b")] {
        fs::write(
            dir.join("readme.qmd.md"),
            "# Typer [[typer_wiki: __Workspace]]\n\n## A [[a: Thing]]\n",
        )
        .unwrap();
    }

    let (mut service, socket) = LspService::new(Backend::new);
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(_msg) = socket.next().await {}
    });
    let folders = json!([
        { "uri": Url::from_file_path(&first).unwrap().to_string(), "name": "first" },
        { "uri": Url::from_file_path(&second).unwrap().to_string(), "name": "second" },
    ]);
    let _ = service
        .call(build_request(
            "initialize",
            json!({ "capabilities": {}, "workspaceFolders": folders }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = tree(&mut service, "namespace", 2).await;
    let workspaces = result["workspaces"].as_array().expect("workspaces");
    assert!(!workspaces.is_empty(), "no workspaces in {result}");
    // The index keeps one workspace per id (duplicate ids are a reported error, and
    // which copy wins is not what this test is about). Whatever entry is sent, its
    // `projectRoot` joined with its `file` must name a file that exists — that is the
    // path the client opens.
    for w in workspaces {
        let root = w
            .get("projectRoot")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("entry without projectRoot: {w}"));
        let file = w["file"].as_str().expect("file");
        let full = std::path::Path::new(root).join(file);
        assert!(
            full.is_file(),
            "{} does not exist (entry {w})",
            full.display()
        );
        assert!(
            root == first.to_string_lossy() || root == second.to_string_lossy(),
            "projectRoot {root} is not an editor folder"
        );
    }
}

#[tokio::test]
async fn every_tree_workspace_carries_its_own_project_root() {
    let temp = TempDir::new().unwrap();
    // Canonical: the server canonicalises folder paths (macOS /var -> /private/var).
    let base = temp.path().canonicalize().unwrap();
    let first = base.join("first");
    let second = base.join("second");
    fs::create_dir_all(first.join("docs")).unwrap();
    fs::create_dir_all(second.join("nested/wiki")).unwrap();
    fs::write(
        first.join("docs/readme.qmd.md"),
        "# First [[first_ws: __Workspace]]\n\n## A [[a: Thing]]\n",
    )
    .unwrap();
    fs::write(
        second.join("nested/wiki/readme.qmd.md"),
        "# Second [[second_ws: __Workspace]]\n\n## B [[b: Thing]]\n",
    )
    .unwrap();

    let (mut service, socket) = LspService::new(Backend::new);
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(_msg) = socket.next().await {}
    });

    let folders = json!([
        { "uri": Url::from_file_path(&first).unwrap().to_string(), "name": "first" },
        { "uri": Url::from_file_path(&second).unwrap().to_string(), "name": "second" },
    ]);
    let _ = service
        .call(build_request(
            "initialize",
            json!({ "capabilities": {}, "workspaceFolders": folders }),
            1,
        ))
        .await;
    let _ = service
        .call(build_notification("initialized", json!({})))
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let expected = [
        ("first_ws", &first, "docs/readme.qmd.md"),
        ("second_ws", &second, "nested/wiki/readme.qmd.md"),
    ];
    for (n, mode) in ["namespace", "file", "smart", "kind"].iter().enumerate() {
        let result = tree(&mut service, mode, 10 + n as i64).await;
        let workspaces = result
            .get("workspaces")
            .and_then(|w| w.as_array())
            .unwrap_or_else(|| panic!("mode {mode}: no `workspaces` in {result}"));
        for (id, root, file) in expected {
            let ws = workspaces
                .iter()
                .find(|w| w.get("id").and_then(|v| v.as_str()) == Some(id))
                .unwrap_or_else(|| panic!("mode {mode}: workspace {id} missing from {result}"));
            let project_root = ws
                .get("projectRoot")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("mode {mode}: {id} has no projectRoot: {ws}"));
            assert_eq!(
                project_root,
                root.to_string_lossy(),
                "mode {mode}: {id} projectRoot"
            );
            assert_eq!(
                ws.get("file").and_then(|v| v.as_str()),
                Some(file),
                "mode {mode}: {id} file must stay relative to projectRoot"
            );
        }
    }
}
