//! Simple LSP test to debug hanging

use futures::StreamExt;
use qmdc::lsp::server::Backend;
use serde_json::json;
use tower::Service;
use tower_lsp::LspService;

fn build_request(
    method: &'static str,
    params: serde_json::Value,
    id: i64,
) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .id(id)
        .finish()
}

fn build_notification(
    method: &'static str,
    params: serde_json::Value,
) -> tower_lsp::jsonrpc::Request {
    tower_lsp::jsonrpc::Request::build(method)
        .params(params)
        .finish()
}

#[tokio::test]
async fn test_multi_step() {
    println!("1. Creating service...");
    let (mut service, socket) = LspService::new(Backend::new);

    // Spawn task to drain socket messages (diagnostics, etc.)
    tokio::spawn(async move {
        let mut socket = socket;
        while let Some(msg) = socket.next().await {
            println!("   [socket] received: {:?}", msg);
        }
    });

    println!("2. Initialize...");
    let init = build_request("initialize", json!({"capabilities":{}}), 1);
    let _ = service.call(init).await;
    println!("   OK");

    println!("3. Initialized notification...");
    let initialized = build_notification("initialized", json!({}));
    let _ = service.call(initialized).await;
    println!("   OK");

    // Test references/003 scenario
    let content = "# Users [[users]]\n\nSee [[users]] for details.\n";

    println!("4. didOpen...");
    let did_open = build_notification(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": "file:///test.qmd.md",
                "languageId": "qmd",
                "version": 1,
                "text": content,
            }
        }),
    );
    let _ = service.call(did_open).await;
    println!("   OK");

    println!("5. References with includeDeclaration=true on definition...");
    let refs = build_request(
        "textDocument/references",
        json!({
            "textDocument": { "uri": "file:///test.qmd.md" },
            "position": { "line": 0, "character": 12 },
            "context": { "includeDeclaration": true },
        }),
        2,
    );
    let response = service.call(refs).await;
    println!("   References Response: {:?}", response);

    println!("DONE!");
}
