//! Tests for UTF-16 position handling in the LSP server.
//!
//! Verifies that multi-byte characters (Cyrillic, emoji, CJK) don't cause panics
//! or incorrect positions when used in QMD documents.

use futures::StreamExt;
use qmdc::lsp::server::Backend;
use qmdc::lsp::{byte_offset_to_utf16_offset, utf16_offset_to_byte_offset};
use serde_json::json;
use tower::Service;
use tower_lsp::LspService;

// --- Unit tests for conversion helpers ---

#[test]
fn test_utf16_to_byte_ascii() {
    let line = "hello world";
    assert_eq!(utf16_offset_to_byte_offset(line, 0), 0);
    assert_eq!(utf16_offset_to_byte_offset(line, 5), 5);
    assert_eq!(utf16_offset_to_byte_offset(line, 11), 11);
    // Past end
    assert_eq!(utf16_offset_to_byte_offset(line, 100), line.len());
}

#[test]
fn test_utf16_to_byte_cyrillic() {
    // Cyrillic: each char is 2 bytes UTF-8, 1 UTF-16 unit
    // "Привет мир" = 9 Cyrillic chars + 1 space = 10 chars, 19 bytes, 10 UTF-16 units
    let line = "Привет мир";
    assert_eq!(line.len(), 19);
    assert_eq!(utf16_offset_to_byte_offset(line, 0), 0);
    assert_eq!(utf16_offset_to_byte_offset(line, 1), 2); // after 'П'
    assert_eq!(utf16_offset_to_byte_offset(line, 6), 12); // after 'Привет'
    assert_eq!(utf16_offset_to_byte_offset(line, 7), 13); // after space
    assert_eq!(utf16_offset_to_byte_offset(line, 10), 19); // end
}

#[test]
fn test_utf16_to_byte_emoji() {
    // 🎉 is U+1F389: 4 bytes UTF-8, 2 UTF-16 units (surrogate pair)
    let line = "a🎉b"; // byte offsets: a=0, 🎉=1..5, b=5
    assert_eq!(utf16_offset_to_byte_offset(line, 0), 0); // before 'a'
    assert_eq!(utf16_offset_to_byte_offset(line, 1), 1); // after 'a', before 🎉
    assert_eq!(utf16_offset_to_byte_offset(line, 3), 5); // after 🎉 (2 UTF-16 units), before 'b'
    assert_eq!(utf16_offset_to_byte_offset(line, 4), 6); // after 'b'
}

#[test]
fn test_utf16_to_byte_mixed() {
    // "- desc: Описание [[#ref]]"
    // ASCII prefix "- desc: " = 8 bytes, 8 UTF-16 units
    // Cyrillic "Описание" = 8 chars, 16 bytes, 8 UTF-16 units
    // ASCII " [[#ref]]" = 9 bytes, 9 UTF-16 units
    let line = "- desc: Описание [[#ref]]";
    // UTF-16 offset 8 = start of Cyrillic
    assert_eq!(utf16_offset_to_byte_offset(line, 8), 8);
    // UTF-16 offset 16 = after Cyrillic, before space
    assert_eq!(utf16_offset_to_byte_offset(line, 16), 24);
    // UTF-16 offset 17 = the space before [[
    assert_eq!(utf16_offset_to_byte_offset(line, 17), 25);
    // UTF-16 offset 19 = inside [[#
    assert_eq!(utf16_offset_to_byte_offset(line, 19), 27);
}

#[test]
fn test_byte_to_utf16_ascii() {
    let line = "hello world";
    assert_eq!(byte_offset_to_utf16_offset(line, 0), 0);
    assert_eq!(byte_offset_to_utf16_offset(line, 5), 5);
    assert_eq!(byte_offset_to_utf16_offset(line, 11), 11);
}

