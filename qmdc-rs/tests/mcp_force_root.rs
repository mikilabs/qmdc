//! Force-root boundary tests (INV-1 enforcement at the MCP layer).
//!
//! These live in a SEPARATE test binary from `mcp_tests.rs` on purpose: `set_force_root`
//! installs a process-wide `OnceLock`, so it must be isolated from the fixture tests that
//! operate on temp dirs outside any configured root.

use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, RunningService};
use rmcp::ServiceExt;
use serde_json::{json, Value};

use qmdc::core::index_seam::set_force_root;
use qmdc::mcp::tools::QmdcServer;

/// Write a minimal workspace (a `readme.qmd.md` declaring `__Workspace`) into `dir`.
fn write_workspace(dir: &std::path::Path) {
    let readme = "# Root [[root: __Workspace]]\n\n## Note [[note:Note]]\n- text: hello\n";
    std::fs::write(dir.join("readme.qmd.md"), readme).unwrap();
}

async fn connect() -> RunningService<RoleClient, ()> {
    let (client_io, server_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        if let Ok(server) = QmdcServer::new().serve(server_io).await {
            let _ = server.waiting().await;
        }
    });
    ().serve(client_io).await.expect("client failed to connect")
}

/// Call a tool, returning `(inner_payload_json, is_error)`.
async fn call_tool(
    client: &RunningService<RoleClient, ()>,
    name: &str,
    args: Value,
) -> (Value, bool) {
    let res = client
        .call_tool(CallToolRequestParam {
            name: name.to_string().into(),
            arguments: args.as_object().cloned(),
        })
        .await
        .expect("call_tool");
    let is_error = res.is_error.unwrap_or(false);
    let text = res
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap();
    (serde_json::from_str(&text).unwrap(), is_error)
}

#[tokio::test]
async fn force_root_allows_inside_and_rejects_outside() {
    // Two independent workspaces; only `inside` is the configured force-root.
    let inside = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write_workspace(inside.path());
    write_workspace(outside.path());

    // Canonicalize to match the seam's canonicalization (macOS /var -> /private/var).
    let inside_root = inside.path().canonicalize().unwrap();
    let outside_root = outside.path().canonicalize().unwrap();

    // Configure the process-wide boundary (set-once; this binary is dedicated to it).
    set_force_root(inside_root.clone());

    let client = connect().await;

    // Inside the force-root -> success.
    let (inside_payload, inside_err) = call_tool(
        &client,
        "qmdc_get_tree",
        json!({ "path": inside_root.to_string_lossy() }),
    )
    .await;
    assert!(
        !inside_err,
        "in-root call should not be an error: {inside_payload}"
    );
    assert_eq!(
        inside_payload["success"], true,
        "in-root call should succeed: {inside_payload}"
    );

    // Outside the force-root -> fail-closed out-of-root, even though `outside` is itself
    // a valid workspace.
    let (outside_payload, outside_err) = call_tool(
        &client,
        "qmdc_get_tree",
        json!({ "path": outside_root.to_string_lossy() }),
    )
    .await;
    assert!(
        outside_err,
        "out-of-root call must be an error: {outside_payload}"
    );
    assert_eq!(outside_payload["success"], false);
    assert_eq!(
        outside_payload["error"]["code"], "out-of-root",
        "expected out-of-root code: {outside_payload}"
    );

    let _ = client.cancel().await;
}
