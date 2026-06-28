# QMDC Parser (Rust)

QMDC ↔ JSON parser for Markdown with lossless round-trip support and an LSP server.

## Build

```bash
cd qmdc-rs
cargo build --release
```

## CLI Usage

### Parse (QMD.md → JSON)

```bash
# File → stdout
./target/release/qmdc parse -i doc.qmd.md

# Stdin → stdout
echo "## Test [[test]]" | ./target/release/qmdc parse

# File → file
./target/release/qmdc parse -i doc.qmd.md -o output.json

# Output formats: minimal, standard (default), full
./target/release/qmdc parse -i doc.qmd.md --format full

# Compact JSON
./target/release/qmdc parse -i doc.qmd.md --no-pretty
```

### Rebuild (JSON → QMD.md)

```bash
# File → stdout
./target/release/qmdc rebuild -i data.json

# Stdin → stdout
echo '[{"__id":"test","__label":"Test","__level":2}]' | ./target/release/qmdc rebuild

# File → file
./target/release/qmdc rebuild -i data.json -o doc.qmd.md
```

### LSP Server

```bash
# Start the Language Server (stdio mode)
./target/release/qmdc lsp
```

Supported capabilities:

- `textDocument/completion` — reference autocompletion
- `textDocument/hover` — object information
- `textDocument/definition` — go to definition
- `textDocument/references` — find all references
- `textDocument/publishDiagnostics` — broken links, duplicate IDs
- `textDocument/documentSymbol` — structure outline
- `textDocument/rename` — rename objects
- `textDocument/prepareRename` — prepare for rename
- `workspace/symbol` — search symbols across the whole workspace

### LSP Debug

Test any LSP command without starting the full server (stateless mode):

```bash
# Format: JSON command via stdin or --command
echo '{"command":"qmdc.getWorkspaceTree","arguments":[".","smart"]}' | \
  ./target/release/qmdc lsp-debug ./my-project

# Or via an argument
./target/release/qmdc lsp-debug ./my-project \
  --command '{"command":"qmdc.getWorkspaceTree","arguments":[".","smart"]}'

# Example commands:
# - qmdc.getWorkspaceTree (modes: namespace, kind, file, smart)
# - qmdc.runSqlQuery (SQL queries against a workspace)
# - any other LSP custom command

# With jq for pretty output
echo '{"command":"qmdc.getWorkspaceTree","arguments":[".","smart"]}' | \
  ./target/release/qmdc lsp-debug ./my-project | jq '.workspaces[0].objects'
```

**Command format:** a JSON object with `command` (string) and `arguments` (array) fields.

### Workspace

```bash
# Parse a workspace into JSON
./target/release/qmdc workspace parse ./my-project

# Write to a file (via redirection)
./target/release/qmdc workspace parse ./my-project > workspace.json

# Choose a format (minimal, standard, full)
./target/release/qmdc workspace parse ./my-project --format full
```

### Query (SQL queries against a workspace)

```bash
# SQL query against a workspace (loads all workspaces recursively)
./target/release/qmdc query ./my-project "SELECT __id, __kind, __label FROM objects WHERE __kind = 'Service'"

# With JSON output
./target/release/qmdc query ./my-project "SELECT * FROM objects LIMIT 10" --format json

# Query via a Query object (reference to a [[id:Query]] object in the workspace)
./target/release/qmdc query ./my-project "#all_services"

# Count objects and edges
./target/release/qmdc query ./my-project "SELECT COUNT(*) as total FROM objects"
./target/release/qmdc query ./my-project "SELECT COUNT(*) as edges FROM edges"
```

The `query` command automatically:

- Recursively finds every QMDC workspace under the given folder
- Parses all files and loads objects into SQLite
- Extracts graph edges from references between objects
- Runs the SQL query against the database

Available tables:

- `objects` — all objects with `__id`, `__kind`, `__label`, `__file`, `__line`, `data` (JSON)
- `edges` — all graph edges with `source_id`, `target_id`, `field_name`

## Testing

```bash
# From the project root
make test

# Rust tests only
make rs-test

# cargo directly
cd qmdc-rs
cargo test

# With verbose output
cargo test -- --nocapture
```

## Distribution

`qmdc-rs` is published to **crates.io** only — install the native CLI with:

```bash
cargo install qmdc
```

### CLI usage

```bash
qmdc parse input.qmd.md
qmdc rebuild workspace/
qmdc lsp   # start the Language Server
```

The same `cargo build --release` binary is also bundled into the **`qmdc` PyPI package**
(`uvx qmdc`) and the **`@qmdc/qmdc` npm package** (`npx @qmdc/qmdc`). Those packages copy the native
binary in at their own publish time (see each package's `scripts/publish.sh`) — the binary
build lives here in the crate; the bundling lives in the consumer packages. The VS Code
extension bundles the same binary via its `package:<platform>` scripts.

### Version management

```bash
make bump          # 1.0.0 → 1.0.1 (Cargo.toml)
make bump-minor    # 1.0.0 → 1.1.0
make bump-major    # 1.0.0 → 2.0.0
```

---

## Programmatic API (Rust)

```rust
use qmdc::{parse, rebuild, ParseOptions};

// Parse QMD.md → JSON
let markdown = r#"
## User [[user]]

- name: Alice
- age: 30
"#;

let options = ParseOptions::default();
let result = parse(markdown, options);
// Vec<serde_json::Value> of objects

// Rebuild JSON → QMD.md
let qmdc = rebuild(&result);
// "## User [[user]]\n\n- name: Alice\n- age: 30\n"
```

## Status

✅ **Fully implemented:**

- Tokenizer (pulldown-cmark)
- Header parser (all variants: `[[id]]`, `[[id:Kind]]`, `[[:Kind]]`)
- Field parser (primitives: string, number, boolean, null)
- Nested objects (H1–H6+)
- Arrays (YAML notation, Markdown lists, object arrays)
- Tables
- Comments (`__comments`)
- Syntax metadata (`__syntax`)
- Types metadata (`__types`)
- YAML multiline syntax (`|`)
- Rebuild (canonical form)
- Lossless round-trip (`__level`, `__has_explicit_id`)
- CLI: `parse`, `rebuild`, `lsp`, `lsp-debug`, `query`
- **LSP server**: completion, hover, definition, references, diagnostics, rename
- **Workspace**: multi-file parsing, recursive workspace discovery
- **Query**: SQL queries against a workspace via SQLite, with Query-object support

## VS Code Integration

To use the LSP server in VS Code, add to your settings:

```json
{
  "qmdc.serverPath": "/path/to/qmdc-rs/target/release/qmdc",
  "qmdc.serverArgs": ["lsp"]
}
```

Or use the VS Code extension from `qmdc-vscode/`.

## License

[AGPL-3.0-or-later](LICENSE) © mikilabs
