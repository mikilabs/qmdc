/**
 * QMDC Workspace - Multi-file parsing with cross-file references.
 */

import { readFileSync, readdirSync, statSync, existsSync, realpathSync } from 'fs';
import { join, relative, dirname, resolve, sep } from 'path';
import { isIgnored, loadQmdcignore } from './ignore.js';
import { parse, type QmdcObject, isInsideBackticks } from './parser.js';

export interface WorkspaceError {
  type:
    | 'broken_link'
    | 'broken_parent'
    | 'duplicate_id'
    | 'ambiguous_reference'
    | 'ambiguous_field_reference'
    | 'nested_workspace'
    | 'workspace_in_wrong_file'
    | 'structured_in_textblock'
    | 'dangling_field'
    | 'multiple_definitions'
    | 'explicit_system_type'
    | 'mixed_field_keys'
    | 'nested_subitems'
    | 'ordered_list_in_array'
    | 'table_in_array'
    | 'extra_table_in_array'
    | 'mixed_array'
    | 'block_in_inline_field'
    | 'unsupported_number_format';
  message: string;
  file?: string;
  line?: number;
  objectId?: string;
  fieldName?: string;
  reference?: string;
  candidates?: string[];
  severity: 'error' | 'warning';
}

export interface WorkspaceIndex {
  byId: Record<string, QmdcObject[]>;
  byGlobalId: Record<string, QmdcObject>;
  byKind: Record<string, QmdcObject[]>;
  byFile: Record<string, QmdcObject[]>;
  byNamespace: Record<string, QmdcObject[]>;
  byLocalId: Record<string, QmdcObject[]>;
}

/**
 * One workspace inside a result (QMD-72): where it is on disk, and where its files sit in the
 * result's `__file` values.
 *
 * `__file` is relative to the result's base, and that keeps it unique within one result: two
 * workspaces' `readme.qmd.md` must never collapse into one string, because the reference
 * scanner, the error reports and the `files` list all key on it. `path` is where this
 * workspace sits inside the base (`''` when it IS the base) and `root` is where that place
 * really is, so a consumer locates any file with one rule: take the entry whose `path` is the
 * longest prefix of `__file` and join its `root` with the rest.
 */
export interface WorkspaceEntry {
  id: string;
  root: string;
  path: string;
}

export interface WorkspaceResult {
  /**
   * The base every `__file` is relative to, as a canonical absolute path; `null` when the
   * base is virtual -- the `-w` form, whose workspaces need not share any directory, puts each
   * one at its own id instead (QMD-72).
   */
  root: string | null;
  workspaceId: string | null;
  /** Every workspace in the result, ordered by `path` (QMD-72). */
  workspaces: WorkspaceEntry[];
  files: string[];
  objects: QmdcObject[];
  index: WorkspaceIndex;
  errors: WorkspaceError[];
}

/**
 * Canonical absolute form of `p` with `/` separators (QMD-72). Canonical rather than
 * lexical, because the `-w` duplicate check already treats two spellings of one directory (a
 * symlink and its target) as the same path; the reported root has to agree with that.
 * Mirrors `canonical_slash` in the Rust parser.
 */
function canonicalSlash(p: string): string {
  let abs: string;
  try {
    abs = realpathSync(p);
  } catch {
    abs = resolve(p);
  }
  return sep === '/' ? abs : abs.split(sep).join('/');
}

/** The entry for a single-workspace result: the workspace IS the base (`path` `''`). */
function singleEntry(workspaceId: string | null, root: string): WorkspaceEntry[] {
  return workspaceId ? [{ id: workspaceId, root: canonicalSlash(root), path: '' }] : [];
}

/**
 * Extract namespace ID - now just returns the value as-is (plain ID format).
 */
function extractNamespaceId(namespaceRef: string): string {
  return namespaceRef ?? '';
}

/**
 * Shared `__Workspace` marker check. Detects `[[id: __Workspace]]` in readme
 * content, allowing optional whitespace after the colon. Single source of truth
 * for workspace-root detection (avoids divergent inline regexes).
 */
const WORKSPACE_MARKER_RE = /\[\[[^\]]+:\s*__Workspace\]\]/;

/**
 * Mirrors WORKSPACE_SCAN_MAX_DEPTH in the Rust parser (qmdc-rs/src/workspace.rs) and in
 * Python (qmdc-py/qmdc/workspace.py), and the depth documented in docs/mcp/readme.qmd.md.
 * A caller-supplied path is scanned downward at most this many directory levels, so
 * pointing a tool at a large checkout cannot turn one call into a full-tree crawl. The
 * three implementations must share the number: a marker deeper than this is undiscovered
 * everywhere, or the same `-w` invocation succeeds in one implementation and is a usage
 * error in another.
 */
export const WORKSPACE_SCAN_MAX_DEPTH = 5;

function contentHasWorkspaceMarker(content: string): boolean {
  return WORKSPACE_MARKER_RE.test(content);
}

/**
 * Find workspace root by searching for readme.qmd.md with __Workspace object.
 */
export function findWorkspaceRoot(startPath: string): string | null {
  // Make the path absolute so ancestor traversal works for relative inputs
  // (e.g. `.`). Note: this only absolutizes — it does NOT resolve symlinks,
  // unlike Rust's fs::canonicalize. It mirrors Python's os.path.abspath-style
  // behavior used by the other parsers for workspace root discovery.
  let path = resolve(startPath);

  // If it's a file, start from its directory
  try {
    if (statSync(path).isFile()) {
      path = dirname(path);
    }
  } catch {
    // Path may not exist; still attempt to walk up from the resolved location.
  }

  // Search up the tree
  while (true) {
    const readme = join(path, 'readme.qmd.md');
    try {
      const content = readFileSync(readme, 'utf-8');
      // Check if it contains __Workspace kind
      if (contentHasWorkspaceMarker(content)) {
        return path;
      }
    } catch {
      // File doesn't exist, continue up
    }

    const parent = dirname(path);
    if (parent === path) {
      break; // Reached root
    }
    path = parent;
  }

  return null;
}

/**
 * Find all nested workspace roots within a directory.
 * Returns array of absolute paths to directories containing [[id:__Workspace]].
 */
export function findNestedWorkspaceRoots(rootPath: string): string[] {
  const roots: string[] = [];
  const ignorePatterns = loadQmdcignore(rootPath);

  function scan(dir: string): void {
    // QMD-77 D1: a directory we cannot read is skipped, exactly as rs, py and `git` itself do.
    // Letting EACCES escape returned NOTHING -- not one readable file -- for the whole scan.
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }

    for (const entry of entries) {
      if (!entry.isDirectory()) continue;

      const fullPath = join(dir, entry.name);
      const readme = join(fullPath, 'readme.qmd.md');

      // An ignored directory hides everything below it, as in git (QMD-73).
      if (isIgnored(fullPath, rootPath, ignorePatterns, true)) {
        continue;
      }

      if (!isIgnored(readme, rootPath, ignorePatterns)) {
        try {
          const content = readFileSync(readme, 'utf-8');
          if (contentHasWorkspaceMarker(content)) {
            roots.push(fullPath);
          }
        } catch {
          // No readme.qmd.md, continue scanning
        }
      }

      // Recursively scan subdirectories
      scan(fullPath);
    }
  }

  scan(rootPath);
  return roots;
}

/**
 * Scan workspace directory for all *.qmd.md files.
 * Excludes files from nested workspaces if excludeNested is true.
 */
export function scanWorkspace(rootPath: string, excludeNested = true): string[] {
  const files: string[] = [];
  const nestedRoots = excludeNested ? findNestedWorkspaceRoots(rootPath) : [];
  const ignorePatterns = loadQmdcignore(rootPath);

  function scan(dir: string): void {
    // QMD-77 D1: skip a directory we cannot read, as rs, py and `git` do.
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }

    for (const entry of entries) {
      const fullPath = join(dir, entry.name);

      if (entry.isDirectory()) {
        // Skip nested workspace directories
        if (nestedRoots.includes(fullPath)) {
          continue;
        }
        // An ignored directory hides everything below it, as in git (QMD-73).
        if (isIgnored(fullPath, rootPath, ignorePatterns, true)) {
          continue;
        }
        scan(fullPath);
      } else if (entry.isFile() && entry.name.endsWith('.qmd.md')) {
        // Check .qmdcignore before processing
        if (isIgnored(fullPath, rootPath, ignorePatterns)) {
          continue;
        }
        const relPath = relative(rootPath, fullPath);
        files.push(relPath);
      }
    }
  }

  scan(rootPath);

  // Sort: readme.qmd.md first in each directory
  files.sort((a, b) => {
    const aDirParts = a.split('/');
    const bDirParts = b.split('/');
    const aDir = aDirParts.slice(0, -1).join('/');
    const bDir = bDirParts.slice(0, -1).join('/');
    const aFile = aDirParts[aDirParts.length - 1] || '';
    const bFile = bDirParts[bDirParts.length - 1] || '';

    if (aDir !== bDir) {
      return aDir.localeCompare(bDir);
    }

    // readme.qmd.md comes first
    const aPriority = aFile === 'readme.qmd.md' ? 0 : 1;
    const bPriority = bFile === 'readme.qmd.md' ? 0 : 1;

    if (aPriority !== bPriority) {
      return aPriority - bPriority;
    }

    return aFile.localeCompare(bFile);
  });

  return files;
}

