use clap::{Parser, Subcommand};
use qmdc::{
    execute_query, parse, parse_all_workspaces, rebuild, resolve_workspace, run_lsp,
    run_mcp_server, OutputFormat, ParseOptions,
};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

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
        /// Workspace directory path
        workspace: PathBuf,

        /// Query: SQL or "#query_id" for Query object reference
        query: String,

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
        /// Workspace directory path
        path: PathBuf,

        /// Output format: minimal, standard, full
        #[arg(short = 'f', long = "format", default_value = "standard")]
        format: String,
    },

    /// Validate workspace and return errors as JSON array
    Validate {
        /// Workspace directory path
        path: PathBuf,
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
            WorkspaceAction::Parse { path, format } => {
                let fmt = match format.as_str() {
                    "minimal" => OutputFormat::Minimal,
                    "full" => OutputFormat::Full,
                    _ => OutputFormat::Standard,
                };
                // QMD-59: unified resolver — walk-up to an ancestor workspace,
                // else walk-down into contained sub-workspaces.
                let result = resolve_workspace(&path, fmt);

                // Output-shape (QMD-59): never emit a bare `workspace: null` when
                // workspaces were actually resolved. Derive id(s) from objects:
                //   - workspace_id set (walk-up/self)      -> "workspace": id
                //   - exactly one resolved sub-workspace   -> "workspace": that id
                //   - multiple resolved sub-workspaces     -> "workspaces": [ids]
                let mut payload = json!({
                    "root": result.root,
                    "files": result.files,
                    "objects": result.objects,
                    "errors": result.errors,
                });
                let payload_map = payload.as_object_mut().unwrap();
                if let Some(ws_id) = &result.workspace_id {
                    payload_map.insert("workspace".to_string(), json!(ws_id));
                } else {
                    let mut ws_ids: Vec<String> = result
                        .objects
                        .iter()
                        .filter(|o| o.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace"))
                        .filter_map(|o| o.get("__id").and_then(|v| v.as_str()).map(String::from))
                        .collect();
                    ws_ids.sort();
                    ws_ids.dedup();
                    match ws_ids.len() {
                        1 => {
                            payload_map.insert("workspace".to_string(), json!(ws_ids[0]));
                        }
                        n if n > 1 => {
                            payload_map.insert("workspaces".to_string(), json!(ws_ids));
                        }
                        _ => {
                            payload_map.insert("workspace".to_string(), Value::Null);
                        }
                    }
                }
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            }
            WorkspaceAction::Validate { path } => {
                // QMD-59: unified resolver — walk-up then walk-down.
                let result = resolve_workspace(&path, OutputFormat::Standard);
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
            lang: _,
            format,
        } => {
            // QMD-59: unified resolver — walk-up to an ancestor workspace, else
            // walk-down into contained sub-workspaces (so query works from any dir).
            let ws_result = resolve_workspace(&workspace, OutputFormat::Standard);

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

        Commands::Mcp { force_root } => {
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