#[test]
fn test_byte_to_utf16_cyrillic() {
    let line = "Привет мир";
    assert_eq!(byte_offset_to_utf16_offset(line, 0), 0);
    assert_eq!(byte_offset_to_utf16_offset(line, 2), 1); // after 'П'
    assert_eq!(byte_offset_to_utf16_offset(line, 12), 6); // after 'Привет'
    assert_eq!(byte_offset_to_utf16_offset(line, 13), 7); // after space
    assert_eq!(byte_offset_to_utf16_offset(line, 19), 10); // end
}

#[test]
fn test_byte_to_utf16_emoji() {
    let line = "a🎉b";
    assert_eq!(byte_offset_to_utf16_offset(line, 0), 0); // before 'a'
    assert_eq!(byte_offset_to_utf16_offset(line, 1), 1); // after 'a'
    assert_eq!(byte_offset_to_utf16_offset(line, 5), 3); // after 🎉 (4 bytes → 2 UTF-16 units)
    assert_eq!(byte_offset_to_utf16_offset(line, 6), 4); // after 'b'
}

#[test]
fn test_roundtrip_utf16_byte() {
    let lines = [
        "hello",
        "Привет",
        "🎉🎊🎈",
        "- name: Тест [[#ref]]",
        "emoji 🚀 before [[#id]] ref",
    ];
    for line in lines {
        // For every valid char boundary, roundtrip should be identity
        for (byte_idx, _) in line.char_indices() {
            let utf16 = byte_offset_to_utf16_offset(line, byte_idx);
            let back = utf16_offset_to_byte_offset(line, utf16);
            assert_eq!(
                back, byte_idx,
                "Roundtrip failed for line {:?} at byte {}",
                line, byte_idx
            );
        }
        // Also test end of string
        let utf16_end = byte_offset_to_utf16_offset(line, line.len());
        let back_end = utf16_offset_to_byte_offset(line, utf16_end);
        assert_eq!(back_end, line.len());
    }
}

// --- Integration tests: LSP completion with non-ASCII ---

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

/// Helper: initialize LSP service and open a document, return the service ready for requests
async fn setup_lsp(
    content: &str,
) -> (
    tower_lsp::LspService<Backend>,
    tokio::task::JoinHandle<()>,
    String,
) {
    let (service, socket) = LspService::new(Backend::new);
    let handle = tokio::spawn(async move {
        let mut socket = socket;
        while (socket.next().await).is_some() {}
    });
    (service, handle, content.to_string())
}

async fn init_and_open(service: &mut tower_lsp::LspService<Backend>, uri: &str, content: &str) {
    let init = build_request("initialize", json!({"capabilities":{}}), 1);
    let _ = service.call(init).await;
    let initialized = build_notification("initialized", json!({}));
    let _ = service.call(initialized).await;
    let did_open = build_notification(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": "qmd",
                "version": 1,
                "text": content,
            }
        }),
    );
    let _ = service.call(did_open).await;
}

/// Regression test: completion must not panic on Cyrillic text
/// This is the exact scenario from the bug report.
#[tokio::test]
async fn test_completion_no_panic_cyrillic() {
    let content =
        "# Doc [[doc:__Workspace]]\n\n## Question [[q1]]\n\n- answer: В мне нравится, но тут [[#\n";
    let uri = "file:///test_cyrillic.qmd.md";

    let (mut service, _handle, _) = setup_lsp(content).await;
    init_and_open(&mut service, uri, content).await;

    // Request completion at various positions within the Cyrillic text
    // Line 4: "- answer: В мне нравится, но тут [[#"
    // The bug was: position.character used as byte index into UTF-8 string
    for col in [11, 13, 22, 30, 35] {
        let completion = build_request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 4, "character": col },
            }),
            2,
        );
        // Should not panic
        let response = service.call(completion).await;
        assert!(response.is_ok(), "Completion panicked at col {}", col);
    }
}

