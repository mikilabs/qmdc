#!/usr/bin/env bash
# QMDC quickstart demo — CONTENT ONLY. Run via:  scripts/demos/demo.sh all quickstart
# The runner sources scripts/demos/demo.sh first, so the helpers (note / run_live /
# run_shown / mcp_request / mcp_capture / qmdc) are already available here.
# This script and its artifacts share a stem in docs/.assets/:
#   quickstart.demo.sh → quickstart.cast → quickstart.gif → quickstart.mp4
#
# Story: describe a system in plain Markdown, then answer the question that matters
# before any change — "if I touch this, what's affected?" — via SQL and via MCP.

cd "$(mktemp -d)"
echo '# Online Store [[store: __Workspace]]' > readme.qmd.md
cat > services.qmd.md <<'EOF'
## Database [[database: Resource]]

- engine: PostgreSQL

## Checkout [[checkout: Service]]

- uses: [[#database]]

## Search [[search: Service]]

- uses: [[#database]]

## Recommendations [[recs: Service]]

- uses: [[#database]]
EOF

# MCP calls an AI agent would make — pre-captured so the recording stays instant.
mcp_request qmdc_find_references "{\"path\":\"$PWD\",\"id\":\"database\"}" impact.json
mcp_request qmdc_get_tree "{\"path\":\"$PWD\"}" tree.json
IMPACT_OUT="$(mcp_capture impact.json '.references | map(.id)')"
TREE_OUT="$(mcp_capture tree.json '[.nodes[] | {kind, label}]' | head -13)"

note "Describe a system in plain Markdown — three services share one database:"
run_live "cat services.qmd.md"

note "Instantly a queryable graph. List every object, like a database:"
run_live 'qmdc query . "SELECT __id, __kind FROM objects"'

note "The question before any change: touch the database — what breaks?"
run_live 'qmdc query . "SELECT source_id FROM edges WHERE target_id = '\''store::database'\''"'

note "Your AI agent asks the same, over MCP (stdio JSON-RPC):"
run_shown "qmdc mcp < impact.json | jq '… .references | map(.id)'" "$IMPACT_OUT"

note "…or pulls the whole structure as a tree (head, so it fits):"
run_shown "qmdc mcp < tree.json | jq '… [.nodes[] | {kind, label}]' | head -13" "$TREE_OUT"

note "Plain Markdown in. Real answers about your system out."
sleep 3