/**
 * Scan all workspaces including nested ones.
 * Returns array of workspace root paths.
 */
export function scanAllWorkspaces(rootPath: string): string[] {
  const workspaces = [rootPath];
  workspaces.push(...findNestedWorkspaceRoots(rootPath));
  return workspaces;
}

/**
 * Find __Workspace object in parsed objects.
 */
function findWorkspaceObject(objects: QmdcObject[]): QmdcObject | null {
  for (const obj of objects) {
    if (obj.__kind === '__Workspace') {
      return obj;
    }
  }
  return null;
}

/**
 * Find __Namespace object in parsed objects.
 */
function findNamespaceObject(objects: QmdcObject[]): QmdcObject | null {
  for (const obj of objects) {
    if (obj.__kind === '__Namespace') {
      return obj;
    }
  }
  return null;
}

/**
 * Get line number where object is defined.
 */
function getLineNumber(content: string, obj: QmdcObject): number {
  const objId = obj.__id || '';
  const objKind = (obj.__kind as string) || '';

  // Escape special regex characters
  const escapeRegex = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

  const patterns = [
    new RegExp(`^\\s*#+\\s+.*\\[\\[${escapeRegex(objId)}:${escapeRegex(objKind)}\\]\\]`, 'm'),
    new RegExp(`^\\s*#+\\s+.*\\[\\[${escapeRegex(objId)}\\]\\]`, 'm'),
  ];

  const lines = content.split('\n');
  for (let i = 0; i < lines.length; i++) {
    for (const pattern of patterns) {
      if (pattern.test(lines[i] || '')) {
        return i + 1;
      }
    }
  }

  return 1; // Default to line 1
}

/**
 * Wording of the `nested_workspace` report, mirroring `nested_workspace_message` in Rust.
 *
 * Nesting is reported because the outer scan leaves the inner workspace's files out, so they are
 * missing from the graph the caller asked for -- not because the two may never coexist. They may:
 * the container form composes both and drops this report, which is how a repository whose root is a
 * workspace and which also holds a `.qmdc` model workspace is validated. The message states the
 * consequence and the remedy rather than claiming nesting is forbidden (QMD-76).
 *
 * `nestedRelDir` is the inner workspace's directory relative to the outer root, so the remedy is
 * copy-pasteable and carries no absolute path.
 */
function nestedWorkspaceMessage(nestedId: string, nestedRelDir: string): string {
  return (
    `Nested workspace '${nestedId}' inside this workspace: its files are excluded from this graph. ` +
    `Validate both together: --with <root> --with <root>/${nestedRelDir}`
  );
}

/**
 * Parse entire workspace.
 */
export function parseWorkspace(rootPath: string): WorkspaceResult {
  const ignorePatterns = loadQmdcignore(rootPath);
  const files = scanWorkspace(rootPath);

  // Check for nested workspaces (this is an error)
  const nestedWorkspaceRoots = findNestedWorkspaceRoots(rootPath);
  const nestedWorkspaceErrors: WorkspaceError[] = [];

  for (const nestedRoot of nestedWorkspaceRoots) {
    const nestedReadme = join(nestedRoot, 'readme.qmd.md');
    const relPath = relative(rootPath, nestedReadme);
    const relDir = relative(rootPath, nestedRoot).split(sep).join('/');
    const content = readFileSync(nestedReadme, 'utf-8');
    const objects = parse(content);
    const wsObj = findWorkspaceObject(objects);

    if (wsObj) {
      nestedWorkspaceErrors.push({
        type: 'nested_workspace',
        message: nestedWorkspaceMessage(String(wsObj.__id), relDir),
        file: relPath,
        line: getLineNumber(content, wsObj),
        objectId: wsObj.__id,
        severity: 'error',
      });
    }
  }

  const allObjects: QmdcObject[] = [];
  let workspaceId: string | null = null;
  let workspaceRef: string | null = null;

  // First pass: find workspace and namespace definitions
  const namespaceMap: Record<string, string> = {}; // dir_path -> namespace_id

  for (const filePath of files) {
    const fullPath = join(rootPath, filePath);
    const content = readFileSync(fullPath, 'utf-8');
    const objects = parse(content, { format: 'full' });

    const fileDir = dirname(filePath) === '.' ? '' : dirname(filePath);

    // Check if this is a readme.qmd.md file (in any directory)
    const fileName = filePath.split('/').pop() || filePath;
    const isReadme = fileName === 'readme.qmd.md';

    // Check for __Workspace in readme
    if (isReadme) {
      const wsObj = findWorkspaceObject(objects);
      if (wsObj) {
        workspaceId = wsObj.__id;
        workspaceRef = workspaceId; // Store plain ID without [[#...]]
      }
    } else {
      // Check for __Workspace in non-readme file (this is an error)
      const wsObj = findWorkspaceObject(objects);
      if (wsObj) {
        const wsId = wsObj.__id;
        // Check .qmdcignore before adding error
        if (!isIgnored(fullPath, rootPath, ignorePatterns)) {
          nestedWorkspaceErrors.push({
            type: 'workspace_in_wrong_file',
            message: `Workspace '${wsId}' must be defined in readme.qmd.md, not in '${filePath}'.`,
            file: filePath,
            line: getLineNumber(content, wsObj),
            objectId: wsId,
            severity: 'error',
          });
        }
      }
    }

    // Check for __Namespace in subdirectory readme
    if (filePath.endsWith('readme.qmd.md')) {
      const nsObj = findNamespaceObject(objects);
      if (nsObj) {
        namespaceMap[fileDir] = nsObj.__id;
      }
    }
  }

  // Second pass: parse all files with full metadata
  for (const filePath of files) {
    const fullPath = join(rootPath, filePath);
    const content = readFileSync(fullPath, 'utf-8');
    const objects = parse(content, { format: 'full' });

    let fileDir = dirname(filePath);
    if (fileDir === '.') {
      fileDir = '';
    }

    // Find namespace for this file's directory
    let namespaceId: string | null = null;
    let checkDir = fileDir;

    while (checkDir) {
      const ns = namespaceMap[checkDir];
      if (ns) {
        namespaceId = ns;
        break;
      }
      const parent = dirname(checkDir);
      if (parent === '.' || parent === checkDir) {
        checkDir = '';
        break;
      }
      checkDir = parent;
    }

    // Also check current directory
    if (!namespaceId) {
      const ns = namespaceMap[fileDir];
      if (ns) {
        namespaceId = ns;
      }
    }

    // Add metadata to each object
    for (const obj of objects) {
      obj.__file = filePath;
      // Use __line from parser if available, otherwise try to find it
      if (obj.__line === undefined || obj.__line === null) {
        obj.__line = getLineNumber(content, obj);
      }

      // Add workspace reference (except for __Workspace itself)
      if (obj.__kind !== '__Workspace' && workspaceRef) {
        obj.__workspace = workspaceRef;
      }

      // Add namespace reference
      if (obj.__kind !== '__Workspace' && obj.__kind !== '__Namespace' && namespaceId) {
        obj.__namespace = namespaceId; // Store plain ID
      } else if (obj.__kind === '__Namespace' && workspaceRef) {
        obj.__workspace = workspaceRef;
      }
    }

    // Extract __ParsingError objects and convert to WorkspaceError
    const parsingErrorObjs = objects.filter((obj) => obj.__kind === '__ParsingError');
    const regularObjects = objects.filter((obj) => obj.__kind !== '__ParsingError');

    for (const errObj of parsingErrorObjs) {
      const errType = (errObj.type as string) || 'structured_in_textblock';
      // Build message from all non-system fields
      const detailParts: string[] = [];
      for (const [k, v] of Object.entries(errObj)) {
        if (k.startsWith('__') || k === 'type' || k === 'line') continue;
        const vStr = typeof v === 'string' ? v : JSON.stringify(v);
        detailParts.push(`${k}: ${vStr}`);
      }
      const message = detailParts.length > 0 ? `${errType}: ${detailParts.join(', ')}` : errType;

      nestedWorkspaceErrors.push({
        type: errType as WorkspaceError['type'],
        message,
        file: filePath,
        line: errObj.line as number | undefined,
        objectId: errObj.object as string | undefined,
        fieldName: errObj.field as string | undefined,
        reference: errObj.reference as string | undefined,
        severity: 'error',
      });
    }

    // Filter out __Workspace objects from non-readme files
    const fileName = filePath.split('/').pop() || filePath;
    const isReadme = fileName === 'readme.qmd.md';
    if (!isReadme) {
      const filteredObjects = regularObjects.filter((obj) => obj.__kind !== '__Workspace');
      allObjects.push(...filteredObjects);
    } else {
      allObjects.push(...regularObjects);
    }
  }

  // If no explicit workspace found but we have QMD.md files, create virtual workspace
  // BUT: Don't create virtual workspace if there's a workspace_in_wrong_file error
  const hasWrongFileError = nestedWorkspaceErrors.some((e) => e.type === 'workspace_in_wrong_file');

  if (!workspaceId && files.length > 0 && !hasWrongFileError) {
    // Use folder name as workspace ID
    const pathParts = rootPath.split(/[/\\]/);
    const virtualWsId = pathParts[pathParts.length - 1] || 'workspace';

    workspaceId = virtualWsId;
    workspaceRef = virtualWsId;

    // Create __Workspace object for virtual workspace
    const wsObj: QmdcObject = {
      __id: virtualWsId,
      __kind: '__Workspace',
      __file: '',
      __line: 1,
      name: virtualWsId,
    };

    // Add __Workspace object to allObjects (at the beginning)
    allObjects.unshift(wsObj);

    // Update all existing objects to have __workspace field
    for (const obj of allObjects) {
      const kind = obj.__kind || '';
      // Add __workspace to all objects except __Workspace itself
      if (kind !== '__Workspace') {
        obj.__workspace = virtualWsId;
      }
    }
  }

  // Build index
  const index = buildIndex(allObjects);

  // Validate
  const validationErrors = validateWorkspace(allObjects, index, rootPath);
  const errors = [...nestedWorkspaceErrors, ...validationErrors];

  return {
    root: canonicalSlash(rootPath),
    workspaceId,
    workspaces: singleEntry(workspaceId, rootPath),
    files,
    objects: allObjects,
    index,
    errors,
  };
}

