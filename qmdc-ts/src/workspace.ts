/**
 * QMDC Workspace - Multi-file parsing with cross-file references.
 */

import { readFileSync, readdirSync, statSync, existsSync } from 'fs';
import { join, relative, dirname, resolve } from 'path';
import { minimatch } from 'minimatch';
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
    | 'ordered_list_in_array';
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

export interface WorkspaceResult {
  root: string;
  workspaceId: string | null;
  files: string[];
  objects: QmdcObject[];
  index: WorkspaceIndex;
  errors: WorkspaceError[];
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
    const entries = readdirSync(dir, { withFileTypes: true });

    for (const entry of entries) {
      if (!entry.isDirectory()) continue;

      const fullPath = join(dir, entry.name);
      const readme = join(fullPath, 'readme.qmd.md');

      // Check .qmdcignore before processing
      if (isIgnored(readme, rootPath, ignorePatterns)) {
        continue;
      }

      try {
        const content = readFileSync(readme, 'utf-8');
        if (contentHasWorkspaceMarker(content)) {
          roots.push(fullPath);
        }
      } catch {
        // No readme.qmd.md, continue scanning
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
    const entries = readdirSync(dir, { withFileTypes: true });

    for (const entry of entries) {
      const fullPath = join(dir, entry.name);

      if (entry.isDirectory()) {
        // Skip nested workspace directories
        if (nestedRoots.includes(fullPath)) {
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
    const content = readFileSync(nestedReadme, 'utf-8');
    const objects = parse(content);
    const wsObj = findWorkspaceObject(objects);

    if (wsObj) {
      nestedWorkspaceErrors.push({
        type: 'nested_workspace',
        message: `Nested workspace '${wsObj.__id}' found inside workspace. Workspaces cannot be nested.`,
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
    root: rootPath,
    workspaceId,
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
 * Parse reference like [[#ns:Kind:id]] or [[#id]].
 */
function parseReference(ref: string): [string | null, string | null, string] {
  const match = ref.match(/\[\[#([^\]]+)\]\]/);
  if (!match) {
    return [null, null, ref];
  }

  const inner = match[1] || '';
  const parts = inner.split(':');

  if (parts.length === 3) {
    return [parts[0] || null, parts[1] || null, parts[2] || ''];
  } else if (parts.length === 2) {
    // Could be Kind:id or namespace:id
    // Assume Kind:id if first part looks like a Kind (capitalized)
    if (parts[0] && parts[0][0] && parts[0][0] === parts[0][0].toUpperCase()) {
      return [null, parts[0], parts[1] || ''];
    } else {
      return [parts[0] || null, null, parts[1] || ''];
    }
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
  rootPath?: string
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
  const objectsById: Record<string, Array<[string, string, string, number]>> = {};

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
    objectsById[objId].push([objFile, objKind, nsId, objLine]);
  }

  // Check for duplicate IDs (same id, different files or same file)
  // Skip system objects (__Document, __TextBlock) as they are auto-generated per file
  for (const [objId, locations] of Object.entries(objectsById)) {
    // Skip system objects with auto-generated IDs
    const isSystemObject = locations.some(
      ([, kind]) => kind === '__Document' || kind === '__TextBlock'
    );
    if (isSystemObject) {
      continue;
    }

    if (locations.length > 1) {
      // Check if duplicates are in different files
      const files = new Set(locations.map(([file]) => file));
      if (files.size > 1) {
        // Duplicate ID across files
        for (const location of locations.slice(1)) {
          const [file, , , line] = location;
          const candidates = locations.map(([f, , , l]) => `${f}:${l}`);
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
          for (const [file, kind, , line] of locations.slice(1)) {
            const candidates = locations.map(([f, k, , l]) => `${f}:${k}:${l}`);
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

      // Parse reference target to extract namespace, kind, and id
      const [refNs, refKind, refId] = parseReference(target);

      // Find matching objects
      const matchingObjects: Array<[string, string, string, number]> = [];
      if (objectsById[refId]) {
        for (const [file, kind, ns, refLine] of objectsById[refId]) {
          // If reference specifies namespace, must match exactly
          if (refNs !== null) {
            if (ns !== refNs) {
              continue;
            }
          }
          // If reference specifies kind, must match
          if (refKind !== null) {
            if (kind !== refKind) {
              continue;
            }
          }
          // If reference doesn't specify namespace, include all candidates
          matchingObjects.push([file, kind, ns, refLine]);
        }
      }

      // If reference doesn't specify namespace, prefer objects in same namespace as current object
      // According to spec: "current namespace first, then other files in the same namespace"
      // Ambiguous only if:
      // 1. Multiple objects in current namespace, OR
      // 2. No objects in current namespace but multiple in other namespaces
      let resolvedObjects: Array<[string, string, string, number]>;
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

      // Check if reference is inside backticks (inline code) - skip validation
      if (rootPath && objFile) {
        try {
          const filePath = join(rootPath, objFile);
          if (existsSync(filePath)) {
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

        // Filter by target namespace (refNs if explicit, else source obj namespace)
        const targetNs = refNs !== null ? refNs : objNsId;
        let localCandidates: QmdcObject[];
        if (targetNs) {
          localCandidates = localCandidatesRaw.filter(
            (c) => extractNamespaceId((c.__namespace as string) || '') === targetNs
          );
        } else {
          // Root-level: only match other root-level objects
          localCandidates = localCandidatesRaw.filter((c) => !c.__namespace);
        }

        if (localCandidates.length === 1) {
          // Resolved via __local_id — no error
          const matched = localCandidates[0]!;
          resolvedObjects = [
            [
              matched.__file as string,
              (matched.__kind as string) || '',
              extractNamespaceId((matched.__namespace as string) || ''),
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
          if (objectsById[objPrefix]) {
            // Check that the field exists on the target object
            for (const candidateObj of objects) {
              if (candidateObj.__id === objPrefix) {
                if (fieldPart in candidateObj && !fieldPart.startsWith('__')) {
                  isFieldRef = true;
                }
                break;
              }
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
          if (objectsById[objPrefix]) {
            for (const candidateObj of objects) {
              if (candidateObj.__id === objPrefix) {
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
                break;
              }
            }
          }
        }
      } else if (resolvedObjects.length > 1) {
        // Ambiguous reference - multiple matching objects
        const kinds = new Set(resolvedObjects.map(([, kind]) => kind));
        const namespaces = new Set(resolvedObjects.map(([, , ns]) => ns));

        let isAmbiguous = false;
        if (refKind !== null && refNs !== null) {
          isAmbiguous = false; // Fully qualified, should not be ambiguous
        } else if (kinds.size > 1) {
          isAmbiguous = true; // Different kinds
        } else if (namespaces.size > 1) {
          isAmbiguous = true; // Different namespaces
        }

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
  // Output-shape (QMD-59): never emit a bare `workspace: null` when workspaces
  // were actually resolved. Derive workspace id(s) from the resolved objects:
  //   - walk-up/self (workspaceId set)       -> workspace: id
  //   - walk-down, exactly one sub-workspace  -> workspace: that id
  //   - walk-down, multiple sub-workspaces    -> omit workspace, add workspaces: [ids]
  const out: Record<string, unknown> = { root: result.root };

  if (result.workspaceId) {
    out.workspace = result.workspaceId;
  } else {
    const wsIds = Array.from(
      new Set(
        result.objects
          .filter((o) => o.__kind === '__Workspace' && o.__id)
          .map((o) => o.__id as string)
      )
    ).sort();
    if (wsIds.length === 1) {
      out.workspace = wsIds[0];
    } else if (wsIds.length > 1) {
      out.workspaces = wsIds;
    } else {
      out.workspace = null;
    }
  }

  out.files = result.files;
  out.objects = result.objects;
  out.index = {
    byGlobalId: Object.fromEntries(
      Object.entries(result.index.byGlobalId).map(([k, v]) => [k, v.__id]) // Plain IDs
    ),
    byKind: Object.fromEntries(
      Object.entries(result.index.byKind).map(([k, v]) => [k, v.map((o) => o.__id)]) // Plain IDs
    ),
    byFile: Object.fromEntries(
      Object.entries(result.index.byFile).map(([k, v]) => [k, v.map((o) => o.__id)]) // Plain IDs
    ),
  };
  out.errors = result.errors.map((e) => ({
    type: e.type,
    message: e.message,
    file: e.file,
    line: e.line,
    object: e.objectId,
    field: e.fieldName,
    reference: e.reference,
    candidates: e.candidates,
    severity: e.severity,
  }));

  return out;
}

/**
 * Find all workspace directories (directories containing readme.qmd.md with __Workspace).
 */
export function findAllWorkspaceDirs(rootPath: string): string[] {
  const root = resolve(rootPath);
  const workspaceDirs: string[] = [];

  function scanDir(dir: string): void {
    const readmePath = join(dir, 'readme.qmd.md');
    if (existsSync(readmePath)) {
      const content = readFileSync(readmePath, 'utf-8');
      if (contentHasWorkspaceMarker(content)) {
        workspaceDirs.push(dir);
      }
    }

    // Recursively scan subdirectories
    const entries = readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.isDirectory()) {
        scanDir(join(dir, entry.name));
      }
    }
  }

  scanDir(root);
  return workspaceDirs;
}

/**
 * Parse all workspaces found in a directory tree (non-nested).
 *
 * If root_path itself is a workspace, parse only that one.
 * If root_path contains multiple workspace directories, parse all of them.
 */
/**
 * Load .qmdcignore patterns from root directory
 */
function loadQmdcignore(rootPath: string): string[] {
  const qmdcignorePath = join(rootPath, '.qmdcignore');

  if (!existsSync(qmdcignorePath)) {
    return [];
  }

  const content = readFileSync(qmdcignorePath, 'utf-8');
  const patterns: string[] = [];

  for (const line of content.split('\n')) {
    const trimmed = line.trim();

    // Skip empty lines and comments
    if (!trimmed || trimmed.startsWith('#')) {
      continue;
    }

    // If pattern ends with /, replace with /** to match all files within
    const pattern = trimmed.endsWith('/') ? `${trimmed}**` : trimmed;
    patterns.push(pattern);
  }

  return patterns;
}

/**
 * Check if a path should be ignored based on glob patterns
 */
function isIgnored(filePath: string, rootPath: string, patterns: string[]): boolean {
  if (patterns.length === 0) {
    return false;
  }

  const relPath = relative(rootPath, filePath).replace(/\\/g, '/');

  for (const pattern of patterns) {
    // Match against the full path
    if (minimatch(relPath, pattern)) {
      return true;
    }
  }

  return false;
}

/**
 * Recursively find all .qmd.md files in a directory
 */
function findQmdcFiles(dir: string): string[] {
  const results: string[] = [];
  const entries = readdirSync(dir, { withFileTypes: true });

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

  // Root is not a workspace - find all workspaces in subdirectories
  const allWorkspaceDirs = findAllWorkspaceDirs(root);

  // Filter out ignored workspaces
  const workspaceDirs = allWorkspaceDirs.filter((wsDir) => {
    const readme = join(wsDir, 'readme.qmd.md');
    return !isIgnored(readme, root, ignorePatterns);
  });

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
      root,
      workspaceId: null,
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

  // Parse each workspace and combine results
  const allObjects: QmdcObject[] = [];
  const allFiles: string[] = [];
  const allErrors: WorkspaceError[] = [];

  for (const wsDir of workspaceDirs) {
    const wsResult = parseWorkspace(wsDir);

    // Adjust __file paths in objects to be relative to root_path
    for (const obj of wsResult.objects) {
      if (obj.__file && typeof obj.__file === 'string') {
        const fullPath = join(wsDir, obj.__file);
        obj.__file = relative(root, fullPath);
      }
    }

    allObjects.push(...wsResult.objects);

    // Make file paths relative to root_path
    for (const file of wsResult.files) {
      const fullPath = join(wsDir, file);
      const relPath = relative(root, fullPath);
      allFiles.push(relPath);
    }

    // Adjust error file paths to be relative to root_path
    for (const error of wsResult.errors) {
      if (error.file) {
        const fullPath = join(wsDir, error.file);
        error.file = relative(root, fullPath);
      }
      allErrors.push(error);
    }
  }

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
      } catch {
        // Skip files that can't be read
      }
    }
  }

  return {
    root,
    workspaceId: null, // Multiple workspaces, no single ID
    files: allFiles,
    objects: allObjects,
    index: buildIndex(allObjects),
    errors: allErrors,
  };
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
