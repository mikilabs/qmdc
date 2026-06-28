# Quickstart [[quickstart: Tutorial]]

- goal: go from zero to a working QMDC graph in five minutes
- audience: newcomer
- time: 5m
- outcome: a parsed, validated, and queryable workspace
- about: [[#object]], [[#field]], [[#heading]], [[#reference]], [[#data_type]]
- next: [[#guide_first_file]]

## Content Generator [[quickstart_gen: ContentGenerator]]

- target: [[#quickstart.content]]
- about: [[#object]], [[#field]], [[#heading]], [[#reference]], [[#data_type]]
- sources_hash: 27078aa4723f0ce1

### Prompt [[quickstart_gen_prompt: text]]

Write a quickstart that gets someone from zero to working QMDC in 5 minutes. This is a DO page, not a READ page.

Structure:

1. **Install** — `cargo install qmdc` or `uv pip install qmdc` or `npm install -g @qmdc/qmdc` (show all three, let them pick)
2. **Create a file** — literally "create `hello.qmd.md` with this content:" and show a 5-line example
3. **Parse it** — `qmdc parse -i hello.qmd.md` — show the command and what it outputs
4. **Validate** — `qmdc parse -i hello.qmd.md > /dev/null && echo "valid"`
5. **Create a workspace** — add a second file, add a reference between them, run `qmdc workspace validate .`
6. **Query** — `qmdc query . "SELECT __id, __kind FROM objects"` — show the output
7. **Hand it to an agent (MCP)** — `qmdc mcp --force-root .` exposes the same graph to AI agents over the Model Context Protocol; show a minimal MCP client config

Every step: command to run, expected output. Copy-paste friendly.
No theory. No "what is QMDC". No "core concepts". Just do the thing.
End with: "Next: read Why QMDC to understand the philosophy, or jump to the Guides for specific tasks."

At the very top of the content (before "Install"), embed the demo recording:
`![QMDC quickstart — create a .qmd.md file, parse it, validate, and query the graph](../.assets/quickstart.gif)`

## Content [[content: text]]

![QMDC quickstart — create a .qmd.md file, parse it, validate, and query the graph](../.assets/quickstart.gif)

## 1. Install

Pick your package manager:

```bash
# Rust
cargo install qmdc

# Python
uv pip install qmdc

# Node.js
npm install -g @qmdc/qmdc
```

Confirm it's working:

```bash
qmdc --version
```

## 2. Create a file

Create `hello.qmd.md`:

```qmd
## Server [[server: Config]]

- host: localhost
- port: 8080
- debug: true
```

A [[#heading]] with `[[id: Kind]]` creates an [[#object]]. Bullet items with `- key: value` become [[#field]]s. Values are auto-typed — `8080` is a number, `true` is a boolean ([[#data_type]]).

## 3. Parse it

```bash
qmdc parse -i hello.qmd.md --pretty
```

Output:

```json
[
  {
    "__id": "server",
    "__label": "Server",
    "__kind": "Config",
    "host": "localhost",
    "port": 8080,
    "debug": true
  }
]
```

## 4. Validate

```bash
qmdc parse -i hello.qmd.md > /dev/null && echo "valid"
```

Output:

```text
valid
```

If there are syntax errors, you'll see them on stderr and a non-zero exit code.

## 5. Create a workspace

Add a second file, `services.qmd.md`:

```qmd
## API [[api: Service]]

- port: 3000
- config: [[#server]]
```

The [[#reference]] `[[#server]]` creates a typed edge from `api` to `server` with edge type `config`.

Mark the directory as a workspace with `readme.qmd.md`:

```qmd
# My Project [[my_project: __Workspace]]
```

Validate the workspace — this checks that all cross-file references resolve:

```bash
qmdc workspace validate .
```

Output (empty array = no errors):

```json
[]
```

## 6. Query

```bash
qmdc query . "SELECT __id, __kind FROM objects"
```

Output:

```text
__id        | __kind
------------|------------
my_project  | __Workspace
server      | Config
api         | Service
```

Query the edges (relationships between objects):

```bash
qmdc query . "SELECT source_id, target_id, edge_type FROM edges"
```

Output:

```text
source_id | target_id | edge_type
----------|-----------|----------
api       | server    | config
```

## 7. Hand it to your agent (MCP)

The same graph is available to AI agents over the [Model Context Protocol](https://modelcontextprotocol.io/). Start the server (scoped to the current directory):

```bash
qmdc mcp --force-root .
```

Then point an MCP client (Claude Desktop, Cursor, Kiro, …) at it:

```json
{
  "mcpServers": {
    "qmdc": {
      "command": "qmdc",
      "args": ["mcp", "--force-root", "/path/to/your/project"]
    }
  }
}
```

The agent can now search objects, run read-only SQL, walk the reference graph, and preview renames — the same intelligence your editor uses, no prose pasting.

---

Next: read [[#guide_first_file]] for a detailed walkthrough with explanations, or jump to the Guides for specific tasks.