/**
 * Build workspace index for fast lookups.
 */
export function buildIndex(objects: QmdcObject[]): WorkspaceIndex {
  const byId: Record<string, QmdcObject[]> = {};
  const byGlobalId: Record<string, QmdcObject> = {};
  const byKind: Record<string, QmdcObject[]> = {};
  const byFile: Record<string, QmdcObject[]> = {};
  const byNamespace: Record<string, QmdcObject[]> = {};
  const byLocalId: Record<string, QmdcObject[]> = {};

  for (const obj of objects) {
    const objId = obj.__id;
    const objKind = (obj.__kind as string) || '';
    const objFile = (obj.__file as string) || '';
    const objNamespace = obj.__namespace as string | undefined;

    // Skip internal system objects, but index user-facing system kinds
    // __Workspace, __Namespace, __Document, __Object are user-facing and should be indexable
    const userFacingSystemKinds = ['__Workspace', '__Namespace', '__Document', '__Object'];
    if (objKind.startsWith('__') && !userFacingSystemKinds.includes(objKind)) {
      continue;
    }

    if (objId) {
      if (!byId[objId]) byId[objId] = [];
      byId[objId].push(obj);

      // Global ID: namespace:Kind:id
      let nsId = '';
      if (objNamespace) {
        const match = objNamespace.match(/\[\[#([^\]]+)\]\]/);
        if (match) {
          nsId = match[1] || '';
        }
      }

      const globalId = nsId ? `${nsId}:${objKind}:${objId}` : `:${objKind}:${objId}`;
      byGlobalId[globalId] = obj;
    }

    if (objKind) {
      if (!byKind[objKind]) byKind[objKind] = [];
      byKind[objKind].push(obj);
    }

    if (objFile) {
      if (!byFile[objFile]) byFile[objFile] = [];
      byFile[objFile].push(obj);
    }

    if (objNamespace) {
      if (!byNamespace[objNamespace]) byNamespace[objNamespace] = [];
      byNamespace[objNamespace].push(obj);
    }

    const localId = obj.__local_id as string | undefined;
    if (localId) {
      if (!byLocalId[localId]) byLocalId[localId] = [];
      byLocalId[localId].push(obj);
    }
  }

  return { byId, byGlobalId, byKind, byFile, byNamespace, byLocalId };
}

/**
 * Remove content inside backticks (inline code) to avoid extracting escaped refs.
 * `[[#id]]` should not be treated as a reference.
 */
// extractReferences and stripBackticks are no longer used - we use __references from objects instead

/**
 * Whether a candidate in `candWorkspace` satisfies a reference's WORKSPACE qualifier (QMD-69).
 *
 * A name binds the match to that workspace; `null` keeps it inside `objWorkspace`, because a
 * cross-workspace reference must name its workspace. An empty `objWorkspace`
 * (single-workspace parse) imposes no constraint, and so does an empty `candWorkspace` — that
 * is a workspace ROOT object.
 *
 * Mirrors `workspace_matches` in the Rust `core::reference_scan`.
 */
function workspaceMatches(
  refWorkspace: string | null,
  candWorkspace: string,
  objWorkspace: string
): boolean {
  if (refWorkspace !== null) {
    return candWorkspace === refWorkspace;
  }
  return !objWorkspace || !candWorkspace || candWorkspace === objWorkspace;
}

/**
 * Whether a candidate satisfies a reference's qualifiers (QMD-69).
 *
 * Used by every path that resolves a reference by its `__id`: the main candidate filter, the
 * field-reference escape and the `ambiguous_field_reference` check.
 *
 * `refNamespace` is a name for an assertion, `''` for the ELIDED form (`ws::id`, no namespace
 * constraint), and `null` when the reference named none — which imposes no constraint here
 * either, the own-namespace-first preference being the caller's rule.
 *
 * The `__local_id` fallback deliberately does NOT use this: there an unqualified reference is
 * scoped to the referring object's own namespace exactly.
 */
function qualifiersMatch(
  refWorkspace: string | null,
  refNamespace: string | null,
  candWorkspace: string,
  candNamespace: string,
  objWorkspace: string
): boolean {
  if (!workspaceMatches(refWorkspace, candWorkspace, objWorkspace)) {
    return false;
  }
  if (refNamespace === null || refNamespace === '') {
    return true;
  }
  return candNamespace === refNamespace;
}

/**
 * Parse a reference target into [workspace, namespace, id].
 *
 * QMD-69: a reference target is a right-aligned suffix of the `__global_id` grammar
 * `workspace:namespace:id`, with an optional `.field` suffix carried inside the id.
 * There is no `Kind` segment.
 *
 * - `#id` -> [null, null, id]
 * - `#ns:id` -> [null, ns, id]
 * - `#ws:ns:id` -> [ws, ns, id]
 * - `#ws::id` -> [ws, '', id] -- an EMPTY namespace ELIDES rather than asserting the
 *   workspace root, so the target may live in any namespace of `ws`; more than one
 *   candidate is an ambiguity.
 */
function parseReference(ref: string): [string | null, string | null, string] {
  const match = ref.match(/\[\[#([^\]]+)\]\]/);
  if (!match) {
    return [null, null, ref];
  }

  const inner = match[1] || '';
  const parts = inner.split(':');

  if (parts.length >= 3) {
    // workspace:namespace:id -- the id keeps any further colons; the middle segment may
    // be empty (elided namespace), so it must NOT be coerced to null.
    return [parts[0] || null, parts[1] ?? '', parts.slice(2).join(':')];
  } else if (parts.length === 2) {
    return [null, parts[0] ?? '', parts[1] || ''];
  } else {
    return [null, null, parts[0] || ''];
  }
}

/**
 * Resolve a reference to target object(s).
 */
export function resolveReference(
  ref: string,
  _fromObj: QmdcObject, // For context (future use: relative resolution)
  index: WorkspaceIndex
): QmdcObject | QmdcObject[] | null {
  const [ns, kind, objId] = parseReference(ref);

  // If fully qualified, use global_id lookup
  if (ns && kind) {
    const globalId = `${ns}:${kind}:${objId}`;
    return index.byGlobalId[globalId] || null;
  }

  // Get all objects with this id
  let candidates = index.byId[objId] || [];

  if (candidates.length === 0) {
    return null; // Broken link
  }

  // Filter by kind if specified
  if (kind) {
    candidates = candidates.filter((c) => c.__kind === kind);
  }

  // Filter by namespace if specified
  if (ns) {
    candidates = candidates.filter((c) => c.__namespace === ns); // Plain ID comparison
  }

  if (candidates.length === 1) {
    return candidates[0] || null;
  } else if (candidates.length > 1) {
    return candidates; // Ambiguous
  } else {
    return null; // Broken link
  }
}

/**
 * Validate workspace for errors.
 */
export function validateWorkspace(
  objects: QmdcObject[],
  _index: WorkspaceIndex,
  rootPath?: string,
  filePaths?: Map<string, string>
): WorkspaceError[] {
  const errors: WorkspaceError[] = [];

  // Phase 3: Resolve dot-ID parents
  // Objects with __local_id == __id and "." in __id are dot-ID declarations
  // that need parent resolution from the global object graph
  for (const obj of objects) {
    const objId = obj.__id || '';
    const localId = obj.__local_id as string | undefined;
    // Dot-ID detection: __local_id equals __id AND contains a dot
    // (same-file children have __local_id != __id)
    if (!localId || localId !== objId || !objId.includes('.')) {
      continue;
    }
    // Already has a parent (shouldn't happen, but guard)
    if (obj.__parent) {
      continue;
    }
    // Split on last dot to get parent path
    const lastDot = objId.lastIndexOf('.');
    const parentPath = objId.slice(0, lastDot);
    // Look up parent in the object list
    const parentFound = objects.some((o) => o.__id === parentPath);
    if (parentFound) {
      obj.__parent = `[[#${parentPath}]]`;
    } else {
      errors.push({
        type: 'broken_parent',
        message: `Parent object '${parentPath}' not found in workspace`,
        file: obj.__file as string,
        line: obj.__line as number,
        objectId: objId,
        severity: 'error',
      });
    }
  }

  // Build index of all objects by id, kind, and namespace for validation
  // Format: id -> [(file, kind, namespace, line), ...]
  // QMD-69: [file, kind, namespace, workspace, line] -- the workspace is carried so a
  // workspace-qualified reference can be enforced and an unqualified one kept local.
  const objectsById: Record<string, Array<[string, string, string, string, number]>> = {};

  for (const obj of objects) {
    const objId = obj.__id;
    const objFile = obj.__file as string;
    const objLine = obj.__line as number;

    if (!objId || !objFile || objLine === undefined) {
      continue;
    }

    const objKind = (obj.__kind as string) || '__Object';
    const objNamespace = (obj.__namespace as string) || '';
    const nsId = extractNamespaceId(objNamespace);

    if (!objectsById[objId]) {
      objectsById[objId] = [];
    }
    objectsById[objId].push([objFile, objKind, nsId, (obj.__workspace as string) || '', objLine]);
  }

  // Check for duplicate IDs (same id, different files or same file)
  // Skip system objects (__Document, __TextBlock) as they are auto-generated per file
  //
  // QMD-67: duplicate identity is scoped by namespace. Two objects with the same full
  // __id in DIFFERENT namespaces are distinct (distinct rows in the DB, keyed by
  // (__workspace, __namespace, __id), and distinct reference targets). We group by
  // (namespace, id) here. This is a SEPARATE grouping from objectsById (bare-id
  // keyed), which must stay bare-id-keyed for reference resolution / __local_id
  // fallback / cross-namespace hints. Dropping __workspace is safe: validation runs
  // per-workspace. A nested Map gives a collision-safe composite key (no delimiter).
  // Exclude system objects (__Document/__TextBlock/__ParsingError) PER LOCATION while
  // building the duplicate index, rather than skipping a whole (namespace, id) group if
  // any member is a system object. A group-level skip would mask a genuine user-object
  // duplicate whenever a system object happens to share the id in the same namespace.
  // (objectsById itself stays complete — reference resolution depends on it.)
  const SYSTEM_KINDS = new Set(['__Document', '__TextBlock', '__ParsingError']);
  const objectsByNsId = new Map<
    string,
    Map<string, Array<[string, string, string, string, number]>>
  >();
  for (const [objId, locations] of Object.entries(objectsById)) {
    for (const loc of locations) {
      if (SYSTEM_KINDS.has(loc[1])) {
        continue;
      }
      const namespace = loc[2];
      let byId = objectsByNsId.get(namespace);
      if (!byId) {
        byId = new Map();
        objectsByNsId.set(namespace, byId);
      }
      const bucket = byId.get(objId);
      if (bucket) {
        bucket.push(loc);
      } else {
        byId.set(objId, [loc]);
      }
    }
  }

  for (const byId of objectsByNsId.values()) {
    for (const [objId, locations] of byId) {
      if (locations.length > 1) {
        // Check if duplicates are in different files
        const files = new Set(locations.map(([file]) => file));
        if (files.size > 1) {
          // Duplicate ID across files
          for (const location of locations.slice(1)) {
            const [file, , , , line] = location;
            const candidates = locations.map(([f, , , , l]) => `${f}:${l}`);
            errors.push({
              type: 'duplicate_id',
              message: `Duplicate ID '${objId}' found in multiple files`,
              file,
              line,
              objectId: objId,
              candidates,
              severity: 'error',
            });
          }
        } else {
          // Same file - check if different kinds
          const kinds = new Set(locations.map(([, kind]) => kind));
          if (kinds.size > 1) {
            // Same ID, different kinds - ambiguous
            const firstKind = locations[0]?.[1];
            if (!firstKind) continue;
            for (const [file, kind, , , line] of locations.slice(1)) {
              const candidates = locations.map(([f, k, , , l]) => `${f}:${k}:${l}`);
              errors.push({
                type: 'duplicate_id',
                message: `Duplicate ID '${objId}' with different kinds: ${firstKind} and ${kind}`,
                file,
                line,
                objectId: objId,
                candidates,
                severity: 'error',
              });
            }
          }
        }
      }
    }
  }

  // Check for broken links and ambiguous references using __references from objects
  for (const obj of objects) {
    // Get namespace of current object
    const objNamespace = (obj.__namespace as string) || '';
    let objNsId = extractNamespaceId(objNamespace);
    // A __Namespace root object has no own __namespace, but it defines a
    // namespace and resolves its references within it (its own __id).
    // Mirror the Rust resolver, which derives the effective namespace from
    // the file directory for such objects.
    if (!objNsId && obj.__kind === '__Namespace') {
      objNsId = (obj.__id as string) || '';
    }
    // QMD-69: the referring object's own workspace. Empty in a single-workspace parse,
    // where it correctly imposes no filter.
    const objWsId = (obj.__workspace as string) || '';
    {
    }

    // Get all references from this object using __references field
    const refs = (obj.__references as Array<{ target: string; line: number; raw?: string }>) || [];

    for (const refInfo of refs) {
      // Use 'raw' field if available (contains full [[#...]]), otherwise use 'target'
      let target = refInfo.raw || refInfo.target;
      const line = refInfo.line;

      if (!target || line === undefined || line === null) {
        continue;
      }

      // If target doesn't have [[#...]], add it
      if (!target.startsWith('[[')) {
        target = target.startsWith('#') ? `[[${target}]]` : `[[#${target}]]`;
      }

      const objId = obj.__id;
      const objFile = (obj.__file as string) || '';

      // Parse reference target to extract workspace, namespace and id
      const [refWs, refNs, refId] = parseReference(target);

      // QMD-69 candidate filter. Each qualifier present narrows the search; the WORKSPACE
      // never widens past the referring object's own workspace unless the reference names
      // one.
      const matchingObjects: Array<[string, string, string, string, number]> = [];
      if (objectsById[refId]) {
        for (const [file, kind, ns, ws, refLine] of objectsById[refId]) {
          if (refWs !== null) {
            // `ws:...` -- only inside the named workspace.
            if (ws !== refWs) {
              continue;
            }
          } else if (objWsId && ws && ws !== objWsId) {
            // Unqualified: a cross-workspace reference MUST name its workspace, so an
            // unqualified one stays local.
            continue;
          }
          if (refNs !== null && refNs !== '' && ns !== refNs) {
            // An EMPTY namespace segment (`ws::id`) elides rather than asserting the root
            // namespace: any namespace matches.
            continue;
          }
          matchingObjects.push([file, kind, ns, ws, refLine]);
        }
      }

      // If reference doesn't specify namespace, prefer objects in same namespace as current object
      // According to spec: "current namespace first, then other files in the same namespace"
      // Ambiguous only if:
      // 1. Multiple objects in current namespace, OR
      // 2. No objects in current namespace but multiple in other namespaces
      let resolvedObjects: Array<[string, string, string, string, number]>;
      if (refNs === null) {
        if (objNsId) {
          // Prefer objects from same namespace
          const sameNs = matchingObjects.filter(([, , ns]) => ns === objNsId);
          if (sameNs.length > 0) {
            resolvedObjects = sameNs;
          } else {
            // No objects in current namespace - all matching objects are candidates
            resolvedObjects = matchingObjects;
          }
        } else {
          // Object is in root namespace - all matching objects are candidates
          resolvedObjects = matchingObjects;
        }
      } else {
        // Reference specifies namespace - use all matching objects
        resolvedObjects = matchingObjects;
      }

      // Check if reference is inside backticks (inline code) - skip validation.
      // Through `filePaths` when given: a composed `-w` set's `__file` is relative to a
      // virtual base and cannot be joined onto any directory (QMD-72).
      if (objFile && (filePaths !== undefined || rootPath)) {
        try {
          const filePath =
            filePaths !== undefined ? filePaths.get(objFile) : join(rootPath as string, objFile);
          if (filePath !== undefined && existsSync(filePath)) {
            const fileContent = readFileSync(filePath, 'utf-8');
            const fileLines = fileContent.split('\n');
            if (line > 0 && line <= fileLines.length) {
              const origLine = fileLines[line - 1];
              if (origLine !== undefined) {
                // Find position of reference in line
                const rawRef = refInfo.raw || target;
                const refPos = origLine.indexOf(rawRef);
                if (refPos >= 0) {
                  // Check if reference is inside backticks (single or double)
                  if (isInsideBackticks(origLine, refPos)) {
                    continue;
                  }
                  // Also check if reference is between double backticks (``...``)
                  // Find all pairs of double backticks and check if ref is inside any pair
                  const doubleBacktickRegex = /``/g;
                  const matches: number[] = [];
                  let match;
                  while ((match = doubleBacktickRegex.exec(origLine)) !== null) {
                    matches.push(match.index);
                  }
                  let skipValidation = false;
                  for (let i = 0; i < matches.length; i += 2) {
                    if (i + 1 < matches.length) {
                      const startPos = matches[i];
                      const endPos = matches[i + 1];
                      if (
                        startPos !== undefined &&
                        endPos !== undefined &&
                        startPos < refPos &&
                        refPos < endPos
                      ) {
                        // Reference is inside double backticks - skip validation
                        skipValidation = true;
                        break;
                      }
                    }
                  }
                  if (skipValidation) {
                    continue;
                  }
                }
              }
            }
          }
        } catch {
          // If we can't read the file, continue with validation
        }
      }

      if (resolvedObjects.length === 0) {
        // __local_id fallback: try to resolve by __local_id within same namespace
        const localCandidatesRaw = _index.byLocalId[refId] || [];

        // QMD-69: honour the reference's WORKSPACE qualifier here too. This path used to
        // ignore it entirely, so `[[#no_such_ws:ns:leaf]]` was accepted.
        const wsScoped = localCandidatesRaw.filter((c) =>
          workspaceMatches(refWs, (c.__workspace as string) || '', objWsId)
        );

        // The NAMESPACE rule is deliberately NOT the elide/assert rule used by
        // qualifiersMatch: an unqualified reference resolves by __local_id only inside the
        // referring object's OWN namespace (the root namespace when it has none). That is what
        // keeps a bare [[#config]] at the workspace root from reaching gateway.config in the
        // `services` namespace.
        let localCandidates: QmdcObject[];
        if (refNs === '') {
          // `ws::id` — namespace elided: any namespace of the named workspace.
          localCandidates = wsScoped;
        } else {
          const targetNs = refNs !== null ? refNs : objNsId;
          if (targetNs) {
            localCandidates = wsScoped.filter(
              (c) => extractNamespaceId((c.__namespace as string) || '') === targetNs
            );
          } else {
            // Root-level: only match other root-level objects
            localCandidates = wsScoped.filter((c) => !c.__namespace);
          }
        }

        if (localCandidates.length === 1) {
          // Resolved via __local_id — no error
          const matched = localCandidates[0]!;
          resolvedObjects = [
            [
              matched.__file as string,
              (matched.__kind as string) || '',
              extractNamespaceId((matched.__namespace as string) || ''),
              (matched.__workspace as string) || '',
              matched.__line as number,
            ],
          ];
        } else if (localCandidates.length > 1) {
          // Ambiguous by __local_id
          const candidates = localCandidates.map((c) => {
            const cNs = extractNamespaceId((c.__namespace as string) || '');
            const cKind = (c.__kind as string) || '';
            if (cNs) {
              return `${cNs}:${cKind}:${c.__id || ''}`;
            } else {
              return `${cKind}:${c.__id || ''}`;
            }
          });
          errors.push({
            type: 'ambiguous_reference',
            message: `Ambiguous reference '${target}' - multiple objects match by __local_id`,
            file: objFile,
            line,
            objectId: objId,
            reference: target,
            candidates,
            severity: 'error',
          });
          continue; // Skip further processing for this ref
        }
        // else: localCandidates is empty, fall through to existing broken_link / field-ref logic
      }

      if (resolvedObjects.length === 0) {
        // Try field-level resolution: if refId contains a dot,
        // split on last dot and check if prefix is a valid object
        // AND the field actually exists on that object
        let isFieldRef = false;
        if (refId.includes('.')) {
          const lastDot = refId.lastIndexOf('.');
          const objPrefix = refId.slice(0, lastDot);
          const fieldPart = refId.slice(lastDot + 1);
          // QMD-69: the prefix object must itself satisfy the reference's qualifiers, and
          // the scan must not stop at the first object with a matching id — in a composed
          // container the same id legitimately exists in several workspaces. This escape
          // used to accept any object with a matching id and field, so a reference naming a
          // workspace that does not exist, or one that exists but does not hold the object,
          // was silently treated as a field reference and never reported.
          for (const candidateObj of objects) {
            if (candidateObj.__id !== objPrefix) {
              continue;
            }
            if (
              !qualifiersMatch(
                refWs,
                refNs,
                (candidateObj.__workspace as string) || '',
                extractNamespaceId((candidateObj.__namespace as string) || ''),
                objWsId
              )
            ) {
              continue;
            }
            if (fieldPart in candidateObj && !fieldPart.startsWith('__')) {
              isFieldRef = true;
              break;
            }
          }
        }

        if (!isFieldRef) {
          // Check if the object exists in a different namespace
          // (cross-namespace hint for better error messages)
          let hint = '';
          const otherNsLocal = _index.byLocalId[refId] || [];
          if (otherNsLocal.length > 0) {
            const others = otherNsLocal.filter((c) => {
              const cNs = extractNamespaceId((c.__namespace as string) || '');
              return objNsId ? cNs !== objNsId : cNs !== '';
            });
            if (others.length > 0) {
              const otherNs = extractNamespaceId((others[0]!.__namespace as string) || '');
              const otherId = (others[0]!.__id as string) || refId;
              hint = `. Did you mean [[#${otherNs}:${otherId}]]?`;
            }
          }
          if (!hint) {
            // Check by __id in other namespaces
            const otherNsId = _index.byId[refId] || [];
            if (otherNsId.length > 0) {
              const others = otherNsId.filter((c) => {
                const cNs = extractNamespaceId((c.__namespace as string) || '');
                return objNsId ? cNs !== objNsId : cNs !== '';
              });
              if (others.length > 0) {
                const otherNs = extractNamespaceId((others[0]!.__namespace as string) || '');
                const otherId = (others[0]!.__id as string) || refId;
                hint = `. Did you mean [[#${otherNs}:${otherId}]]?`;
              }
            }
          }

          // Broken link - reference not found
          errors.push({
            type: 'broken_link',
            message: `Object '${refId}' not found${hint}`,
            file: objFile,
            line,
            objectId: objId,
            reference: target,
            severity: 'error',
          });
        }
      } else if (resolvedObjects.length === 1) {
        // Object found — check for ambiguous_field_reference
        // If refId contains a dot, check if the field-path interpretation
        // also resolves to a scalar field (not a reference to this object)
        if (refId.includes('.')) {
          const lastDot = refId.lastIndexOf('.');
          const objPrefix = refId.slice(0, lastDot);
          const fieldPart = refId.slice(lastDot + 1);
          // QMD-69: same qualifier rule as the field-ref escape above — the prefix object
          // considered here must be one the reference could actually name, or QMDC009 would
          // be raised about an object in a workspace the reference never mentioned.
          for (const candidateObj of objects) {
            if (candidateObj.__id !== objPrefix) {
              continue;
            }
            if (
              !qualifiersMatch(
                refWs,
                refNs,
                (candidateObj.__workspace as string) || '',
                extractNamespaceId((candidateObj.__namespace as string) || ''),
                objWsId
              )
            ) {
              continue;
            }
            if (fieldPart in candidateObj && !fieldPart.startsWith('__')) {
              const fieldVal = candidateObj[fieldPart];
              // Ambiguous if field value is NOT a reference to the object
              if (fieldVal !== `[[#${refId}]]`) {
                const fieldValRepr = JSON.stringify(fieldVal).slice(0, 40);
                errors.push({
                  type: 'ambiguous_field_reference',
                  message: `Reference '${target}' cannot be unequivocally resolved to an object or a field`,
                  file: objFile,
                  line,
                  objectId: objId,
                  reference: target,
                  candidates: [
                    `object with __id '${refId}'`,
                    `field '${fieldPart}' on object '${objPrefix}' (value: ${fieldValRepr})`,
                  ],
                  severity: 'error',
                });
              }
            }
            // Only the first qualifying candidate is considered.
            break;
          }
        }
      } else if (resolvedObjects.length > 1) {
        // Ambiguous reference - multiple matching objects
        const kinds = new Set(resolvedObjects.map(([, kind]) => kind));
        const namespaces = new Set(resolvedObjects.map(([, , ns]) => ns));

        // QMD-69: there is no Kind segment to suppress ambiguity with, and an elided
        // namespace (`ws::id`) explicitly MAY match several namespaces -- which is an
        // ambiguity, not a silent pick.
        const isAmbiguous = kinds.size > 1 || namespaces.size > 1;

        if (isAmbiguous) {
          const candidates = resolvedObjects.map(([, kind, ns]) => {
            if (ns && ns.length > 0) {
              return `${ns}:${kind}:${refId}`;
            } else {
              return `${kind}:${refId}`;
            }
          });
          errors.push({
            type: 'ambiguous_reference',
            message: `Ambiguous reference '${target}' - multiple objects match`,
            file: objFile,
            line,
            objectId: objId,
            reference: target,
            candidates,
            severity: 'error',
          });
        }
      }
    }
  }

  return errors;
}

/**
 * Convert WorkspaceResult to JSON-serializable object.
 */
export function workspaceToJson(result: WorkspaceResult): Record<string, unknown> {
  // Output shape (QMD-72): one shape for every invocation. `workspaces` is always present --
  // zero, one or many {id, root, path} entries -- replacing the QMD-59 `workspace: id` /
  // `workspaces: [ids]` / `workspace: null` trio a consumer had to tell apart. `root` is the
  // base every __file is relative to, or null when that base is virtual (-w).
  const out: Record<string, unknown> = { root: result.root, workspaces: result.workspaces };

  out.files = result.files;
  out.objects = result.objects;
  // QMD-75: the EMITTED keys are snake_case, like every other key in the output
  // (`by_global_id`, not `byGlobalId`). TypeScript published the internal camelCase names
  // here, so the `index` block itself disagreed with Python even where both emitted one. The
  // in-memory index keeps its camelCase names; only the wire shape is renamed.
  out.index = {
    by_global_id: Object.fromEntries(
      Object.entries(result.index.byGlobalId).map(([k, v]) => [k, v.__id]) // Plain IDs
    ),
    by_kind: Object.fromEntries(
      Object.entries(result.index.byKind).map(([k, v]) => [k, v.map((o) => o.__id)]) // Plain IDs
    ),
    by_file: Object.fromEntries(
      Object.entries(result.index.byFile).map(([k, v]) => [k, v.map((o) => o.__id)]) // Plain IDs
    ),
  };
  out.errors = result.errors.map((e) => ({
    type: e.type,
    message: e.message,
    file: e.file,
    line: e.line,
    // QMD-77 C3: the same keys `workspace validate` uses. This envelope said `object`/`field`
    // for the very same error, so a consumer reading both commands had to know two spellings.
    objectId: e.objectId,
    fieldName: e.fieldName,
    reference: e.reference,
    candidates: e.candidates,
    severity: e.severity,
  }));

  return out;
}

/**
 * Find all workspace directories (directories containing readme.qmd.md with __Workspace).
 *
 * Unbounded depth. Use `findWorkspaceDirsBounded` where a caller-supplied path must not
 * turn into a full-tree crawl.
 */
export function findAllWorkspaceDirs(rootPath: string): string[] {
  return scanWorkspaceDirs(rootPath, Number.POSITIVE_INFINITY);
}

/**
 * Depth-bounded variant of `findAllWorkspaceDirs` (QMD-72).
 *
 * Mirrors `find_nested_workspace_roots_bounded` in the Rust parser and
 * `find_workspace_dirs_bounded` in Python: the crawl is capped at `maxDepth` directory
 * levels below `rootPath`. The cap is part of the contract, not an optimisation -- a marker
 * deeper than this is NOT discovered, and all three implementations must agree on that, or
 * the same `-w` invocation is a usage error in one and a successful composition in another.
 */
export function findWorkspaceDirsBounded(rootPath: string, maxDepth: number): string[] {
  return scanWorkspaceDirs(rootPath, maxDepth);
}

function scanWorkspaceDirs(rootPath: string, maxDepth: number): string[] {
  const root = resolve(rootPath);
  // The root's .qmdcignore steers discovery, as it does in the Rust scan (QMD-73): without
  // it `-w` on a directory that ignores one of its two workspaces composed in Rust and was
  // refused here as "contains 2 workspaces".
  const ignorePatterns = loadQmdcignore(root);
  const workspaceDirs: string[] = [];

  function scanDir(dir: string, depth: number): void {
    // The cap applies to the README's own depth, not the directory's: a readme inside a
    // directory `depth` levels below root sits at `depth + 1`. Checking the directory
    // instead finds markers one level deeper than rs and py do — measured, that off-by-one
    // made depth 6 discoverable here and invisible there.
    const readmeDepth = depth + 1;
    if (readmeDepth <= maxDepth) {
      const readmePath = join(dir, 'readme.qmd.md');
      if (existsSync(readmePath) && !isIgnored(readmePath, root, ignorePatterns)) {
        let content: string;
        try {
          content = readFileSync(readmePath, 'utf-8');
        } catch {
          // An unreadable marker is a marker we cannot see; treat it as absent rather than
          // aborting the whole scan with a raw OS error, which the Rust and Python scans
          // also do (their read failure falls through the same way).
          content = '';
        }
        if (contentHasWorkspaceMarker(content)) {
          workspaceDirs.push(dir);
        }
      }
    }

    // No readme below this point could pass the cap, so stop descending.
    if (readmeDepth >= maxDepth) {
      return;
    }

    // Recursively scan subdirectories
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      // Unreadable or not a directory: nothing to discover below it. The caller turns an
      // empty result into the "not a workspace" usage error, which is what rs and py
      // report for the same input -- letting EACCES/ENOTDIR escape would exit 1 instead.
      return;
    }
    for (const entry of entries) {
      const child = join(dir, entry.name);
      if (entry.isDirectory() && !isIgnored(child, root, ignorePatterns, true)) {
        scanDir(child, depth + 1);
      }
    }
  }

  scanDir(root, 0);
  return workspaceDirs;
}

/**
 * Parse all workspaces found in a directory tree (non-nested).
 *
 * If root_path itself is a workspace, parse only that one.
 * If root_path contains multiple workspace directories, parse all of them.
 */
/**
 * Recursively find all .qmd.md files in a directory
 */
function findQmdcFiles(dir: string): string[] {
  const results: string[] = [];
  // QMD-77 D1: skip a directory we cannot read, as rs, py and `git` do.
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return results;
  }

  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...findQmdcFiles(fullPath));
    } else if (entry.isFile() && entry.name.endsWith('.qmd.md')) {
      results.push(fullPath);
    }
  }

  return results;
}

/**
 * Parse all workspaces found in a directory tree (non-nested).
 *
 * If root_path itself is a workspace, parse only that one.
 * If root_path contains multiple workspace directories, parse all of them.
 * Respects .qmdcignore patterns at the root level.
 */
export function parseAllWorkspaces(rootPath: string): WorkspaceResult {
  const root = resolve(rootPath);

  // Load .qmdcignore patterns
  const ignorePatterns = loadQmdcignore(root);

  // Check if root_path itself is a workspace
  const rootReadme = join(root, 'readme.qmd.md');
  if (existsSync(rootReadme) && !isIgnored(rootReadme, root, ignorePatterns)) {
    const content = readFileSync(rootReadme, 'utf-8');
    if (contentHasWorkspaceMarker(content)) {
      // Root is a workspace - use single workspace parsing
      return parseWorkspace(root);
    }
  }

  // Root is not a workspace - find all workspaces in subdirectories. findAllWorkspaceDirs
  // already drops a marker the root's .qmdcignore hides, using the same root and the same
  // rules, so there is nothing left for a second filter to remove (QMD-73).
  const workspaceDirs = findAllWorkspaceDirs(root);

  if (workspaceDirs.length === 0) {
    // No explicit workspaces found - check if root has .qmd.md files
    // If yes, treat root as a virtual workspace
    // IMPORTANT: Must respect .qmdcignore when checking for files
    let hasQmdcFiles = false;
    const qmdcFiles = findQmdcFiles(root);
    for (const qmdcFile of qmdcFiles) {
      // Check .qmdcignore before considering file
      if (!isIgnored(qmdcFile, root, ignorePatterns)) {
        // Check max depth (5 levels)
        const relPath = relative(root, qmdcFile);
        const depth = relPath.split(/[/\\]/).length;
        if (depth <= 5) {
          hasQmdcFiles = true;
          break;
        }
      }
    }

    if (hasQmdcFiles) {
      // Treat root as a virtual workspace
      return parseWorkspace(root);
    }

    // No workspaces and no QMD.md files - return empty result
    return {
      root: canonicalSlash(root),
      workspaceId: null,
      workspaces: [],
      files: [],
      objects: [],
      index: {
        byId: {},
        byGlobalId: {},
        byKind: {},
        byFile: {},
        byNamespace: {},
        byLocalId: {},
      },
      errors: [],
    };
  }

  // Parse each workspace and combine results -- through the one composition primitive.
  const {
    objects: allObjects,
    files: allFiles,
    filePaths: allFilePaths,
    errors: allErrors,
    workspaces,
  } = composeWorkspaceRoots(workspaceDirs, root);

  // After parsing explicit workspaces, check for orphan .qmd.md files
  // (files outside any workspace directory that should be loaded too)
  const allQmdcFiles = findQmdcFiles(root);
  const orphanFiles = allQmdcFiles.filter((file) => {
    // Exclude files inside explicit workspace directories
    const isInsideWorkspace = workspaceDirs.some(
      (wsDir) => file.startsWith(wsDir + '/') || file.startsWith(wsDir + '\\')
    );
    // Apply .qmdcignore filtering
    return !isInsideWorkspace && !isIgnored(file, root, ignorePatterns);
  });

  if (orphanFiles.length > 0) {
    // First pass: check for workspace_in_wrong_file errors in orphan files
    let hasWrongFileError = false;
    for (const filePath of orphanFiles) {
      try {
        const content = readFileSync(filePath, 'utf-8');
        const objects = parse(content, { randomSeed: 666 });
        const fileName = filePath.split(/[/\\]/).pop() || '';
        const isReadme = fileName === 'readme.qmd.md';

        for (const obj of objects) {
          if (!isReadme && obj.__kind === '__Workspace') {
            hasWrongFileError = true;
            const wsId = obj.__id;
            const relFile = relative(root, filePath);
            // Check .qmdcignore before adding error
            if (!isIgnored(filePath, root, ignorePatterns)) {
              allErrors.push({
                type: 'workspace_in_wrong_file',
                message: `Workspace '${wsId}' must be defined in readme.qmd.md, not in '${relFile}'.`,
                file: relFile,
                line: getLineNumber(content, obj),
                objectId: wsId,
                severity: 'error',
              });
            }
          }
        }
      } catch {
        // Skip files that can't be read
      }
    }

    // Only create virtual workspace if:
    // 1. There are no explicit workspaces (workspaceDirs.length === 0)
    // 2. There's no workspace_in_wrong_file error
    const virtualWsId = root.split(/[/\\]/).pop() || 'workspace';
    const shouldCreateVirtualWorkspace = workspaceDirs.length === 0 && !hasWrongFileError;

    if (shouldCreateVirtualWorkspace) {
      // Create __Workspace object for virtual workspace
      const wsObj: QmdcObject = {
        __id: virtualWsId,
        __kind: '__Workspace',
        __file: '' as string,
        __line: 1,
        name: virtualWsId,
      };
      allObjects.unshift(wsObj);
    }

    // Second pass: parse orphan files
    for (const filePath of orphanFiles) {
      try {
        const content = readFileSync(filePath, 'utf-8');
        const objects = parse(content, { randomSeed: 666 });

        const relFile = relative(root, filePath);
        const fileName = filePath.split(/[/\\]/).pop() || '';
        const isReadme = fileName === 'readme.qmd.md';

        // Add __file and __workspace metadata to each object
        for (const obj of objects) {
          // Skip __Workspace objects from non-readme files
          if (!isReadme && obj.__kind === '__Workspace') {
            continue; // Already handled in first pass
          }

          obj.__file = relFile;
          if (shouldCreateVirtualWorkspace) {
            obj.__workspace = virtualWsId; // Store plain ID
          }
          allObjects.push(obj);
        }

        allFiles.push(relFile);
        allFilePaths.set(relFile, filePath);
      } catch {
        // Skip files that can't be read
      }
    }
  }

  // QMD-69: re-run reference validation over the COMPOSED object set. See
  // `rescanComposedReferences` for why the per-workspace findings cannot be reused.
  const composedIndex = rescanComposedReferences(allObjects, allFilePaths, allErrors);

  return {
    root: canonicalSlash(root),
    workspaceId: null, // Multiple workspaces, no single ID
    workspaces,
    files: allFiles,
    objects: allObjects,
    index: composedIndex,
    errors: allErrors,
  };
}

/**
 * Whether a workspace error was produced by the reference resolver.
 *
 * QMD-69: these are recomputed over the composed object set when a container holds several
 * sibling workspaces, so the per-workspace copies must be discarded first.
 */
function isReferenceFinding(errorType: string): boolean {
  return (
    errorType === 'broken_link' ||
    errorType === 'ambiguous_reference' ||
    errorType === 'ambiguous_field_reference'
  );
}

/**
 * A workspace command was invoked in a way that cannot mean anything (QMD-72).
 *
 * Distinct from a parse or validation failure: nothing was wrong with the documents, the
 * invocation itself was refused, so the CLI reports it as a usage error (exit 2) rather than
 * as a result.
 */
export class WorkspaceUsageError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'WorkspaceUsageError';
  }
}

