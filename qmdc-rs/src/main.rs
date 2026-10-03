use clap::{Parser, Subcommand};
use qmdc::{
    execute_query, parse, parse_all_workspaces, rebuild, resolve_workspace_input, run_lsp,
    run_mcp_server, OutputFormat, ParseOptions,
};
use serde_json::{json, Map, Value};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

/// Build the `index` block of `workspace parse` output.
///
/// QMD-75: Python and TypeScript both emitted this and Rust did not, so the parse contract
/// depended on which parser a consumer happened to read. Three maps, values are plain `__id`s:
/// `by_global_id` keyed `namespace:Kind:id` (empty namespace leaves the first segment empty),
/// `by_kind`, and `by_file`. Objects whose kind is a system kind OTHER than the four
/// user-facing ones are left out, matching the other two implementations.
fn build_parse_index(objects: &[Value]) -> Value {
    const USER_FACING_SYSTEM_KINDS: [&str; 4] =
        ["__Workspace", "__Namespace", "__Document", "__Object"];

    let mut by_global_id = Map::new();
    let mut by_kind: Vec<(String, Vec<Value>)> = Vec::new();
    let mut by_file: Vec<(String, Vec<Value>)> = Vec::new();

    // Insertion order is preserved for the list maps, so a caller sees objects in document
    // order the way Python's dict and TypeScript's object literal present them.
    let push = |acc: &mut Vec<(String, Vec<Value>)>, key: &str, id: &Value| {
        if let Some(entry) = acc.iter_mut().find(|(k, _)| k == key) {
            entry.1.push(id.clone());
        } else {
            acc.push((key.to_string(), vec![id.clone()]));
        }
    };

    for obj in objects {
        let kind = obj.get("__kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind.starts_with("__") && !USER_FACING_SYSTEM_KINDS.contains(&kind) {
            continue;
        }
        let id = match obj.get("__id") {
            Some(v) if !v.is_null() => v.clone(),
            _ => continue,
        };
        let namespace = obj
            .get("__namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        by_global_id.insert(
            format!("{}:{}:{}", namespace, kind, id.as_str().unwrap_or("")),
            id.clone(),
        );
        if !kind.is_empty() {
            push(&mut by_kind, kind, &id);
        }
        if let Some(file) = obj.get("__file").and_then(|v| v.as_str()) {
            if !file.is_empty() {
                push(&mut by_file, file, &id);
            }
        }
    }

    let to_map = |pairs: Vec<(String, Vec<Value>)>| -> Map<String, Value> {
        pairs
            .into_iter()
            .map(|(k, v)| (k, Value::Array(v)))
            .collect()
    };

    json!({
        "by_global_id": by_global_id,
        "by_kind": to_map(by_kind),
        "by_file": to_map(by_file),
    })
}