/// Regression test: completion must not panic on emoji
#[tokio::test]
async fn test_completion_no_panic_emoji() {
    let content = "# Doc [[doc:__Workspace]]\n\n## Item [[item1]]\n\n- note: 🎉🚀 see [[#\n";
    let uri = "file:///test_emoji.qmd.md";

    let (mut service, _handle, _) = setup_lsp(content).await;
    init_and_open(&mut service, uri, content).await;

    // Line 4: "- note: 🎉🚀 see [[#"
    // 🎉 = 2 UTF-16 units, 🚀 = 2 UTF-16 units
    // "- note: " = 8 UTF-16 units
    // After emojis: 8 + 2 + 2 = 12 UTF-16 units, then " see [[#" = 8 more = 20
    for col in [8, 10, 12, 15, 19, 20] {
        let completion = build_request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 4, "character": col },
            }),
            2,
        );
        let response = service.call(completion).await;
        assert!(response.is_ok(), "Completion panicked at col {}", col);
    }
}

/// Test that hover works correctly with Cyrillic text before a reference
#[tokio::test]
async fn test_hover_cyrillic_before_ref() {
    let content = "# Doc [[doc:__Workspace]]\n\n## Target [[target]]\n\n- desc: Описание\n\n## Source [[source]]\n\n- ref: [[#target]]\n";
    let uri = "file:///test_hover_cyr.qmd.md";

    let (mut service, _handle, _) = setup_lsp(content).await;
    init_and_open(&mut service, uri, content).await;

    // Line 8: "- ref: [[#target]]"
    // Hover on the reference — should find it and return hover info
    let hover = build_request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 8, "character": 12 },
        }),
        2,
    );
    let response = service.call(hover).await;
    assert!(response.is_ok());
    if let Some(resp) = response.unwrap() {
        let result = resp.result().unwrap();
        // Should have hover content (not null)
        assert!(!result.is_null(), "Hover should find the reference");
    }
}

/// Test that references are correctly positioned with emoji before them
#[tokio::test]
async fn test_reference_positions_with_emoji() {
    // 🎉 before the reference — tests that start_col/end_col are correct UTF-16 offsets
    let content = "# Doc [[doc:__Workspace]]\n\n## Target [[target]]\n\n- x: value\n\n## Source [[source]]\n\n- note: 🎉 see [[#target]]\n";
    let uri = "file:///test_ref_emoji.qmd.md";

    let (mut service, _handle, _) = setup_lsp(content).await;
    init_and_open(&mut service, uri, content).await;

    // Line 8: "- note: 🎉 see [[#target]]"
    // "- note: " = 8 UTF-16 units
    // "🎉" = 2 UTF-16 units
    // " see " = 5 UTF-16 units
    // "[[#target]]" starts at UTF-16 offset 15
    // Hover at offset 17 should be inside [[#target]]
    let hover = build_request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 8, "character": 17 },
        }),
        2,
    );
    let response = service.call(hover).await;
    assert!(response.is_ok());
    if let Some(resp) = response.unwrap() {
        let result = resp.result().unwrap();
        assert!(!result.is_null(), "Hover should find reference after emoji");
    }
}

/// Test that goto definition works with CJK characters before reference
#[tokio::test]
async fn test_goto_definition_cjk() {
    let content = "# Doc [[doc:__Workspace]]\n\n## 目標 [[target]]\n\n- name: target\n\n## ソース [[source]]\n\n- dep: [[#target]]\n";
    let uri = "file:///test_cjk.qmd.md";

    let (mut service, _handle, _) = setup_lsp(content).await;
    init_and_open(&mut service, uri, content).await;

    // Line 8: "- dep: [[#target]]"
    let def = build_request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 8, "character": 12 },
        }),
        2,
    );
    let response = service.call(def).await;
    assert!(response.is_ok());
    if let Some(resp) = response.unwrap() {
        let result = resp.result().unwrap();
        assert!(!result.is_null(), "GotoDefinition should resolve reference");
    }
}