/** What `composeWorkspaceRoots` produces (QMD-72). */
export interface Composition {
  objects: QmdcObject[];
  files: string[];
  /**
   * Real location of each `files` entry: `files` is relative to the result's base, which is
   * virtual in the `-w` form and so cannot be joined.
   */
  filePaths: Map<string, string>;
  errors: WorkspaceError[];
  /** One entry per composed root, ordered by `path`. */
  workspaces: WorkspaceEntry[];
}

/**
 * Compose an explicit set of workspace ROOTS into one graph (QMD-72).
 *
 * This is the one composition primitive: the container form, the CLI's `-w` / `--with`
 * form and any other surface that composes workspaces reach composition through it, so none
 * of them can resolve references differently from the others. Each root must already be a
 * single workspace; discovery is the caller's job.
 *
 * `base` decides where each workspace's files appear in `__file`. A real directory that
 * contains every root (the container form) puts them relative to it. `null` (the `-w` form)
 * puts each workspace at its own id in a virtual base: its roots need not share any
 * directory, so a real common ancestor can degenerate to `/` and would put the host's own
 * directory names into `__file`. The id depends only on the content, so the same
 * repositories give the same `__file` wherever they are checked out, and ids are distinct
 * within a composable set, so the values stay unique. Mirrors `Mount` in the Rust parser.
 *
 * Reference findings are dropped per workspace and recomputed once over the composed set by
 * `rescanComposedReferences`, because a workspace validated in isolation cannot see its
 * siblings' objects.
 */
