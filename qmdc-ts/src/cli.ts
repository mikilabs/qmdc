/**
 * CLI commands using Commander
 */

import { Command } from 'commander';
import { readFileSync } from 'fs';
import { parse, rebuild, stringifyParseResult } from './parser.js';
import {
  resolveWorkspaceInput,
  scanWorkspace,
  workspaceToJson,
  WorkspaceUsageError,
  type WorkspaceResult,
} from './workspace.js';
import { executeQuery, QmdcDatabase } from './db.js';

const program = new Command();

/**
 * Collector for the repeatable `-w` / `--with` option (QMD-72).
 *
 * Commander calls this once per occurrence, so every path is kept as a peer instead of the
 * last one winning.
 */
function collectWith(value: string, previous: string[]): string[] {
  return previous.concat([value]);
}

/**
 * Exit for an error thrown out of a workspace command.
 *
 * A refused INVOCATION is a usage error (exit 2); anything else is a failure to produce a
 * result (exit 1). Keeping the two apart is what lets a conformance fixture assert on the
 * exit code.
 */
function exitUsageOrError(error: unknown): never {
  if (error instanceof WorkspaceUsageError) {
    console.error(`error: ${error.message}`);
    process.exit(2);
  }
  console.error('Error:', error instanceof Error ? error.message : error);
  process.exit(1);
}

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

      // Output (default pretty=true unless --no-pretty).
      // QMD-71: via stringifyParseResult, so a float keeps the precision the author wrote.
      const json = stringifyParseResult(result, options.pretty !== false);

      if (options.output) {
        const { writeFileSync } = await import('fs');
        writeFileSync(options.output, json);
      } else {
        console.log(json);
      }

      // #11: a document that produced __ParsingError objects is not a clean parse, so the
      // exit code says so (the output above is still complete). The minimal format drops
      // system kinds, so it cannot be counted from its own output.
      const counted = options.format === 'minimal' ? parse(markdown) : result;
      const errors = counted.filter((obj) => obj.__kind === '__ParsingError').length;
      if (errors > 0) {
        console.error(
          `error: document has ${errors} parsing error(s); see the __ParsingError objects in the output`
        );
        process.exitCode = 1;
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
  .argument('[path]', 'Path to workspace root (omit when composing with --with)')
  .option(
    '-w, --with <path>',
    'Compose this workspace explicitly; repeatable. Mutually exclusive with PATH.',
    collectWith,
    []
  )
  .option('-o, --output <path>', 'Output JSON file (default: stdout)')
  .option('--no-pretty', 'Disable JSON formatting')
  .action(async (pathArg: string | undefined, options) => {
    try {
      // QMD-59: unified resolver — walk-up then walk-down.
      // QMD-72: or compose the explicitly-supplied `--with` workspaces.
      const result: WorkspaceResult = resolveWorkspaceInput(pathArg, options.with as string[]);
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
      exitUsageOrError(error);
    }
  });

workspace
  .command('validate')
  .description('Validate workspace for errors. Returns JSON array of errors.')
  .argument('[path]', 'Path to workspace root (omit when composing with --with)')
  .option(
    '-w, --with <path>',
    'Compose this workspace explicitly; repeatable. Mutually exclusive with PATH.',
    collectWith,
    []
  )
  .action((pathArg: string | undefined, options) => {
    try {
      // QMD-59: unified resolver — walk-up then walk-down.
      // QMD-72: or compose the explicitly-supplied `--with` workspaces.
      const result: WorkspaceResult = resolveWorkspaceInput(pathArg, options.with as string[]);

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
      exitUsageOrError(error);
    }
  });

workspace
  .command('files')
  .description('List files in workspace')
  .argument('[path]', 'Path to workspace root (omit when composing with --with)')
  .option(
    '-w, --with <path>',
    'Compose this workspace explicitly; repeatable. Mutually exclusive with PATH.',
    collectWith,
    []
  )
  .action((pathArg: string | undefined, options) => {
    try {
      const withPaths = options.with as string[];
      if (withPaths.length > 0) {
        const result = resolveWorkspaceInput(pathArg, withPaths);
        for (const file of result.files) {
          console.log(file);
        }
        return;
      }
      const files = scanWorkspace(pathArg ?? '.');
      for (const file of files) {
        console.log(file);
      }
    } catch (error) {
      exitUsageOrError(error);
    }
  });

// Query command
program
  .command('query')
  .description('Execute SQL query against workspace')
  .argument('[workspace]', 'Workspace directory path, or the QUERY itself when --with is used')
  .argument('[query]', 'SQL query or "#query_id" for Query object reference')
  .option(
    '-w, --with <path>',
    'Compose this workspace explicitly; repeatable. Mutually exclusive with WORKSPACE.',
    collectWith,
    []
  )
  .option('-f, --format <format>', 'Output format: table or json', 'table')
  .action(async (workspacePath: string | undefined, query: string | undefined, options) => {
    // QMD-72: with `--with`, the single remaining positional IS the query — a composed set
    // has no one path, so there is nothing for a path positional to name. Both positionals
    // alongside `--with` is the mutually-exclusive usage error, not a path to ignore.
    const withPaths = options.with as string[];
    let wsPath = workspacePath;
    let sql = query;
    if (withPaths.length > 0) {
      if (workspacePath !== undefined && query !== undefined) {
        console.error(
          'error: a positional WORKSPACE and --with are mutually exclusive; ' +
            'pass every workspace as --with'
        );
        process.exit(2);
      }
      sql = workspacePath;
      wsPath = undefined;
    }
    if (sql === undefined) {
      console.error('error: a QUERY is required');
      process.exit(2);
    }

    try {
      // QMD-59: unified resolver — walk-up then walk-down (query from any dir).
      // QMD-72: or compose the explicitly-supplied `--with` workspaces.
      const ws = resolveWorkspaceInput(wsPath, withPaths);
      const result = await executeQuery(ws, sql);

      if (options.format === 'json') {
        console.log(JSON.stringify({ columns: result.columns, rows: result.rows }, null, 2));
      } else {
        process.stdout.write(QmdcDatabase.toTableString(result));
      }
    } catch (error) {
      exitUsageOrError(error);
    }
  });

export { program };
