/**
 * Public API of `@qmdc/qmdc` — the only module `exports` exposes.
 *
 * Mirrors the root of the other two libraries: qmdc-py's `__all__` and the
 * `pub use` list in qmdc-rs/src/lib.rs. Anything not re-exported here is
 * internal and may change without a major version bump.
 */

export { parse, rebuild, stringifyParseResult } from './parser.js';
export type { OutputFormat, ParseOptions, ParseResult, QmdcObject } from './parser.js';

export {
  buildIndex,
  findWorkspaceRoot,
  parseAllWorkspaces,
  parseWorkspace,
  resolveReference,
  resolveWorkspace,
  scanWorkspace,
  validateWorkspace,
  workspaceToJson,
} from './workspace.js';
export type { WorkspaceError, WorkspaceIndex, WorkspaceResult } from './workspace.js';

export { QmdcDatabase, executeQuery } from './db.js';
export type { QueryResult } from './db.js';
