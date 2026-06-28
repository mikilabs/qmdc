import * as path from 'path';
import * as fs from 'fs';
import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
  TransportKind,
  State,
} from 'vscode-languageclient/node';
import { QmdcExplorerProvider } from './qmdcTreeProvider';
import {
  generatePreviewHtml,
  parseBlockContent,
  QueryExecutor,
} from './preview-renderer';

let client: LanguageClient | undefined;
let extensionVersion = 'unknown';
let previewPanel: vscode.WebviewPanel | undefined;
let outputChannel: vscode.OutputChannel;
let workspaceDiagnostics: vscode.DiagnosticCollection | undefined;

// Minimum required qmdc version (must have __level in SQLite schema)
const MIN_QMDC_VERSION = '0.3.0';

function log(msg: string) {
  outputChannel?.appendLine(`[${new Date().toISOString()}] ${msg}`);
  console.log('[QMDC]', msg);
}

function parseVersion(ver: string): number[] {
  return ver.split('.').map(n => parseInt(n, 10) || 0);
}

function isVersionOk(actual: string, minimum: string): boolean {
  const a = parseVersion(actual);
  const m = parseVersion(minimum);
  for (let i = 0; i < Math.max(a.length, m.length); i++) {
    const av = a[i] || 0;
    const mv = m[i] || 0;
    if (av > mv) return true;
    if (av < mv) return false;
  }
  return true; // equal
}

async function getQmdcVersion(serverPath: string): Promise<string | null> {
  return new Promise((resolve) => {
    const { execFile } = require('child_process');
    // Use --version flag (clap built-in)
    execFile(serverPath, ['--version'], { timeout: 5000 }, (error: any, stdout: string, stderr: string) => {
      if (error) {
        resolve(null);
        return;
      }
      // Output: "qmdc 0.3.0" or similar
      const match = (stdout + stderr).match(/(\d+\.\d+\.\d+)/);
      resolve(match ? match[1] : null);
    });
  });
}

// Dynamic block types that support "▶ Run Query" CodeLens
const DYNAMIC_BLOCK_TYPES = ['table'];

/**
 * CodeLens provider for QMDC dynamic blocks.
 * Shows "▶ Run Query" above ```table blocks.
 */
class QmdcCodeLensProvider implements vscode.CodeLensProvider {
  private _onDidChangeCodeLenses = new vscode.EventEmitter<void>();
  readonly onDidChangeCodeLenses = this._onDidChangeCodeLenses.event;

  provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
    const codeLenses: vscode.CodeLens[] = [];
    const text = document.getText();
    
    // Find all ```{lang} blocks where lang is a dynamic block type
    const codeFenceRegex = /^```(\w+)\s*$/gm;
    let match;
    
    while ((match = codeFenceRegex.exec(text)) !== null) {
      const lang = match[1];
      if (!DYNAMIC_BLOCK_TYPES.includes(lang)) {
        continue;
      }
      
      const startPos = document.positionAt(match.index);
      const line = startPos.line;
      
      // Find the closing ```
      const afterOpen = match.index + match[0].length;
      const closingIndex = text.indexOf('\n```', afterOpen);
      if (closingIndex === -1) {
        continue; // Unclosed block
      }
      
      // Extract block content (between opening and closing ```)
      const blockContent = text.substring(afterOpen, closingIndex).trim();
      
      const range = new vscode.Range(line, 0, line, 0);
      const codeLens = new vscode.CodeLens(range, {
        title: '▶ Run Query',
        command: 'qmdc.runQueryFromBlock',
        arguments: [document.uri, line, blockContent],
      });
      
      codeLenses.push(codeLens);
    }
    
    return codeLenses;
  }

  refresh(): void {
    this._onDidChangeCodeLenses.fire();
  }
}

function getBinaryName(): string {
  return process.platform === 'win32' ? 'qmdc.exe' : 'qmdc';
}

function findQmdcPath(context: vscode.ExtensionContext): string | undefined {
  const binaryName = getBinaryName();
  
  // 1. Check settings (explicit override)
  const config = vscode.workspace.getConfiguration('qmdc');
  const configPath = config.get<string>('server.path');
  if (configPath && fs.existsSync(configPath)) {
    log(`Using qmdc from settings: ${configPath}`);
    return configPath;
  }

  // 2. Use bundled binary (extension/bin/qmdc)
  const bundledPath = path.join(context.extensionPath, 'bin', binaryName);
  if (fs.existsSync(bundledPath)) {
    log(`Using bundled binary: ${bundledPath}`);
    return bundledPath;
  }

  // 3. Fallback to PATH
  log(`Falling back to PATH: qmdc`);
  return 'qmdc';
}