export function composeWorkspaceRoots(roots: string[], base: string | null): Composition {
  const out: Composition = {
    objects: [],
    files: [],
    filePaths: new Map(),
    errors: [],
    workspaces: [],
  };

  // A workspace inside another one is reported as `nested_workspace` because its files are
  // then missing from the outer workspace's graph. When the inner one is itself a member of
  // this set, nothing is missing: the outer scan already leaves its files out, and they are
  // composed under the inner workspace. The report would contradict the composition the caller
  // asked for (`-w repo -w repo/.qmdc`), so it is dropped -- matched by path, not by id, since
  // the container form does not refuse two members sharing an id (QMD-72).
  const canon = (p: string): string => {
    try {
      return realpathSync(p);
    } catch {
      return resolve(p);
    }
  };
  const members = new Set(roots.map(canon));

  for (const wsDir of roots) {
    const wsResult = parseWorkspace(wsDir);

    // Where this workspace sits in the base. null only when a container member is somehow
    // not under the container, which discovery never produces; its values are then left as
    // they came, the historical behaviour.
    let prefix: string | null;
    if (base === null) {
      prefix = wsResult.workspaceId;
    } else {
      const rel = relative(base, wsDir);
      prefix = rel.startsWith('..') ? null : rel.split(sep).join('/');
    }
    const relocate = (rel: string): string | null =>
      prefix === null ? null : prefix === '' ? rel : `${prefix}/${rel}`;

    if (wsResult.workspaceId && prefix !== null) {
      out.workspaces.push({
        id: wsResult.workspaceId,
        root: canonicalSlash(wsDir),
        path: prefix,
      });
    }

    for (const obj of wsResult.objects) {
      if (obj.__file && typeof obj.__file === 'string') {
        const moved = relocate(obj.__file);
        if (moved !== null) {
          obj.__file = moved;
        }
      }
    }
    out.objects.push(...wsResult.objects);

    for (const file of wsResult.files) {
      const moved = relocate(file);
      if (moved !== null) {
        out.files.push(moved);
        out.filePaths.set(moved, join(wsDir, file));
      }
    }

    for (const error of wsResult.errors) {
      // QMD-69: reference findings are dropped here and recomputed once over the composed
      // object set -- in isolation this workspace could not see its siblings' objects, so
      // any cross-workspace reference looked broken.
      if (isReferenceFinding(error.type)) {
        continue;
      }
      if (
        error.type === 'nested_workspace' &&
        error.file &&
        members.has(canon(dirname(join(wsDir, error.file))))
      ) {
        continue;
      }
      if (error.file) {
        const moved = relocate(error.file);
        if (moved !== null) {
          error.file = moved;
        }
      }
      out.errors.push(error);
    }
  }

  out.workspaces.sort((a, b) =>
    a.path < b.path ? -1 : a.path > b.path ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0
  );
  return out;
}

