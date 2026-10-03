# Server [[mcp_server: Category]]

Cross-cutting behavior of the qmdc MCP server: how it talks, how it stays safe, and the shape of every response. Run it with `qmdc mcp`.

## Protocol [[mcp_protocol: NarrativeDoc]]

JSON-RPC 2.0 over newline-delimited messages. The handshake is `initialize` → `initialized`, then `tools/list`, `tools/call`, `resources/list`, `resources/read`, and `shutdown`. A message with no `method` returns `-32600 Invalid Request`; unparseable JSON returns `-32700`; an unknown method returns `-32601`. Tool and resource failures are returned as data (see [[#mcp_error_envelope]]), not as JSON-RPC errors.

## Transport [[mcp_transport: NarrativeDoc]]

Production transport is stdio: requests on stdin, responses on stdout, one JSON object per line. Tests use an in-process channel against the same loop. Blank lines are skipped, and a single line larger than 64 MiB is rejected as protocol abuse rather than buffered unboundedly.

## Security and Force-Root [[mcp_security: NarrativeDoc]]

By default the server trusts the `path` each caller supplies (the local single-user stdio model). Start it with `qmdc mcp --force-root <DIR>` to install a fail-closed boundary (INV-1): both the request `path` and the resolved workspace root must canonicalize inside `<DIR>`, otherwise the call is rejected with `out-of-root`.

## Explicit Composition [[mcp_compose_with: NarrativeDoc]]

`qmdc mcp -w <A> -w <B> …` serves the composition of exactly those workspaces, wherever they are on disk — the same graph `qmdc query -w <A> -w <B>` and `qmdc workspace validate -w …` see. Every tool and resource then answers over that one graph, and its `path` argument must lie inside one of the composed workspaces; any other path is rejected with `out-of-root` rather than answered from a graph it is not part of. As in `workspace parse -w`, the base is virtual: each workspace's files appear in `__file` under its id (`w2/readme.qmd.md`), `qmdc_dump_index` reports `root: null`, and a `file` scope is one of those `__file` values.

The `-w` set is checked once at startup with the CLI's rules, and a refused set exits with code 2 before the server starts: a path resolving to zero or several workspaces, the same path twice, two paths declaring the same workspace id, and — with `--force-root` — any `-w` path, or the workspace it resolves to, outside the boundary.

SQL is read-only (INV-2, see [[#tool_query_sql]]). A panic while handling one request is caught and converted into an `internal-error` response, so a single bad request cannot take the server down.

## Bounded Output [[mcp_bounded: NarrativeDoc]]

List-producing results are bounded so a huge workspace can't blow up a response (NFR-4). The default cap is 200 items; the envelope carries `truncated` and, when truncated, `remaining`, alongside the domain key (`items` / `references` / `edits` / …). `qmdc_get_tree` and `qmdc_describe_metamodel` are nested/aggregate and bounded instead by the NFR-2 file-count cap.

## Error Envelope [[mcp_error_envelope: NarrativeDoc]]

Every failure shares one shape: `{ "success": false, "error": { "code": "...", "message": "..." } }`. Tool failures are returned as an error-as-data content block with `isError: true`; resource failures return the same envelope as `application/json`. Codes are stable and content-free — e.g. `invalid-argument`, `not-found`, `not-resolved`, `ambiguous`, `out-of-root`, `not-read-only`, `reparse-bound-exceeded`, `internal-error` — so clients and tests can match on them across versions. `ambiguous` is returned when `path` contains two workspaces declaring the **same** workspace id, which no composed graph can represent; it additionally carries a `candidates` array of the workspace roots found, so the caller can see which two collided (see [[#mcp.mcp_path_anchor]]). A `path` holding several workspaces with distinct ids is **composed** into one graph and succeeds — re-invoking with a single candidate is neither required nor correct, because no candidate alone can see its siblings' objects.