#[derive(Parser)]
#[command(name = "qmdc")]
#[command(version)]
#[command(about = "QMDC Parser CLI (Rust)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse QMD.md to JSON
    Parse {
        /// Input file (reads from stdin if not provided)
        #[arg(short = 'i', long = "input")]
        input: Option<PathBuf>,

        /// Output file (writes to stdout if not provided)
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,

        /// Output format: minimal, standard, full
        #[arg(short = 'f', long = "format", default_value = "standard")]
        format: String,

        /// Remove __comments from output
        #[arg(long = "no-comments")]
        no_comments: bool,

        /// Compact JSON output (no pretty print)
        #[arg(long = "no-pretty")]
        no_pretty: bool,
    },

    /// Rebuild QMD.md from JSON
    Rebuild {
        /// Input JSON file (reads from stdin if not provided)
        #[arg(short = 'i', long = "input")]
        input: Option<PathBuf>,

        /// Output QMD.md file (writes to stdout if not provided)
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,
    },

    /// Workspace operations
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },

    /// Execute SQL query against workspace
    Query {
        /// Workspace directory path, or the QUERY itself when --with is used
        workspace: Option<String>,

        /// Query: SQL or "#query_id" for Query object reference
        query: Option<String>,

        /// Compose this workspace explicitly; repeatable. Mutually exclusive with WORKSPACE.
        #[arg(short = 'w', long = "with")]
        with: Vec<PathBuf>,

        /// Query language (default: sql)
        #[arg(short = 'l', long = "lang", default_value = "sql")]
        lang: String,

        /// Output format: table or json
        #[arg(short = 'f', long = "format", default_value = "table")]
        format: String,
    },

    /// Start LSP server
    Lsp {
        /// Use stdio transport (default)
        #[arg(long, default_value = "true")]
        stdio: bool,
    },

    /// Start MCP server (Model Context Protocol over stdio)
    Mcp {
        /// Restrict every operation to paths within this root directory (fail-closed
        /// INV-1 boundary). When omitted, the server trusts each caller-supplied path
        /// (local single-user model).
        #[arg(long = "force-root")]
        force_root: Option<PathBuf>,

        /// Serve the composition of these workspaces; repeatable (GitHub #10). Every tool
        /// then answers over the same graph `qmdc query -w …` sees, and its `path` argument
        /// must lie inside one of them.
        #[arg(short = 'w', long = "with")]
        with: Vec<PathBuf>,
    },

    /// Debug LSP commands (stateless, no server)
    LspDebug {
        /// Workspace path
        workspace: PathBuf,

        /// Command JSON (from stdin if not provided)
        #[arg(short = 'c', long = "command")]
        command: Option<String>,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    /// Parse workspace directory
    Parse {
        /// Workspace directory path (omit when composing with --with)
        path: Option<PathBuf>,

        /// Compose this workspace explicitly; repeatable. Mutually exclusive with PATH.
        #[arg(short = 'w', long = "with")]
        with: Vec<PathBuf>,

        /// Output format: minimal, standard, full
        #[arg(short = 'f', long = "format", default_value = "standard")]
        format: String,
    },

    /// Validate workspace and return errors as JSON array
    Validate {
        /// Workspace directory path (omit when composing with --with)
        path: Option<PathBuf>,

        /// Compose this workspace explicitly; repeatable. Mutually exclusive with PATH.
        #[arg(short = 'w', long = "with")]
        with: Vec<PathBuf>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // No subcommand - show version and hint
            println!("qmdc {}", env!("CARGO_PKG_VERSION"));
            eprintln!("Use --help for usage");
            return;
        }
    };

    match command {
        Commands::Parse {
            input,
            output,
            format,
            no_comments,
            no_pretty,
        } => {
            // Read input
            let content = match input {
                Some(path) => fs::read_to_string(&path).expect("Failed to read input file"),
                None => {
                    let mut buffer = String::new();
                    io::stdin()
                        .read_to_string(&mut buffer)
                        .expect("Failed to read stdin");
                    buffer
                }
            };

            let fmt = match format.as_str() {
                "minimal" => OutputFormat::Minimal,
                "full" => OutputFormat::Full,
                _ => OutputFormat::Standard,
            };

            let options = ParseOptions {
                random_seed: Some(666),
                format: fmt,
            };

            let mut objects = parse(&content, options);

            // Remove __comments if requested
            if no_comments {
                for obj in &mut objects {
                    if let Value::Object(map) = obj {
                        map.remove("__comments");
                    }
                }
            }

            // Format output
            let json_output = if no_pretty {
                serde_json::to_string(&objects).unwrap()
            } else {
                serde_json::to_string_pretty(&objects).unwrap()
            };

            // Write output
            match output {
                Some(path) => {
                    fs::write(&path, &json_output).expect("Failed to write output file");
                }
                None => {
                    println!("{}", json_output);
                }
            }

            // #11: a document that produced __ParsingError objects is not a clean parse, so
            // the exit code says so (the output above is still complete). Rust keeps
            // `__kind` on errors in every format, so the output itself can be counted.
            let errors = objects
                .iter()
                .filter(|obj| obj.get("__kind").and_then(|v| v.as_str()) == Some("__ParsingError"))
                .count();
            if errors > 0 {
                eprintln!(
                    "error: document has {} parsing error(s); see the __ParsingError objects in the output",
                    errors
                );
                std::process::exit(1);
            }
        }

        Commands::Rebuild { input, output } => {
            // Read input JSON
            let content = match input {
                Some(path) => fs::read_to_string(&path).expect("Failed to read input file"),
                None => {
                    let mut buffer = String::new();
                    io::stdin()
                        .read_to_string(&mut buffer)
                        .expect("Failed to read stdin");
                    buffer
                }
            };

            let objects: Vec<Value> = serde_json::from_str(&content).expect("Failed to parse JSON");

            let qmdc_output = rebuild(&objects);

            // Write output
            match output {
                Some(path) => {
                    fs::write(&path, &qmdc_output).expect("Failed to write output file");
                }
                None => {
                    print!("{}", qmdc_output);
                }
            }
        }

        Commands::Workspace { action } => match action {
            WorkspaceAction::Parse { path, with, format } => {
                let fmt = match format.as_str() {
                    "minimal" => OutputFormat::Minimal,
                    "full" => OutputFormat::Full,
                    _ => OutputFormat::Standard,
                };
                // QMD-59: unified resolver — walk-up to an ancestor workspace,
                // else walk-down into contained sub-workspaces.
                // QMD-72: or compose the explicitly-supplied `--with` workspaces.
                let result = match resolve_workspace_input(path.as_deref(), &with, fmt) {
                    Ok(r) => r,
                    Err(msg) => {
                        eprintln!("error: {}", msg);
                        std::process::exit(2);
                    }
                };

                // Output shape (QMD-72): one shape for every invocation. `workspaces` is always
                // present — zero, one or many entries, each `{id, root, path}` — replacing the
                // QMD-59 `workspace: id` / `workspaces: [ids]` / `workspace: null` trio a
                // consumer had to tell apart. `root` is the base every `__file` is relative
                // to, or null when that base is virtual (`-w`).
                //
                // QMD-75: `index` is part of that shape too. Python and TypeScript both emitted
                // it and Rust did not, so a consumer written against either of them read a
                // missing key here.
                let payload = json!({
                    "root": result.root,
                    "workspaces": result.workspaces,
                    "files": result.files,
                    "objects": result.objects,
                    "index": build_parse_index(&result.objects),
                    "errors": result.errors,
                });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            }
            WorkspaceAction::Validate { path, with } => {
                // QMD-59: unified resolver — walk-up then walk-down.
                // QMD-72: or compose the explicitly-supplied `--with` workspaces.
                let result =
                    match resolve_workspace_input(path.as_deref(), &with, OutputFormat::Standard) {
                        Ok(r) => r,
                        Err(msg) => {
                            eprintln!("error: {}", msg);
                            std::process::exit(2);
                        }
                    };
                // Convert workspace errors to unified format matching Python/TypeScript
                let errors_array: Vec<Value> = result
                    .errors
                    .iter()
                    .map(|e| {
                        json!({
                            "type": e.error_type,
                            "message": e.message,
                            "file": e.file,
                            "line": e.line,
                            "objectId": e.object,
                            "fieldName": e.field_name,
                            "reference": e.reference,
                            "candidates": e.candidates,
                            "severity": e.severity,
                        })
                    })
                    .collect();

                println!("{}", serde_json::to_string_pretty(&errors_array).unwrap());
                std::process::exit(if errors_array.is_empty() { 0 } else { 1 });
            }
        },

        Commands::Query {
            workspace,
            query,
            with,
            lang: _,
            format,
        } => {
            // QMD-72: with `--with`, the single remaining positional IS the query — a
            // composed set has no one path, so there is nothing for a path positional to
            // name. Both positionals alongside `--with` is the mutually-exclusive usage
            // error, not a path to silently ignore.
            //
            // WITHOUT `--with` both positionals stay required. A lone positional must NOT
            // be read as the query against an implied `.`: py and ts refuse it with exit 2,
            // and so did this CLI before QMD-72, so accepting it here would be a Rust-only
            // convenience that breaks three-way conformance on the `query` surface.
            let (ws_path, query) = if with.is_empty() {
                match (workspace, query) {
                    (Some(w), Some(q)) => (Some(PathBuf::from(w)), q),
                    _ => {
                        eprintln!("error: a QUERY is required");
                        std::process::exit(2);
                    }
                }
            } else {
                match (workspace, query) {
                    (Some(q), None) => (None, q),
                    (Some(_), Some(_)) => {
                        eprintln!(
                            "error: a positional WORKSPACE and --with are mutually exclusive; pass every workspace as --with"
                        );
                        std::process::exit(2);
                    }
                    _ => {
                        eprintln!("error: a QUERY is required");
                        std::process::exit(2);
                    }
                }
            };

            // QMD-59: unified resolver — walk-up to an ancestor workspace, else
            // walk-down into contained sub-workspaces (so query works from any dir).
            let ws_result =
                match resolve_workspace_input(ws_path.as_deref(), &with, OutputFormat::Standard) {
                    Ok(r) => r,
                    Err(msg) => {
                        eprintln!("error: {}", msg);
                        std::process::exit(2);
                    }
                };

            // Execute query
            match execute_query(&ws_result, &query) {
                Ok(result) => {
                    match format.as_str() {
                        "json" => {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "columns": result.columns,
                                    "rows": result.rows,
                                }))
                                .unwrap()
                            );
                        }
                        _ => {
                            // table format (default)
                            print!("{}", result.to_table_string());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Lsp { stdio: _ } => {
            run_lsp().await;
        }

        Commands::Mcp { force_root, with } => {
            if let Err(msg) = qmdc::mcp::server::configure_compose_with(force_root.clone(), with) {
                eprintln!("error: {}", msg);
                std::process::exit(2);
            }
            run_mcp_server(force_root).await;
        }

        Commands::LspDebug { workspace, command } => {
            use qmdc::core::tree::{get_tree_by_file, get_tree_by_namespace, get_tree_by_smart};
            use qmdc::db::QmdcDatabase;

            // Read command JSON
            let cmd_json = match command {
                Some(c) => c,
                None => {
                    let mut buffer = String::new();
                    io::stdin()
                        .read_to_string(&mut buffer)
                        .expect("Failed to read stdin");
                    buffer
                }
            };

            // Parse command
            let cmd: Value = serde_json::from_str(&cmd_json).expect("Invalid JSON");

            let command_name = cmd
                .get("command")
                .and_then(|c| c.as_str())
                .expect("Missing 'command' field");
            let args = cmd.get("arguments").and_then(|a| a.as_array());

            // Parse workspace
            let ws_result = parse_all_workspaces(&workspace, OutputFormat::Full);

            // Create DB
            let db = QmdcDatabase::new().expect("Failed to create DB");
            db.sync_objects_from_vec(&ws_result.objects)
                .expect("Failed to sync objects");

            // Execute command
            let result = match command_name {
                "qmdc.getWorkspaceTree" => {
                    let mode = args
                        .and_then(|a| a.get(1))
                        .and_then(|m| m.as_str())
                        .unwrap_or("namespace");

                    match mode {
                        "file" => get_tree_by_file(&db),
                        "smart" => get_tree_by_smart(&db),
                        _ => get_tree_by_namespace(&db),
                    }
                }
                _ => {
                    eprintln!("Unknown command: {}", command_name);
                    std::process::exit(1);
                }
            };

            match result {
                Ok(Some(data)) => {
                    println!("{}", serde_json::to_string_pretty(&data).unwrap());
                }
                Ok(None) => {
                    println!("null");
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