/**
 * Re-run reference validation over a COMPOSED object set (QMD-69), returning the composed
 * index so the caller can publish it.
 *
 * Each workspace was parsed and validated in isolation, so its reference findings were
 * computed against an index that could not see the other workspaces' objects: a
 * workspace-qualified cross-workspace reference was therefore always reported broken. The
 * stale findings are dropped by `composeWorkspaceRoots` and the shared validator runs once
 * here over every object in the composed set.
 *
 * `filePaths` maps each `__file` to its real location; the validator reads a referring
 * object's source line through it, because under the `-w` form that value cannot be joined
 * onto any directory (QMD-72).
 *
 * Structural findings (duplicate_id, workspace_in_wrong_file, parsing errors) stay
 * per-workspace, because identity is workspace-scoped (QMD-67).
 */
export function rescanComposedReferences(
  objects: QmdcObject[],
  filePaths: Map<string, string>,
  errors: WorkspaceError[]
): WorkspaceIndex {
  const composedIndex = buildIndex(objects);
  for (const error of validateWorkspace(objects, composedIndex, undefined, filePaths)) {
    if (isReferenceFinding(error.type)) {
      errors.push(error);
    }
  }
  return composedIndex;
}

/**
 * Resolve one `-w` / `--with` path to exactly one workspace root (QMD-72).
 *
 * A `--with` path names a workspace, not a container: zero and several are both usage
 * errors, because the caller asked to compose a specific workspace and the tool must not
 * guess which one was meant.
 */