export async function activate(context: vscode.ExtensionContext) {
  // Create output channel for logging
  outputChannel = vscode.window.createOutputChannel('QMDC Explorer');
  outputChannel.show(true); // Show immediately for debugging
  
  // Get extension version from package.json
  extensionVersion = context.extension.packageJSON.version || 'unknown';
  log(`QMDC extension v${extensionVersion} is activating...`);
  
  // Log workspace info for debugging
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (workspaceFolders) {
    log(`Workspace folders: ${workspaceFolders.map(f => f.uri.fsPath).join(', ')}`);
  } else {
    log(`No workspace folders found`);
  }
  log(`Extension path: ${context.extensionPath}`);

  const serverPath = findQmdcPath(context);
  log(`QMDC server path: ${serverPath}`);

  if (!serverPath || (serverPath !== 'qmdc' && !fs.existsSync(serverPath))) {
    vscode.window.showErrorMessage(
      'QMDC Language Server (qmdc) not found. Please install it or set qmdc.server.path in settings.'
    );
    return;
  }

  // Check qmdc version
  const qmdcVersion = await getQmdcVersion(serverPath);
  log(`qmdc version: ${qmdcVersion || 'unknown'}, required: >= ${MIN_QMDC_VERSION}`);
  
  if (!qmdcVersion) {
    vscode.window.showWarningMessage(
      `Could not determine qmdc version. Required: >= ${MIN_QMDC_VERSION}`
    );
  } else if (!isVersionOk(qmdcVersion, MIN_QMDC_VERSION)) {
    vscode.window.showErrorMessage(
      `qmdc version ${qmdcVersion} is too old! Required: >= ${MIN_QMDC_VERSION}. ` +
      `Please rebuild: cd qmdc-rs && cargo build && cp target/debug/qmdc ../qmdc-vscode/bin/`
    );
    return;
  } else {
    log(`✓ qmdc version OK: ${qmdcVersion}`);
  }

  // Server executable
  const run: Executable = {
    command: serverPath,
    args: ['lsp'],
    transport: TransportKind.stdio,
  };

  const debug: Executable = {
    command: serverPath,
    args: ['lsp'],
    transport: TransportKind.stdio,
    options: {
      env: {
        ...process.env,
        RUST_LOG: 'debug',
        RUST_BACKTRACE: '1',
      },
    },
  };

  // Server options
  const serverOptions: ServerOptions = {
    run,
    debug,
  };

  // Client options
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'qmdmd' },
      { scheme: 'file', pattern: '**/*.qmd.md' },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.qmd.md'),
    },
    outputChannelName: 'QMDC Language Server',
  };

  // Create the language client
  client = new LanguageClient(
    'qmdc',
    'QMDC Language Server',
    serverOptions,
    clientOptions
  );

  // Register restart command
  const restartCommand = vscode.commands.registerCommand('qmdc.restartServer', async () => {
    if (client) {
      await client.stop();
      await client.start();
      explorerProvider.setClient(client);
      vscode.window.showInformationMessage('QMDC Language Server restarted');
    }
  });
  context.subscriptions.push(restartCommand);

  // Register Go to Object command (Ctrl+Shift+O)
  const goToObjectCommand = vscode.commands.registerCommand('qmdc.goToObject', async () => {
    if (!client) {
      vscode.window.showErrorMessage('QMDC Language Server not running');
      return;
    }

    // Request workspace symbols from LSP
    const symbols = await client.sendRequest('workspace/symbol', { query: '' });
    
    if (!symbols || !Array.isArray(symbols) || symbols.length === 0) {
      vscode.window.showInformationMessage('No QMD.md objects found in workspace');
      return;
    }

    // Create QuickPick items
    const items = symbols.map((sym: any) => ({
      label: sym.name,
      description: sym.containerName || '',
      detail: sym.location?.uri ? path.basename(sym.location.uri) : '',
      symbol: sym,
    }));

    // Show QuickPick
    const selected = await vscode.window.showQuickPick(items, {
      placeHolder: 'Select an object to go to...',
      matchOnDescription: true,
      matchOnDetail: true,
    });

    if (selected && selected.symbol.location) {
      const uri = vscode.Uri.parse(selected.symbol.location.uri);
      const range = new vscode.Range(
        selected.symbol.location.range.start.line,
        selected.symbol.location.range.start.character,
        selected.symbol.location.range.end.line,
        selected.symbol.location.range.end.character
      );
      
      const doc = await vscode.workspace.openTextDocument(uri);
      const editor = await vscode.window.showTextDocument(doc);
      editor.selection = new vscode.Selection(range.start, range.start);
      editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
    }
  });
  context.subscriptions.push(goToObjectCommand);

  // Register Show References command (Shift+F12)
  const showReferencesCommand = vscode.commands.registerCommand('qmdc.showReferences', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'qmdmd') {
      vscode.window.showErrorMessage('No QMD.md file open');
      return;
    }
    
    // Use built-in references command
    await vscode.commands.executeCommand('editor.action.goToReferences');
  });
  context.subscriptions.push(showReferencesCommand);

  // Register Parse Workspace command
  const parseWorkspaceCommand = vscode.commands.registerCommand('qmdc.parseWorkspace', async () => {
    if (!client) {
      vscode.window.showErrorMessage('QMDC Language Server not running');
      return;
    }
    
    // Restart server to re-parse workspace
    await client.stop();
    await client.start();
    vscode.window.showInformationMessage('QMDC workspace re-parsed');
  });
  context.subscriptions.push(parseWorkspaceCommand);

  // Register Validate Workspace command
  const validateWorkspaceCommand = vscode.commands.registerCommand('qmdc.validateWorkspace', async () => {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders || workspaceFolders.length === 0) {
      vscode.window.showErrorMessage('No workspace folder open');
      return;
    }

    const workspacePath = workspaceFolders[0].uri.fsPath;
    const qmdcPath = findQmdcPath(context);
    
    if (!qmdcPath || (qmdcPath !== 'qmdc' && !fs.existsSync(qmdcPath))) {
      vscode.window.showErrorMessage('QMDC Language Server (qmdc) not found');
      return;
    }

    log(`Validating workspace: ${workspacePath}`);
    vscode.window.setStatusBarMessage('Validating QMDC workspace...', 1000);

    try {
      const { execFile } = require('child_process');
      const result = await new Promise<{ stdout: string; stderr: string }>((resolve, reject) => {
        execFile(
          qmdcPath,
          ['workspace', 'parse', workspacePath, '--format', 'standard'],
          { timeout: 30000, maxBuffer: 50 * 1024 * 1024, cwd: workspacePath },
          (error: any, stdout: string, stderr: string) => {
            if (error && error.code !== 0) {
              // qmdc may return non-zero exit code if there are errors, but still output JSON
              resolve({ stdout, stderr });
            } else if (error) {
              reject(error);
            } else {
              resolve({ stdout, stderr });
            }
          }
        );
      });

      // Parse JSON output
      let workspaceResult: any;
      try {
        workspaceResult = JSON.parse(result.stdout);
      } catch (parseError) {
        log(`Failed to parse qmdc output: ${parseError}`);
        vscode.window.showErrorMessage(`Failed to parse workspace validation result: ${result.stderr || 'Invalid JSON'}`);
        return;
      }

      const errors = workspaceResult.errors || [];
      const files = workspaceResult.files || [];
      const errorCount = errors.length;

      log(`Workspace validation: ${files.length} files, ${errorCount} errors`);

      // Clear previous diagnostics if no errors
      if (workspaceDiagnostics) {
        workspaceDiagnostics.clear();
      }

      if (errorCount === 0) {
        vscode.window.showInformationMessage(`✅ Workspace validated: ${files.length} files, no errors`);
      } else {
        // Show errors in Problems panel by publishing diagnostics
        const diagnosticsMap = new Map<string, vscode.Diagnostic[]>();
        
        for (const error of errors) {
          const file = error.file || 'unknown';
          const filePath = path.isAbsolute(file) ? file : path.join(workspacePath, file);
          const uri = vscode.Uri.file(filePath);
          
          const line = (error.line || 1) - 1; // Convert to 0-based
          const range = new vscode.Range(line, 0, line, 1000);
          
          const diagnostic = new vscode.Diagnostic(
            range,
            error.message || 'Workspace validation error',
            vscode.DiagnosticSeverity.Error
          );
          diagnostic.source = 'QMDC';
          diagnostic.code = error.error_type || 'validation_error';
          
          if (!diagnosticsMap.has(uri.toString())) {
            diagnosticsMap.set(uri.toString(), []);
          }
          diagnosticsMap.get(uri.toString())!.push(diagnostic);
        }

        // Create or reuse diagnostic collection
        if (!workspaceDiagnostics) {
          workspaceDiagnostics = vscode.languages.createDiagnosticCollection('qmdc-workspace');
          context.subscriptions.push(workspaceDiagnostics);
        }
        
        // Clear previous diagnostics
        workspaceDiagnostics.clear();
        
        // Publish diagnostics for each file
        for (const [uriString, diagnostics] of diagnosticsMap) {
          const uri = vscode.Uri.parse(uriString);
          workspaceDiagnostics.set(uri, diagnostics);
        }

        vscode.window.showWarningMessage(
          `⚠️ Workspace has ${errorCount} error(s) in ${files.length} files`,
          'Show Problems'
        ).then(selection => {
          if (selection === 'Show Problems') {
            vscode.commands.executeCommand('workbench.actions.view.problems');
          }
        });
      }
    } catch (error: any) {
      const errorMsg = error?.message || String(error);
      log(`Workspace validation failed: ${errorMsg}`);
      vscode.window.showErrorMessage(`Failed to validate workspace: ${errorMsg}`);
    }
  });
  context.subscriptions.push(validateWorkspaceCommand);

  // Create output channel for SQL queries
  const sqlOutputChannel = vscode.window.createOutputChannel('QMDC SQL');
  context.subscriptions.push(sqlOutputChannel);

  // Register CodeLens provider for dynamic blocks
  const codeLensProvider = new QmdcCodeLensProvider();
  const codeLensDisposable = vscode.languages.registerCodeLensProvider(
    { language: 'qmdmd', scheme: 'file' },
    codeLensProvider
  );
  context.subscriptions.push(codeLensDisposable);

  // Register QMDC Explorer (TreeDataProvider)
  const explorerProvider = new QmdcExplorerProvider(outputChannel);
  const treeView = vscode.window.createTreeView('qmdcObjects', {
    treeDataProvider: explorerProvider,
    showCollapseAll: true,
  });
  context.subscriptions.push(treeView);

  // Set client when ready (deferred until client starts)
  // Will be called after client.start() below

  // Refresh explorer command
  const refreshExplorerCommand = vscode.commands.registerCommand('qmdc.refreshExplorer', () => {
    explorerProvider.refresh();
  });
  context.subscriptions.push(refreshExplorerCommand);

  // Grouping mode commands
  const groupByNamespaceCommand = vscode.commands.registerCommand('qmdc.groupByNamespace', () => {
    explorerProvider.setGroupingMode('namespace');
  });
  context.subscriptions.push(groupByNamespaceCommand);


  const groupByFileCommand = vscode.commands.registerCommand('qmdc.groupByFile', () => {
    explorerProvider.setGroupingMode('file');
  });
  context.subscriptions.push(groupByFileCommand);

  const groupBySmartCommand = vscode.commands.registerCommand('qmdc.groupBySmart', () => {
    explorerProvider.setGroupingMode('smart');
  });
  context.subscriptions.push(groupBySmartCommand);

  // Go to object from explorer click
  const goToObjectFromExplorerCommand = vscode.commands.registerCommand(
    'qmdc.goToObjectFromExplorer',
    async (uri: string, position?: { line: number; character: number }, workspacePath?: string) => {
      log(`goToObjectFromExplorer: uri=${uri}, pos=${JSON.stringify(position)}, workspacePath=${workspacePath}`);
      if (!uri) {
        log('goToObjectFromExplorer: no uri!');
        vscode.window.showErrorMessage('Failed to open: no URI provided');
        return;
      }
      
      try {
        const parsedUri = vscode.Uri.parse(uri);
        log(`goToObjectFromExplorer: parsed URI scheme=${parsedUri.scheme}, path=${parsedUri.fsPath}`);
        
        // Check if file exists (for file:// URIs)
        if (parsedUri.scheme === 'file') {
          const filePath = parsedUri.fsPath;
          if (!fs.existsSync(filePath)) {
            log(`goToObjectFromExplorer: file does not exist: ${filePath}`);
            // Try to find the file in workspace folders, prefer the provided workspacePath
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (workspaceFolders) {
              let found = false;
              
              // First, try to find in the provided workspacePath if available
              if (workspacePath) {
                const relativePath = path.relative(workspacePath, filePath);
                // If filePath is relative to workspacePath, try to resolve it
                if (!path.isAbsolute(relativePath) || relativePath.startsWith('..')) {
                  // Extract relative path from filePath if it contains workspacePath
                  const fileRelativePath = filePath.includes(workspacePath) 
                    ? filePath.substring(workspacePath.length + 1)
                    : path.basename(filePath);
                  const candidatePath = path.join(workspacePath, fileRelativePath);
                  if (fs.existsSync(candidatePath)) {
                    log(`goToObjectFromExplorer: found file in provided workspace: ${candidatePath}`);
                    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(candidatePath));
                    const editor = await vscode.window.showTextDocument(doc);
                    if (position) {
                      const pos = new vscode.Position(position.line, position.character);
                      editor.selection = new vscode.Selection(pos, pos);
                      editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
                    }
                    found = true;
                  }
                } else if (fs.existsSync(filePath)) {
                  // File path is already correct
                  const doc = await vscode.workspace.openTextDocument(parsedUri);
                  const editor = await vscode.window.showTextDocument(doc);
                  if (position) {
                    const pos = new vscode.Position(position.line, position.character);
                    editor.selection = new vscode.Selection(pos, pos);
                    editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
                  }
                  found = true;
                }
              }
              
              // Fallback: try to find file by name in workspace folders (original logic)
              if (!found) {
                for (const folder of workspaceFolders) {
                  // Skip if we already tried this workspacePath
                  if (workspacePath && folder.uri.fsPath === workspacePath) {
                    continue;
                  }
                  // Try to find file by name in workspace
                  const fileName = path.basename(filePath);
                  const candidatePath = path.join(folder.uri.fsPath, fileName);
                  if (fs.existsSync(candidatePath)) {
                    log(`goToObjectFromExplorer: found file in workspace: ${candidatePath}`);
                    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(candidatePath));
                    const editor = await vscode.window.showTextDocument(doc);
                    if (position) {
                      const pos = new vscode.Position(position.line, position.character);
                      editor.selection = new vscode.Selection(pos, pos);
                      editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
                    }
                    found = true;
                    break;
                  }
                }
              }
              
              if (!found) {
                vscode.window.showErrorMessage(`Failed to open: File not found: ${filePath}`);
                return;
              }
            } else {
              vscode.window.showErrorMessage(`Failed to open: File not found: ${filePath}`);
              return;
            }
          } else {
            // DEBUG: Check file size before opening
            const filePath = parsedUri.fsPath;
            try {
              const stats = fs.statSync(filePath);
              log(`goToObjectFromExplorer: file size check: path=${filePath}, size=${stats.size} bytes (${(stats.size / 1024).toFixed(2)} KB)`);
              if (stats.size > 50 * 1024 * 1024) {
                log(`goToObjectFromExplorer: WARNING - file is larger than 50MB!`);
              }
            } catch (e) {
              log(`goToObjectFromExplorer: failed to stat file: ${e}`);
            }
            
            // Try to open file - with fallback for VS Code's "50MB sync" bug
            let doc: vscode.TextDocument;
            let editor: vscode.TextEditor;
            try {
              doc = await vscode.workspace.openTextDocument(parsedUri);
              editor = await vscode.window.showTextDocument(doc);
            } catch (openError: any) {
              const errorMsg = openError?.message || String(openError);
              log(`goToObjectFromExplorer: openTextDocument failed: ${errorMsg}`);
              
              // Check if this is the "50MB sync" bug (file is actually small)
              if (errorMsg.includes('50MB') || errorMsg.includes('cannot be synchronized')) {
                log(`goToObjectFromExplorer: detected VS Code 50MB sync bug, trying vscode.open command`);
                
                // Fallback: use vscode.open command which bypasses extension host sync
                await vscode.commands.executeCommand('vscode.open', parsedUri);
                
                // Get the editor after opening
                await new Promise(resolve => setTimeout(resolve, 100)); // Small delay for editor to open
                const activeEditor = vscode.window.activeTextEditor;
                if (activeEditor && activeEditor.document.uri.fsPath === filePath) {
                  editor = activeEditor;
                  if (position) {
                    const pos = new vscode.Position(position.line, position.character);
                    editor.selection = new vscode.Selection(pos, pos);
                    editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
                  }
                  log(`goToObjectFromExplorer: fallback vscode.open succeeded`);
                  return;
                } else {
                  log(`goToObjectFromExplorer: fallback vscode.open opened, but can't navigate to position`);
                  return;
                }
              }
              throw openError; // Re-throw if not the 50MB bug
            }
            
            if (position) {
              const pos = new vscode.Position(position.line, position.character);
              editor.selection = new vscode.Selection(pos, pos);
              editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
            }
          }
        } else {
          // For non-file URIs, try to open directly
          // DEBUG: Log URI info
          log(`goToObjectFromExplorer: opening non-file URI: ${parsedUri.toString()}, scheme=${parsedUri.scheme}`);
          const doc = await vscode.workspace.openTextDocument(parsedUri);
          const editor = await vscode.window.showTextDocument(doc);
          
          if (position) {
            const pos = new vscode.Position(position.line, position.character);
            editor.selection = new vscode.Selection(pos, pos);
            editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
          }
        }
      } catch (error: any) {
        const errorMsg = error?.message || String(error);
        log(`goToObjectFromExplorer: error=${errorMsg}`);
        vscode.window.showErrorMessage(`Failed to open: ${errorMsg}`);
      }
    }
  );
  context.subscriptions.push(goToObjectFromExplorerCommand);

  // Register Run Query from Block command (triggered by CodeLens)
  const runQueryFromBlockCommand = vscode.commands.registerCommand(
    'qmdc.runQueryFromBlock',
    async (uri: vscode.Uri, line: number, blockContent: string) => {
      if (!client) {
        vscode.window.showErrorMessage('QMDC Language Server not running');
        return;
      }

      // Wait for client to be ready
      if (client.state !== State.Running) {
        vscode.window.showInformationMessage('Waiting for QMDC Language Server to start...');
        await new Promise<void>((resolve) => {
          const checkState = () => {
            if (client && client.state === State.Running) {
              resolve();
            } else {
              setTimeout(checkState, 100);
            }
          };
          checkState();
        });
      }

      // Parse block content to extract SQL and scope
      const { sql, scope } = parseBlockContent(blockContent);
      if (!sql) {
        vscode.window.showErrorMessage('No query found in block. Use "query: [[#id]]" or "sql: SELECT ..."');
        return;
      }

      try {
        // Execute command on LSP server with URI and scope
        const result = await client.sendRequest('workspace/executeCommand', {
          command: 'qmdc.runSqlQuery',
          arguments: [sql, uri.toString(), scope]
        }) as any;

        // Show output channel and focus on it
        sqlOutputChannel.clear();
        sqlOutputChannel.show();

        if (result?.success) {
          sqlOutputChannel.appendLine(`=== QMDC v${extensionVersion} | Query (line ${line + 1}) ===`);
          sqlOutputChannel.appendLine(`Query: ${sql}`);
          sqlOutputChannel.appendLine(`Stats: ${result.stats?.objects || 0} objects, ${result.stats?.edges || 0} edges`);
          sqlOutputChannel.appendLine(`Rows: ${result.row_count}`);
          sqlOutputChannel.appendLine('');
          sqlOutputChannel.appendLine(result.table || '(no results)');
        } else {
          sqlOutputChannel.appendLine(`=== QMDC v${extensionVersion} | Query Error ===`);
          sqlOutputChannel.appendLine(`Query: ${sql}`);
          sqlOutputChannel.appendLine('');
          sqlOutputChannel.appendLine(`Error: ${result?.error || 'Unknown error'}`);
          vscode.window.showErrorMessage(`Query Error: ${result?.error || 'Unknown error'}`);
        }
      } catch (error) {
        sqlOutputChannel.clear();
        sqlOutputChannel.appendLine(`=== QMDC v${extensionVersion} | Error ===`);
        sqlOutputChannel.appendLine(`${error}`);
        sqlOutputChannel.show();
        vscode.window.showErrorMessage(`Failed to execute query: ${error}`);
      }
    }
  );
  context.subscriptions.push(runQueryFromBlockCommand);

  // Register Run SQL Query command
  const runSqlQueryCommand = vscode.commands.registerCommand('qmdc.runSqlQuery', async () => {
    if (!client) {
      vscode.window.showErrorMessage('QMDC Language Server not running');
      return;
    }
    
    // Wait for client to be ready
    if (client.state !== State.Running) {
      vscode.window.showInformationMessage('Waiting for QMDC Language Server to start...');
      // Wait a bit for server to start
      await new Promise<void>((resolve) => {
        const checkState = () => {
          if (client && client.state === State.Running) {
            resolve();
          } else {
            setTimeout(checkState, 100);
          }
        };
        checkState();
      });
    }

    // Show input box for SQL query
    const sql = await vscode.window.showInputBox({
      prompt: 'Enter SQL query',
      placeHolder: 'SELECT * FROM objects LIMIT 10',
      value: 'SELECT __id, __kind, __label, __namespace FROM objects LIMIT 20',
      validateInput: (value) => {
        if (!value.trim()) {
          return 'SQL query cannot be empty';
        }
        return null;
      }
    });

    if (!sql) {
      return; // User cancelled
    }

    try {
      // Execute command on LSP server
      const result = await client.sendRequest('workspace/executeCommand', {
        command: 'qmdc.runSqlQuery',
        arguments: [sql]
      }) as any;

      // Show output channel and focus on it
      sqlOutputChannel.clear();
      sqlOutputChannel.show(); // No preserveFocus - switch to panel

      if (result?.success) {
        sqlOutputChannel.appendLine(`=== QMDC v${extensionVersion} | SQL Query ===`);
        sqlOutputChannel.appendLine(`Query: ${sql}`);
        sqlOutputChannel.appendLine(`Stats: ${result.stats?.objects || 0} objects, ${result.stats?.edges || 0} edges`);
        sqlOutputChannel.appendLine(`Rows: ${result.row_count}`);
        sqlOutputChannel.appendLine('');
        sqlOutputChannel.appendLine(result.table || '(no results)');
      } else {
        sqlOutputChannel.appendLine(`=== QMDC v${extensionVersion} | SQL Error ===`);
        sqlOutputChannel.appendLine(`Query: ${sql}`);
        sqlOutputChannel.appendLine('');
        sqlOutputChannel.appendLine(`Error: ${result?.error || 'Unknown error'}`);
        vscode.window.showErrorMessage(`SQL Error: ${result?.error || 'Unknown error'}`);
      }
    } catch (error) {
      sqlOutputChannel.clear();
      sqlOutputChannel.appendLine(`=== QMDC v${extensionVersion} | Error ===`);
      sqlOutputChannel.appendLine(`${error}`);
      sqlOutputChannel.show(); // Focus on panel to show error
      vscode.window.showErrorMessage(`Failed to execute SQL: ${error}`);
    }
  });
  context.subscriptions.push(runSqlQueryCommand);

  // Track which document is being previewed
  let previewDocumentUri: vscode.Uri | undefined;
  const previewHistory: vscode.Uri[] = [];
  const previewForwardHistory: vscode.Uri[] = [];

  // Function to update preview content
  async function updatePreview(document: vscode.TextDocument, scrollToId?: string) {
    if (!previewPanel || !client || client.state !== State.Running) {
      return;
    }
    
    try {
      const t0 = Date.now();
      previewPanel.title = `Preview: ${path.basename(document.fileName)}`;
      let queryCount = 0;
      let queryTotalMs = 0;
      const queryExecutor: QueryExecutor = {
        async executeQuery(sql: string, documentUri: string, scope: string) {
          const qt0 = Date.now();
          const result = await client!.sendRequest('workspace/executeCommand', {
            command: 'qmdc.runSqlQuery',
            arguments: [sql, documentUri, scope]
          }) as any;
          queryTotalMs += Date.now() - qt0;
          queryCount++;
          return result;
        }
      };

      // Resolve mermaid script URI for the webview
      const mermaidPath = path.join(context.extensionPath, 'node_modules', 'mermaid', 'dist', 'mermaid.min.js');
      const mermaidUri = previewPanel.webview.asWebviewUri(vscode.Uri.file(mermaidPath));

      // Resolve local <img> paths to webview URIs (relative to the document's dir),
      // so screenshots/media embedded with ![](...) actually load in the webview.
      const docDir = path.dirname(document.uri.fsPath);
      const panel = previewPanel;
      const resolveImageSrc = (src: string): string => {
        try {
          const abs = path.isAbsolute(src) ? src : path.resolve(docDir, decodeURIComponent(src));
          return panel.webview.asWebviewUri(vscode.Uri.file(abs)).toString();
        } catch {
          return src;
        }
      };

      const t1 = Date.now();
      const html = await generatePreviewHtml(
        document.getText(),
        queryExecutor,
        document.uri.toString(),
        { includeVscodeApi: true, mermaidScript: mermaidUri.toString(), scrollToId, resolveImageSrc }
      );
      const t2 = Date.now();
      previewPanel.webview.html = html;
      const t3 = Date.now();
      log(`[PERF] updatePreview: setup=${t1-t0}ms, generateHtml=${t2-t1}ms (${(html.length/1024).toFixed(0)}KB, ${queryCount} queries in ${queryTotalMs}ms), setHtml=${t3-t2}ms, total=${t3-t0}ms`);
    } catch (error) {
      previewPanel.webview.html = `<html><body><h1>Error</h1><pre>${error}</pre></body></html>`;
    }
  }

  // Helper function to open preview with specified view column
  async function openPreviewInColumn(viewColumn: vscode.ViewColumn) {
    log(`openPreviewInColumn called with viewColumn: ${viewColumn} (Active=${vscode.ViewColumn.Active}, Beside=${vscode.ViewColumn.Beside})`);
    
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'qmdmd') {
      log(`ERROR: No QMDC editor found. Editor: ${editor ? editor.document.languageId : 'null'}`);
      vscode.window.showErrorMessage('Open a QMD.md file first');
      return;
    }

    if (!client || client.state !== State.Running) {
      log(`ERROR: LSP client not running. State: ${client?.state}`);
      vscode.window.showErrorMessage('QMDC Language Server not running');
      return;
    }

    const document = editor.document;
    log(`Opening preview for document: ${path.basename(document.fileName)}`);

    // Determine actual view column
    let actualViewColumn = viewColumn;
    if (viewColumn === vscode.ViewColumn.Active) {
      // For active column, use the editor's exact view column
      // This should open the panel in the same column as the editor
      const editorColumn = editor.viewColumn;
      
      log(`Opening preview in active column. Editor column: ${editorColumn}`);
      
      // Use editor's column if available and valid, otherwise use column 1
      if (editorColumn !== undefined && editorColumn !== null && typeof editorColumn === 'number' && editorColumn >= 1) {
        actualViewColumn = editorColumn;
      } else {
        // Fallback: try to get active editor's column or use One
        const activeEditor = vscode.window.activeTextEditor;
        if (activeEditor?.viewColumn !== undefined && activeEditor.viewColumn >= 1) {
          actualViewColumn = activeEditor.viewColumn;
        } else {
          actualViewColumn = vscode.ViewColumn.One;
        }
      }
      
      log(`Using view column: ${actualViewColumn} for active preview`);
    } else {
      // For split screen, use Beside (opens to the right)
      actualViewColumn = vscode.ViewColumn.Beside;
      log(`Opening preview in split screen (Beside)`);
    }

    // For active column, always recreate panel to ensure it opens in correct column
    // For split screen, can reuse existing panel if it's already in the right place
    if (previewPanel && viewColumn === vscode.ViewColumn.Active) {
      // Always dispose and recreate when opening in active column
      log(`Disposing existing panel (was in column ${previewPanel.viewColumn}) to recreate in active column`);
      previewPanel.dispose();
      previewPanel = undefined;
    }
    
    // Create or reveal preview panel
    if (previewPanel) {
      // For split screen, just reveal existing panel
      previewPanel.reveal(actualViewColumn);
      log(`Revealed existing panel in column: ${actualViewColumn}, actual column: ${previewPanel.viewColumn}`);
    } else {
      // Create new panel.
      // localResourceRoots must include every dir the webview may load files from:
      // the bundled node_modules (mermaid), all workspace folders, and the current
      // document's dir — so local images referenced via ![](...) resolve.
      const resourceRoots: vscode.Uri[] = [
        vscode.Uri.file(path.join(context.extensionPath, 'node_modules')),
        vscode.Uri.file(path.dirname(document.uri.fsPath)),
      ];
      for (const folder of vscode.workspace.workspaceFolders ?? []) {
        resourceRoots.push(folder.uri);
      }
      previewPanel = vscode.window.createWebviewPanel(
        'qmdcPreview',
        `Preview: ${path.basename(document.fileName)}`,
        actualViewColumn,
        {
          enableScripts: true,
          retainContextWhenHidden: true,
          localResourceRoots: resourceRoots,
        }
      );
      log(`Created new panel in column: ${actualViewColumn}, actual column: ${previewPanel.viewColumn}`);

      previewPanel.onDidDispose(() => {
        previewPanel = undefined;
        previewDocumentUri = undefined;
        previewHistory.length = 0;
        previewForwardHistory.length = 0;
      });

      // Handle messages from webview
      previewPanel.webview.onDidReceiveMessage(async (message) => {
        if (message.type === 'navigateBack') {
          if (previewHistory.length > 0 && previewDocumentUri) {
            // Push current to forward stack before going back
            previewForwardHistory.push(previewDocumentUri);
            const previousUri = previewHistory.pop()!;
            previewDocumentUri = previousUri;
            const doc = await vscode.workspace.openTextDocument(previousUri);
            await updatePreview(doc);
          }
          return;
        }

        if (message.type === 'navigateForward') {
          if (previewForwardHistory.length > 0 && previewDocumentUri) {
            // Push current to back stack before going forward
            previewHistory.push(previewDocumentUri);
            const nextUri = previewForwardHistory.pop()!;
            previewDocumentUri = nextUri;
            const doc = await vscode.workspace.openTextDocument(nextUri);
            await updatePreview(doc);
          }
          return;
        }

        if (message.type === 'navigateToRef' && client) {
          const refId = message.refId;
          const navT0 = Date.now();
          
          try {
            if (!previewDocumentUri) {
              vscode.window.showWarningMessage(`No document context for navigation`);
              return;
            }

            // Strategy 1: If the ref exists as [[#refId]] in the current document,
            // use textDocument/definition for precise resolution (handles qualified refs)
            const previewDoc = await vscode.workspace.openTextDocument(previewDocumentUri);
            const text = previewDoc.getText();
            const refPattern = `[[#${refId}]]`;
            const refIndex = text.indexOf(refPattern);

            let targetUri: vscode.Uri | null = null;
            let scrollTarget = refId.split(':').pop() || refId;

            if (refIndex >= 0) {
              const pos = previewDoc.positionAt(refIndex + 3);
              const navT1 = Date.now();
              let definitions = await client.sendRequest('textDocument/definition', {
                textDocument: { uri: previewDocumentUri.toString() },
                position: { line: pos.line, character: pos.character },
              }) as any[];
              log(`[PERF] navigateToRef '${refId}': definition request=${Date.now()-navT1}ms`);

              if (definitions && !Array.isArray(definitions)) {
                definitions = [definitions];
              }
              if (definitions && definitions.length > 0) {
                const def = definitions[0];
                targetUri = vscode.Uri.parse(def.uri || def.targetUri);
              }
            }

            // Strategy 2: Direct SQL lookup by __id (reliable for sidebar/search navigation)
            if (!targetUri) {
              const navT2 = Date.now();
              const result = await client.sendRequest('workspace/executeCommand', {
                command: 'qmdc.runSqlQuery',
                arguments: [
                  `SELECT __file FROM objects WHERE __id = '${refId.replace(/'/g, "''")}'`,
                  previewDocumentUri.toString(),
                  'workspace'
                ]
              }) as any;
              log(`[PERF] navigateToRef '${refId}': SQL lookup=${Date.now()-navT2}ms`);

              if (result?.success && result?.rows?.length > 0) {
                const targetFile = result.rows[0][0];
                if (targetFile) {
                  // Resolve file path relative to workspace
                  const workspaceFolders = vscode.workspace.workspaceFolders;
                  if (workspaceFolders) {
                    for (const folder of workspaceFolders) {
                      const fullPath = path.join(folder.uri.fsPath, targetFile);
                      if (fs.existsSync(fullPath)) {
                        targetUri = vscode.Uri.file(fullPath);
                        break;
                      }
                    }
                  }
                }
              }
            }

            if (targetUri) {
              previewHistory.push(previewDocumentUri);
              previewForwardHistory.length = 0;
              const doc = await vscode.workspace.openTextDocument(targetUri);
              previewDocumentUri = targetUri;
              await updatePreview(doc, scrollTarget);
              log(`[PERF] navigateToRef '${refId}': total=${Date.now()-navT0}ms`);
            } else {
              vscode.window.showWarningMessage(`Object '${refId}' not found`);
            }
          } catch (error) {
            vscode.window.showErrorMessage(`Failed to navigate: ${error}`);
          }
        }
      });
    }

    // Set the document URI after panel creation/dispose to avoid the onDidDispose
    // callback clearing it (dispose runs synchronously and wipes previewDocumentUri).
    previewDocumentUri = document.uri;

    // Generate and set HTML content
    await updatePreview(document);
  }

  // Register Open Preview command (opens in active column)
  const openPreviewCommand = vscode.commands.registerCommand('qmdc.openPreview', async () => {
    await openPreviewInColumn(vscode.ViewColumn.Active);
  });
  context.subscriptions.push(openPreviewCommand);

  // Register Open Preview Beside command (opens in split screen)
  const openPreviewBesideCommand = vscode.commands.registerCommand('qmdc.openPreviewBeside', async () => {
    await openPreviewInColumn(vscode.ViewColumn.Beside);
  });
  context.subscriptions.push(openPreviewBesideCommand);

  // Status bar button for Open Preview (active column)
  const previewStatusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    100
  );
  previewStatusBarItem.command = 'qmdc.openPreview';
  previewStatusBarItem.text = 'QMDC: $(open-preview) Preview';
  previewStatusBarItem.tooltip = 'Open QMDC Preview';
  
  // Status bar button for Open Preview Beside (split screen)
  const previewBesideStatusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    99
  );
  previewBesideStatusBarItem.command = 'qmdc.openPreviewBeside';
  previewBesideStatusBarItem.text = 'QMDC: $(split-horizontal) Split';
  previewBesideStatusBarItem.tooltip = 'Open QMDC Preview in Split Screen';
  
  // Show status bar buttons only for QMD.md files
  const updateStatusBar = () => {
    const editor = vscode.window.activeTextEditor;
    if (editor && editor.document.languageId === 'qmdmd') {
      previewStatusBarItem.show();
      previewBesideStatusBarItem.show();
    } else {
      previewStatusBarItem.hide();
      previewBesideStatusBarItem.hide();
    }
  };
  
  updateStatusBar();
  context.subscriptions.push(
    previewStatusBarItem,
    previewBesideStatusBarItem,
    vscode.window.onDidChangeActiveTextEditor(updateStatusBar),
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document === vscode.window.activeTextEditor?.document) {
        updateStatusBar();
      }
    })
  );

  // Register Copy Global ID command
  const copyGlobalIdCommand = vscode.commands.registerCommand('qmdc.copyGlobalId', async (item: any) => {
    if (!item || !item.objectId) {
      vscode.window.showWarningMessage('No object selected');
      return;
    }

    const workspaceId = item.workspaceId || '';
    const namespaceId = item.namespaceId || '';
    const objectId = item.objectId;

    // Compute __global_id
    const globalId = namespaceId 
      ? `${workspaceId}:${namespaceId}:${objectId}`
      : `${workspaceId}::${objectId}`;

    await vscode.env.clipboard.writeText(globalId);
    vscode.window.showInformationMessage(`Copied: ${globalId}`);
  });
  context.subscriptions.push(copyGlobalIdCommand);

  // Register Reveal in Explorer command
  const revealInExplorerCommand = vscode.commands.registerCommand('qmdc.revealInExplorer', async (item: any) => {
    if (!item?.objectData?.file) {
      vscode.window.showWarningMessage('No file associated with this object');
      return;
    }

    const filePath = item.objectData.file;
    const workspacePath = item.workspacePath || '';
    
    // Resolve file path
    let fullPath = filePath;
    if (!path.isAbsolute(filePath) && workspacePath) {
      fullPath = path.join(workspacePath, filePath);
    }

    const fileUri = vscode.Uri.file(fullPath);
    await vscode.commands.executeCommand('revealInExplorer', fileUri);
  });
  context.subscriptions.push(revealInExplorerCommand);

  // Register Preview Object File command
  const previewObjectFileCommand = vscode.commands.registerCommand('qmdc.previewObjectFile', async (item: any) => {
    if (!item?.objectData?.file) {
      vscode.window.showWarningMessage('No file associated with this object');
      return;
    }

    const filePath = item.objectData.file;
    const workspacePath = item.workspacePath || '';
    
    // Resolve file path
    let fullPath = filePath;
    if (!path.isAbsolute(filePath) && workspacePath) {
      fullPath = path.join(workspacePath, filePath);
    }

    const fileUri = vscode.Uri.file(fullPath);
    
    // Open the document in editor first, then trigger preview
    await vscode.window.showTextDocument(fileUri);
    await vscode.commands.executeCommand('qmdc.openPreview');
  });
  context.subscriptions.push(previewObjectFileCommand);

  // Live Preview: update on document save
  const onSaveDisposable = vscode.workspace.onDidSaveTextDocument(async (document) => {
    if (!previewPanel || !previewDocumentUri) {
      return;
    }
    
    // Update if same document OR if any QMD.md file changed (queries might reference other files)
    if (document.languageId === 'qmdmd') {
      // Re-fetch the previewed document (it might have been updated by LSP)
      try {
        const previewDoc = await vscode.workspace.openTextDocument(previewDocumentUri);
        await updatePreview(previewDoc);
      } catch {
        // Document might have been deleted
      }
    }
  });
  context.subscriptions.push(onSaveDisposable);

  // Start the client
  try {
    await client.start();
    console.log('QMDC Language Server started successfully');
    
    // Initialize explorer with client
    explorerProvider.setClient(client);
    
    // Listen for workspace update notifications from LSP
    client.onNotification('qmdc/workspaceUpdated', () => {
      log('Received qmdc/workspaceUpdated notification');
      explorerProvider.refresh();
    });
    
    // Also refresh on save as fallback (in case notification is missed)
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === 'qmdmd') {
        // Small delay to let LSP process the save
        setTimeout(() => {
          explorerProvider.refresh();
        }, 100);
      }
    });
  } catch (error) {
    console.error('Failed to start QMDC Language Server:', error);
    vscode.window.showErrorMessage(
      `Failed to start QMDC Language Server: ${error}`
    );
  }
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}
