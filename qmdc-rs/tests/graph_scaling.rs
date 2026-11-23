//! QMD-86 (GitHub #17): `traverse` and `find_path` must run in O(V + E) per call.
//!
//! Before the fix both operations scanned every edge for each node they dequeued (and found
//! each node with a linear search), so a 30k-object workspace took seconds in a release build
//! and minutes in a debug one. The workspace below is the one from the issue: a ring of `n`
//! objects, each with three references (`next`, `a`, `b`), so the whole graph is reachable.
//!
//! The index is built in memory, without parsing, so the time measured is the graph
//! operation alone. The bound is generous on purpose: linear work on this graph takes well
//! under a second even unoptimized, while the quadratic version takes minutes.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use qmdc::core::ops::{find_path, traverse};
use qmdc::core::resolved_index::ResolvedIndex;
use qmdc::db::QmdcDatabase;
use qmdc::workspace::WorkspaceResult;

const N: usize = 30_000;
const BOUND: Duration = Duration::from_secs(5);

fn ring_index(n: usize) -> ResolvedIndex {
    let objects: Vec<Value> = (0..n)
        .map(|i| {
            json!({
                "__id": format!("f{i}"),
                "__kind": "__Object",
                "__namespace": "",
                "__file": format!("part{:04}.qmd.md", i / 500),
                "__line": (i % 500) as i64 * 6 + 1,
                "next": format!("[[#f{}]]", (i + 1) % n),
                "a": format!("[[#f{}]]", (i * 7 + 3) % n),
                "b": format!("[[#f{}]]", (i * 13 + 5) % n),
            })
        })
        .collect();
    ResolvedIndex {
        root: PathBuf::from("/graph-scaling"),
        workspace: WorkspaceResult {
            root: Some("/graph-scaling".to_string()),
            workspace_id: None,
            workspaces: vec![],
            files: vec![],
            objects,
            errors: vec![],
        },
        db: QmdcDatabase::new().expect("in-memory db"),
        built_at: Instant::now(),
        file_count: 0,
    }
}

#[test]
fn traverse_whole_graph_is_linear() {
    let index = ring_index(N);
    let t = Instant::now();
    let out = traverse(&index, "f0", None, "outgoing", 50).expect("traverse succeeds");
    let elapsed = t.elapsed();
    assert_eq!(out["success"], json!(true));
    // depth 50 from f0 reaches every node of this graph; the envelope reports the rest.
    let shown = out["items"].as_array().map_or(0, |a| a.len());
    let remaining = out["remaining"].as_u64().unwrap_or(0) as usize;
    assert_eq!(shown + remaining, N, "traverse must visit the whole graph");
    assert!(
        elapsed < BOUND,
        "traverse over {N} objects took {elapsed:?} (bound {BOUND:?})"
    );
}

#[test]
fn find_path_across_graph_is_linear() {
    let index = ring_index(N);
    let t = Instant::now();
    let out = find_path(&index, "f0", &format!("f{}", N / 2), None).expect("path exists");
    let elapsed = t.elapsed();
    assert_eq!(out["success"], json!(true));
    assert!(out["length"].as_u64().unwrap_or(0) > 0);
    assert!(
        elapsed < BOUND,
        "find_path over {N} objects took {elapsed:?} (bound {BOUND:?})"
    );
}
