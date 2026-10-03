//! GitHub #10: `qmdc mcp -w A -w B` serves the composition of exactly those workspaces —
//! the same graph `qmdc query -w A -w B` sees — and `--force-root` refuses any `-w` path
//! outside its boundary at startup.
//!
//! A separate test binary because the `-w` set is a process-wide `OnceLock` (same reason
//! as `mcp_force_root.rs`); the in-process checks therefore share one configuration and
//! live in one test function.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, RunningService};
use rmcp::ServiceExt;
use serde_json::{json, Value};

use qmdc::mcp::server::configure_compose_with;
use qmdc::mcp::tools::QmdcServer;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Two unrelated workspaces: `w1` refers into `w2`, which sits one level deeper on disk.
fn two_workspaces(base: &Path) -> (PathBuf, PathBuf) {
    let w1 = base.join("one/w1");
    let w2 = base.join("elsewhere/deep/w2");
    write(
        &w1.join("readme.qmd.md"),
        "# W1 [[w1: __Workspace]]\n\n## X [[x: Thing]]\n\n- r: [[#w2::y]]\n",
    );
    write(
        &w2.join("readme.qmd.md"),
        "# W2 [[w2: __Workspace]]\n\n## Y [[y: Thing]]\n\n- note: target\n",
    );
    (w1, w2)
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

async fn call(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> (Value, bool) {
    let res = client
        .call_tool(CallToolRequestParam {
            name: name.to_string().into(),
            arguments: args.as_object().cloned(),
        })
        .await
        .expect("call_tool");
    let text = res
        .content
        .first()
        .and_then(|c| c.as_text())
        .unwrap()
        .text
        .clone();
    (
        serde_json::from_str(&text).unwrap(),
        res.is_error.unwrap_or(false),
    )
}

#[tokio::test]
async fn composed_server_answers_over_the_with_set() {
    let base = tempfile::tempdir().unwrap();
    let base_path = base.path().canonicalize().unwrap();
    let (w1, w2) = two_workspaces(&base_path);
    // A third workspace on disk that is NOT part of the composition.
    let stray = base_path.join("stray");
    write(&stray.join("readme.qmd.md"), "# S [[stray: __Workspace]]\n");

    configure_compose_with(None, vec![w1.clone(), w2.clone()]).expect("configure");
    let client = connect().await;
    let p1 = json!(w1.to_string_lossy());

    // Validation sees the cross-workspace edge resolved.
    let (v, err) = call(&client, "qmdc_validate_references", json!({ "path": p1 })).await;
    assert!(!err, "{v}");
    assert_eq!(v["count"], 0, "{v}");

    // A path inside EITHER member answers from the same graph.
    let sql = "SELECT __workspace, __id FROM objects WHERE __kind = 'Thing' ORDER BY __id";
    for p in [&w1, &w2] {
        let (q, err) = call(
            &client,
            "qmdc_query_sql",
            json!({ "path": p.to_string_lossy(), "sql": sql }),
        )
        .await;
        assert!(!err, "{q}");
        assert_eq!(
            q["items"],
            json!([{ "__workspace": "w1", "__id": "x" }, { "__workspace": "w2", "__id": "y" }]),
            "{q}"
        );
    }

    // __file is the composed (virtual) one, as in `workspace parse -w`.
    let (loc, err) = call(
        &client,
        "qmdc_locate_object",
        json!({ "path": p1, "ref": "y" }),
    )
    .await;
    assert!(!err, "{loc}");
    assert_eq!(loc["file"], "w2/readme.qmd.md", "{loc}");

    // Rename reads the referring line from disk through the virtual path.
    let (ren, err) = call(
        &client,
        "qmdc_rename_object",
        json!({ "path": p1, "old_id": "y", "new_id": "z" }),
    )
    .await;
    assert!(!err, "{ren}");
    let edits = ren["edits"].as_array().unwrap();
    let ref_edit = edits
        .iter()
        .find(|e| e["file"] == "w1/readme.qmd.md")
        .unwrap_or_else(|| panic!("no edit in w1: {ren}"));
    assert_eq!(ref_edit["old_text"], "[[#w2::y]]", "{ren}");
    // The definition edit carries the full anchor read from disk; without the read it would
    // fall back to the bare `[[y]]`.
    let def_edit = edits
        .iter()
        .find(|e| e["file"] == "w2/readme.qmd.md")
        .unwrap_or_else(|| panic!("definition edit missing: {ren}"));
    assert_eq!(def_edit["old_text"], "[[y: Thing]]", "{ren}");

    // Validation scoped to a composed file works; a stray file is refused.
    let (scoped, err) = call(
        &client,
        "qmdc_validate_references",
        json!({ "path": p1, "file": "w1/readme.qmd.md" }),
    )
    .await;
    assert!(!err, "{scoped}");
    let (bad, err) = call(
        &client,
        "qmdc_validate_references",
        json!({ "path": p1, "file": "readme.qmd.md" }),
    )
    .await;
    assert!(err, "{bad}");
    assert_eq!(bad["error"]["code"], "out-of-root", "{bad}");

    // The dump reports the base as virtual.
    let (dump, err) = call(&client, "qmdc_dump_index", json!({ "path": p1 })).await;
    assert!(!err, "{dump}");
    assert!(dump["root"].is_null(), "{dump}");

    // A path outside every member is refused, not answered from the composed graph.
    let (out, err) = call(
        &client,
        "qmdc_get_tree",
        json!({ "path": stray.to_string_lossy() }),
    )
    .await;
    assert!(err, "{out}");
    assert_eq!(out["error"]["code"], "out-of-root", "{out}");
}

/// Run `qmdc mcp <args>` with stdin closed; returns (exit code, stderr).
fn run_mcp(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_qmdc"))
        .arg("mcp")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("spawn qmdc");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn startup_refuses_bad_with_sets() {
    let base = tempfile::tempdir().unwrap();
    let base_path = base.path().canonicalize().unwrap();
    let (w1, w2) = two_workspaces(&base_path);
    let s1 = w1.to_string_lossy().into_owned();
    let s2 = w2.to_string_lossy().into_owned();
    let empty = base_path.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let dup = base_path.join("dup");
    write(&dup.join("readme.qmd.md"), "# Again [[w1: __Workspace]]\n");

    let fr = base_path.join("one").to_string_lossy().into_owned();
    let cases: &[(&[&str], &str)] = &[
        // --force-root: w2 is outside `one/`.
        (
            &["--force-root", &fr, "-w", &s1, "-w", &s2],
            "outside --force-root",
        ),
        (&["-w", &empty.to_string_lossy()], "workspace"),
        (&["-w", &s1, "-w", &s1], "given twice"),
        (
            &["-w", &s1, "-w", &dup.to_string_lossy()],
            "same workspace id",
        ),
    ];
    for (args, needle) in cases {
        let (code, stderr) = run_mcp(args);
        assert_eq!(code, 2, "{args:?}: exit {code}, stderr: {stderr}");
        assert!(
            stderr.contains(needle),
            "{args:?}: stderr lacks {needle:?}: {stderr}"
        );
    }

    // Inside the force-root the same flags start the server (stdin closed -> it ends cleanly).
    let (code, stderr) = run_mcp(&[
        "--force-root",
        &base_path.to_string_lossy(),
        "-w",
        &s1,
        "-w",
        &s2,
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
}