function resolveSingleWorkspaceRoot(path: string): string {
  if (!existsSync(path)) {
    throw new WorkspaceUsageError(`--with path does not exist: ${path}`);
  }
  // realpath, not just resolve: `resolve` is lexical, so a symlink and its target produce
  // two different strings and the caller's duplicate-path check misses the alias. rs
  // canonicalizes and py's Path.resolve() follows symlinks, so this is the key that makes
  // all three refuse `-w ws -w link_to_ws` with the same message.
  let resolved: string;
  try {
    resolved = realpathSync(path);
  } catch {
    resolved = resolve(path);
  }
  const readme = join(resolved, 'readme.qmd.md');
  if (existsSync(readme)) {
    let content = '';
    try {
      content = readFileSync(readme, 'utf-8');
    } catch {
      // Unreadable marker: fall through to the scan, which ends in the "not a workspace"
      // usage error (exit 2) that rs and py report for this input.
      content = '';
    }
    if (WORKSPACE_MARKER_RE.test(content)) {
      return resolved;
    }
  }

  const top: string[] = [];
  for (const r of findWorkspaceDirsBounded(resolved, WORKSPACE_SCAN_MAX_DEPTH)) {
    if (!top.some((kept) => r === kept || r.startsWith(kept + sep))) {
      top.push(r);
    }
  }
  if (top.length === 1) {
    return top[0] as string;
  }
  if (top.length === 0) {
    throw new WorkspaceUsageError(
      `--with path is not a workspace: ${path} ` +
        '(no readme.qmd.md declaring [[id: __Workspace]])'
    );
  }
  throw new WorkspaceUsageError(
    `--with path contains ${top.length} workspaces: ${path} (pass each one as its own --with)`
  );
}

