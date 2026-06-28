/**
 * CLI commands using Commander
 */

import { Command } from 'commander';
import { readFileSync } from 'fs';
import { parse, rebuild } from './parser.js';
import {
  resolveWorkspace,
  scanWorkspace,
  workspaceToJson,
  type WorkspaceResult,
} from './workspace.js';
import { executeQuery, QmdcDatabase } from './db.js';

const program = new Command();

program.name('qmdc').description('QMDC Parser - Convert QMD.md to JSON and back').version('0.1.0');

program
  .command('parse')
  .description('Parse QMD.md to JSON')
  .option('-i, --input <path>', 'Input QMD.md file (default: stdin)')
  .option('-o, --output <path>', 'Output JSON file (default: stdout)')
  .option('-f, --format <format>', 'Output format: minimal, standard, full', 'standard')
  .option('-v, --verbose', 'Increase verbosity', (_, prev) => prev + 1, 0)
  .option('--strict', 'Fail-fast mode')
  .option('--no-comments', 'Exclude __comments from output')
  .option('--no-syntax', 'Exclude __syntax from output')
  .option('--no-pretty', 'Disable JSON formatting')
  .action(async (options) => {
    try {
      let markdown: string;

      if (options.input) {
        markdown = readFileSync(options.input, 'utf-8');
      } else {
        // Read from stdin
        markdown = readFileSync(0, 'utf-8');
      }

      const result = parse(markdown, { format: options.format });

      // Remove metadata if requested (Commander: --no-X sets options.X = false)
      if (options.comments === false) {
        for (const obj of result) {
          delete obj.__comments;
        }
      }

      if (options.syntax === false) {
        for (const obj of result) {
          delete obj.__syntax;
        }
      }

      // Output (default pretty=true unless --no-pretty)
      const json =
        options.pretty === false ? JSON.stringify(result) : JSON.stringify(result, null, 2);

      if (options.output) {
        const { writeFileSync } = await import('fs');
        writeFileSync(options.output, json);
      } else {
        console.log(json);
      }
    } catch (error) {
      console.error('Error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  });

program
  .command('rebuild')
  .description('Rebuild QMD.md from JSON')
  .option('-i, --input <path>', 'Input JSON file (default: stdin)')
  .option('-o, --output <path>', 'Output QMD.md file (default: stdout)')
  .option('-v, --verbose', 'Increase verbosity', (_, prev) => prev + 1, 0)
  .action(async (options) => {
    try {
      let jsonText: string;

      if (options.input) {
        jsonText = readFileSync(options.input, 'utf-8');
      } else {
        // Read from stdin
        jsonText = readFileSync(0, 'utf-8');
      }

      const data = JSON.parse(jsonText);
      const result = rebuild(data);

      if (options.output) {
        const { writeFileSync } = await import('fs');
        writeFileSync(options.output, result);
      } else {
        process.stdout.write(result);
        if (!result.endsWith('\n')) {
          process.stdout.write('\n');
        }
      }
    } catch (error) {
      console.error('Error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  });

// Workspace commands
const workspace = program.command('workspace').description('Workspace operations');

workspace
  .command('parse')
  .description('Parse entire workspace')
  .argument('<path>', 'Path to workspace root')
  .option('-o, --output <path>', 'Output JSON file (default: stdout)')
  .option('--no-pretty', 'Disable JSON formatting')
  .action(async (pathArg: string, options) => {
    try {
      // QMD-59: unified resolver — walk-up then walk-down.
      const result: WorkspaceResult = resolveWorkspace(pathArg);
      const json = workspaceToJson(result);
      const output =
        options.pretty === false ? JSON.stringify(json) : JSON.stringify(json, null, 2);

      if (options.output) {
        const { writeFileSync } = await import('fs');
        writeFileSync(options.output, output);
      } else {
        console.log(output);
      }
    } catch (error) {
      console.error('Error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  });

workspace
  .command('validate')
  .description('Validate workspace for errors. Returns JSON array of errors.')
  .argument('<path>', 'Path to workspace root')
  .action((pathArg: string) => {
    try {
      // QMD-59: unified resolver — walk-up then walk-down.
      const result: WorkspaceResult = resolveWorkspace(pathArg);

      // Output only errors array as JSON
      const errorsArray = result.errors.map((e) => ({
        type: e.type,
        message: e.message,
        file: e.file ?? null,
        line: e.line ?? null,
        objectId: e.objectId ?? null,
        fieldName: e.fieldName ?? null,
        reference: e.reference ?? null,
        candidates: e.candidates ?? null,
        severity: e.severity,
      }));

      console.log(JSON.stringify(errorsArray, null, 2));

      // Exit with error code if there are errors
      process.exit(result.errors.length > 0 ? 1 : 0);
    } catch (error) {
      console.error('Error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  });

workspace
  .command('files')
  .description('List files in workspace')
  .argument('<path>', 'Path to workspace root')
  .action((pathArg: string) => {
    try {
      const files = scanWorkspace(pathArg);
      for (const file of files) {
        console.log(file);
      }
    } catch (error) {
      console.error('Error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  });

// Query command
program
  .command('query')
  .description('Execute SQL query against workspace')
  .argument('<workspace>', 'Workspace directory path')
  .argument('<query>', 'SQL query or "#query_id" for Query object reference')
  .option('-f, --format <format>', 'Output format: table or json', 'table')
  .action(async (workspacePath: string, query: string, options) => {
    try {
      // QMD-59: unified resolver — walk-up then walk-down (query from any dir).
      const ws = resolveWorkspace(workspacePath);
      const result = await executeQuery(ws, query);

      if (options.format === 'json') {
        console.log(JSON.stringify({ columns: result.columns, rows: result.rows }, null, 2));
      } else {
        process.stdout.write(QmdcDatabase.toTableString(result));
      }
    } catch (error) {
      console.error('Error:', error instanceof Error ? error.message : error);
      process.exit(1);
    }
  });

export { program };