/**
 * Compose the workspaces named by repeated `-w` / `--with` (QMD-72).
 *
 * Every path is a peer -- the first is not primary -- and each must resolve to exactly one
 * workspace. Throws `WorkspaceUsageError` rather than returning a wrong answer for the
 * shapes that cannot mean anything: a path that is not a workspace, a path holding several,
 * the same path twice, and two paths carrying the same workspace id (which would make an id
 * ambiguous, so the set could not be composed into one graph).
 */
export function composeWithPaths(paths: string[]): WorkspaceResult {
  const roots: string[] = [];
  for (const path of paths) {
    const root = resolveSingleWorkspaceRoot(path);
    if (roots.includes(root)) {
      throw new WorkspaceUsageError(`--with path given twice: ${path}`);
    }
    roots.push(root);
  }

  const { objects, files, filePaths, errors, workspaces } = composeWorkspaceRoots(roots, null);

  // Two workspaces carrying the same id cannot be composed: every id in one would collide
  // with the other's, so no reference could resolve to a single object. With each workspace
  // at its own id, their files would also land under one __file prefix.
  const seenIds: string[] = [];
  for (const obj of objects) {
    if (obj.__kind === '__Workspace') {
      const wsId = typeof obj.__id === 'string' ? obj.__id : '';
      if (wsId) {
        if (seenIds.includes(wsId)) {
          throw new WorkspaceUsageError(
            `--with paths declare the same workspace id '${wsId}'; ` +
              'ids must be distinct to compose'
          );
        }
        seenIds.push(wsId);
      }
    }
  }

  const composedIndex = rescanComposedReferences(objects, filePaths, errors);

  return {
    root: null, // The base is virtual: each workspace sits at its own id
    workspaceId: null, // A composed set has no single workspace id
    workspaces,
    files,
    objects,
    index: composedIndex,
    errors,
  };
}

/**
 * Entry point for a workspace-aware CLI command (QMD-72).
 *
 * Accepts either the historical positional path or one or more `-w` / `--with` paths, and
 * refuses both at once: they answer different questions ("what is near this path" versus
 * "which workspaces make up this project"), so silently preferring one would answer a
 * question the caller did not ask.
 */
export function resolveWorkspaceInput(
  path: string | undefined,
  withPaths: string[]
): WorkspaceResult {
  if (withPaths.length > 0) {
    if (path !== undefined) {
      throw new WorkspaceUsageError(
        'a positional PATH and --with are mutually exclusive; pass every workspace as --with'
      );
    }
    return composeWithPaths(withPaths);
  }
  return resolveWorkspace(path ?? '.');
}

/**
 * Unified workspace resolver (QMD-59).
 *
 * Lets `workspace parse`/`validate`/`query` work from ANY directory:
 *
 * 1. Walk-UP: if `path` itself or any ancestor is a workspace, parse that
 *    workspace via `parseWorkspace` (preserves nested-workspace detection).
 * 2. Walk-DOWN: otherwise `path` is a non-workspace container; `parseAllWorkspaces`
 *    resolves each contained sub-workspace independently (union of errors),
 *    or falls back to a virtual workspace for orphan files.
 */
export function resolveWorkspace(path: string): WorkspaceResult {
  const root = findWorkspaceRoot(path);
  if (root) {
    return parseWorkspace(root);
  }
  return parseAllWorkspaces(path);
}
