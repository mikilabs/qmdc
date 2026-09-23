/**
 * SQLite database module for QMDC workspace queries
 */

import initSqlJs, { Database } from 'sql.js';
import type { WorkspaceResult } from './workspace.js';

export interface QueryResult {
  columns: string[];
  rows: unknown[][];
}

/**
 * Parse a reference target into [workspace, namespace, id].
 *
 * QMD-69: a reference target is a right-aligned suffix of the `__global_id` grammar
 * `workspace:namespace:id`, with an optional `.field` suffix carried inside the id. There
 * is no `Kind` segment. An EMPTY middle segment (`ws::id`) ELIDES the namespace rather
 * than asserting the workspace root.
 *
 * Mirrors `parseReference` in workspace.ts and the Rust
 * `core::reference_scan::parse_reference_target`, so the graph and the validator can never
 * disagree about what a reference names.
 */
function parseReferenceTarget(target: string): [string | null, string | null, string] {
  let s = target.startsWith('#') ? target.slice(1) : target;
  // file#id (cross-file reference) -- keep only the id part.
  const hashIdx = s.indexOf('#');
  if (hashIdx !== -1) {
    s = s.slice(hashIdx + 1);
  }
  const parts = s.split(':');
  if (parts.length >= 3) {
    return [parts[0] || null, parts[1] ?? '', parts.slice(2).join(':')];
  }
  if (parts.length === 2) {
    return [null, parts[0] ?? '', parts[1] || ''];
  }
  return [null, null, s];
}

export interface QmdcObject {
  __id?: string;
  __kind?: string;
  __label?: string;
  __namespace?: string;
  __workspace?: string;
  __file?: string;
  __parent?: string;
  __line?: number;
  [key: string]: unknown;
}

/**
 * QMDC SQLite database wrapper
 */
export class QmdcDatabase {
  private db: Database;

  private constructor(db: Database) {
    this.db = db;
  }

  /**
   * Create a new in-memory SQLite database with QMDC schema
   */
  static async create(): Promise<QmdcDatabase> {
    const SQL = await initSqlJs();
    const db = new SQL.Database();
    const qmdcDb = new QmdcDatabase(db);
    qmdcDb.createSchema();
    return qmdcDb;
  }

  private createSchema(): void {
    this.db.run(`
      CREATE TABLE IF NOT EXISTS objects (
        __workspace TEXT NOT NULL,
        __namespace TEXT NOT NULL DEFAULT '',
        __id TEXT NOT NULL,
        __global_id TEXT GENERATED ALWAYS AS (
          __workspace || ':' || CASE WHEN __namespace = '' THEN ':' ELSE __namespace || ':' END || __id
        ) STORED UNIQUE,
        __kind TEXT,
        __label TEXT,
        __local_id TEXT,
        __file TEXT,
        __parent TEXT,
        __line INTEGER,
        __level INTEGER,
        data TEXT,
        PRIMARY KEY (__workspace, __namespace, __id)
      );

      CREATE TABLE IF NOT EXISTS edges (
        source_id TEXT NOT NULL,
        source_field TEXT NOT NULL,
        target_id TEXT NOT NULL,
        edge_type TEXT NOT NULL,
        target_field TEXT NOT NULL DEFAULT '',
        __workspace TEXT,
        UNIQUE(source_id, source_field, target_id, edge_type, target_field),
        FOREIGN KEY (source_id) REFERENCES objects(__global_id),
        FOREIGN KEY (target_id) REFERENCES objects(__global_id)
      );

      CREATE INDEX IF NOT EXISTS idx_objects_kind ON objects(__kind);
      CREATE INDEX IF NOT EXISTS idx_objects_namespace ON objects(__namespace);
      CREATE INDEX IF NOT EXISTS idx_objects_parent ON objects(__parent);
      CREATE INDEX IF NOT EXISTS idx_objects_workspace ON objects(__workspace);
      CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
      CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
    `);
  }

  /**
   * Clear all data
   */
  clear(): void {
    this.db.run('DELETE FROM edges');
    this.db.run('DELETE FROM objects');
  }

  /**
   * Compute __global_id from workspace, namespace, and id.
   * Format: workspace:namespace:id or workspace::id (double colon for empty namespace)
   */
  static computeGlobalId(workspace: string, namespace: string, id: string): string {
    if (!namespace) {
      return `${workspace}::${id}`;
    }
    return `${workspace}:${namespace}:${id}`;
  }

  /**
   * Insert or replace an object
   */
  upsertObject(obj: QmdcObject): void {
    const id = obj.__id || '';
    const kind = obj.__kind || null;
    const label = obj.__label || null;
    const localId = ((obj as Record<string, unknown>).__local_id as string | null) ?? null;
    const namespace = obj.__namespace || '';
    const workspace = obj.__workspace || '';
    const file = obj.__file !== undefined ? obj.__file : null; // Preserve empty string
    // Normalize __parent: extract ID from [[#id]] format
    let parent = obj.__parent || null;
    if (parent && parent.startsWith('[[#') && parent.endsWith(']]')) {
      parent = parent.slice(3, -2);
    }
    const line = obj.__line ?? null;
    const level = typeof obj.__level === 'number' ? obj.__level : null;

    // Build data JSON without system fields.
    // Canonical form (cross-parser byte parity): compact JSON, raw UTF-8, keys
    // in document insertion order. Integer-valued float literals (1.0, 1.00,
    // 2.000) collapse to a JS int and must be re-emitted in canonical `X.0`
    // form to match Rust/Python; the raw source tokens are captured in the
    // non-enumerable `__raw_values` (scalars under `key`, array elements under
    // `key[idx]`).
    const rawValues =
      (Object.getOwnPropertyDescriptor(obj, '__raw_values')?.value as
        | Record<string, string>
        | undefined) ?? {};
    const data = serializeCanonicalData(obj, rawValues);

    this.db.run(
      `INSERT OR REPLACE INTO objects (__workspace, __namespace, __id, __kind, __label, __local_id, __file, __parent, __line, __level, data)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      [workspace, namespace, id, kind, label, localId, file, parent, line, level, data]
    );
  }

  /**
   * Insert an edge (reference).
   * sourceId and targetId should be __global_id values.
   * edgeType defaults to sourceField if not provided.
   * targetField is NULL for whole-object references, non-NULL for field references.
   */
  insertEdge(
    sourceId: string,
    sourceField: string,
    targetId: string,
    edgeType?: string,
    targetField?: string | null,
    workspaceId?: string
  ): void {
    // Extract workspace from sourceId (format: workspace:namespace:id or workspace::id)
    const workspace = sourceId.split(':')[0] || workspaceId || '';
    const actualEdgeType = edgeType ?? sourceField;
    this.db.run(
      'INSERT OR IGNORE INTO edges (source_id, source_field, target_id, edge_type, target_field, __workspace) VALUES (?, ?, ?, ?, ?, ?)',
      [sourceId, sourceField, targetId, actualEdgeType, targetField || '', workspace]
    );
  }

  /**
   * Extract references from object and insert as edges.
   * sourceId should be __id, not __global_id. This method computes __global_id.
   */
  private extractAndInsertEdges(sourceId: string, obj: QmdcObject): void {
    const workspaceId = obj.__workspace || '';
    const namespace = obj.__namespace || '';
    const sourceGlobalId = QmdcDatabase.computeGlobalId(workspaceId, namespace, sourceId);

    for (const [field, value] of Object.entries(obj)) {
      if (field.startsWith('__')) continue;

      this.extractRefsFromValue(sourceGlobalId, field, value, workspaceId, namespace);
    }
  }

  /**
   * Resolve a target reference and insert an edge if the target exists.
   * If the full target_id doesn't resolve as an object and contains a dot,
   * tries splitting off the last segment as a target_field (field-level reference).
   */
  private resolveAndInsertEdge(
    sourceGlobalId: string,
    field: string,
    targetId: string,
    workspaceId: string,
    namespace: string,
    edgeType?: string,
    targetField?: string | null
  ): void {
    const [refWs, refNs, refId] = parseReferenceTarget(targetId);
    const targetGlobalId = this.resolveTargetGlobalId(refId, refWs, refNs, workspaceId, namespace);
    if (targetGlobalId) {
      this.insertEdge(sourceGlobalId, field, targetGlobalId, edgeType, targetField, workspaceId);
    } else if (refId.includes('.') && !targetField) {
      // Field-level resolution: split the ID (never the qualifiers) on the last dot.
      const lastDot = refId.lastIndexOf('.');
      const objPath = refId.slice(0, lastDot);
      const fieldPart = refId.slice(lastDot + 1);
      const objGlobalId = this.resolveTargetGlobalId(objPath, refWs, refNs, workspaceId, namespace);
      if (objGlobalId) {
        this.insertEdge(sourceGlobalId, field, objGlobalId, edgeType, fieldPart, workspaceId);
      }
    }
  }

  private extractRefsFromValue(
    sourceGlobalId: string,
    field: string,
    value: unknown,
    workspaceId: string,
    namespace: string
  ): void {
    if (typeof value === 'string') {
      // Try preamble extraction for text field values
      const preambleEdges = QmdcDatabase.extractPreambleRefs(value);
      if (preambleEdges) {
        const preambleTargets = new Set<string>();
        for (const [preambleKey, targetId] of preambleEdges) {
          this.resolveAndInsertEdge(
            sourceGlobalId,
            field,
            targetId,
            workspaceId,
            namespace,
            preambleKey
          );
          preambleTargets.add(targetId);
        }
        // Also extract remaining refs from the rest of the text
        for (const targetId of this.parseAllReferences(value)) {
          if (!preambleTargets.has(targetId)) {
            this.resolveAndInsertEdge(sourceGlobalId, field, targetId, workspaceId, namespace);
          }
        }
      } else {
        // No preamble — standard extraction
        for (const targetId of this.parseAllReferences(value)) {
          this.resolveAndInsertEdge(sourceGlobalId, field, targetId, workspaceId, namespace);
        }
      }
    } else if (Array.isArray(value)) {
      for (const item of value) {
        this.extractRefsFromValue(sourceGlobalId, field, item, workspaceId, namespace);
      }
    }
  }

  /**
   * Extract typed edges from text field preamble.
   * A preamble is a markdown list at the start of a text field where ALL items
   * are valid `- key: [[#ref]]` fields. All-or-nothing.
   */
  private static extractPreambleRefs(text: string): [string, string][] | null {
    if (!text || !text.startsWith('- ')) return null;

    const preambleBlock = text.split('\n\n', 1)[0]!;
    const lines = preambleBlock.split('\n');
    const edges: [string, string][] = [];

    const fieldKeyRe = /^[a-zA-Z_][a-zA-Z0-9_]*$/;
    const singleRefRe = /^\[\[#[^\]]+\]\]$/;
    const multiRefRe = /^\[\[#[^\]]+\]\](?:\s*,\s*\[\[#[^\]]+\]\])+$/;
    const refExtractRe = /\[\[#([^\]]+)\]\]/g;

    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line) continue;
      if (!line.startsWith('- ')) return null;

      const content = line.slice(2).trim();
      const colonIdx = content.indexOf(':');
      if (colonIdx <= 0) return null;

      const key = content.slice(0, colonIdx).trim();
      const val = content.slice(colonIdx + 1).trim();

      if (!fieldKeyRe.test(key)) return null;

      if (singleRefRe.test(val) || multiRefRe.test(val)) {
        let match;
        refExtractRe.lastIndex = 0;
        while ((match = refExtractRe.exec(val)) !== null) {
          // QMD-69: keep the FULL reference target, qualifiers included.
          const targetRef = match[1]!.trim();
          edges.push([key, targetRef]);
        }
      } else {
        return null;
      }
    }

    return edges.length > 0 ? edges : null;
  }

  /**
   * Resolve a target __global_id from a parsed reference target (QMD-69).
   *
   * Each qualifier present narrows the search; the search NEVER widens past the referring
   * object's own workspace unless the reference names one. There is no "any workspace"
   * fallback: a cross-workspace reference must be qualified, and an unresolvable or
   * ambiguous target returns null so no edge is built. That is what keeps the graph and
   * `qmdc workspace validate` in agreement.
   *
   * `refNamespace === ''` is the ELIDED form `ws::id`: any namespace of the named
   * workspace, ambiguous if more than one candidate matches.
   */
  private resolveTargetGlobalId(
    targetId: string,
    refWorkspace: string | null,
    refNamespace: string | null,
    workspaceId: string,
    namespace: string
  ): string | null {
    // The workspace to search: the one the reference names, otherwise the referring object's
    // own. There is no widening past it.
    const effWorkspace = refWorkspace !== null ? refWorkspace : workspaceId;
    const elided = refNamespace === '';
    // A concrete namespace is an assertion. The elided form searches every namespace of
    // effWorkspace. A bare id means own namespace first, then any namespace.
    const concreteNamespace = elided ? null : refNamespace !== null ? refNamespace : namespace;

    // First try: an exact __global_id, available whenever the namespace is concrete.
    if (concreteNamespace !== null) {
      const candidate = QmdcDatabase.computeGlobalId(effWorkspace, concreteNamespace, targetId);
      const stmt1 = this.db.prepare('SELECT 1 FROM objects WHERE __global_id = ? LIMIT 1');
      stmt1.bind([candidate]);
      const found = stmt1.step();
      stmt1.free();
      if (found) {
        return candidate;
      }
    }

    // Second try: any namespace of effWorkspace. Reached by the elided form, and by a bare id
    // whose own namespace held nothing. NOT reached by a concrete namespace qualifier, which
    // must not fall through to a different namespace.
    //
    // LIMIT 2 so that several candidates are an ambiguity rather than an arbitrary pick --
    // `validate` reports `ambiguous_reference` for exactly this case, so the graph must build
    // no edge.
    //
    // A `__Workspace` root object carries no own `__workspace` (it IS the workspace), so it is
    // reachable only when the id being looked up is that workspace's own name.
    if (elided || refNamespace === null) {
      const stmt2 = this.db.prepare(
        'SELECT __global_id FROM objects WHERE __id = ?' +
          " AND (__workspace = ? OR (__workspace = '' AND __id = ?)) LIMIT 2"
      );
      stmt2.bind([targetId, effWorkspace, effWorkspace]);
      const rows: string[] = [];
      while (stmt2.step()) {
        rows.push(stmt2.get()[0] as string);
      }
      stmt2.free();
      if (rows.length === 1) {
        return rows[0]!;
      }
      if (rows.length > 1) {
        return null;
      }
    }

    // Third try: the __local_id fallback for short-form references like [[#child]], which
    // names a hierarchical object by its last segment.
    //
    // QMD-69: the WORKSPACE searched is the one the reference names, not the referring
    // object's. It used to be hard-wired to the source's own workspace, so spelling out the
    // workspace that actually holds the target made the reference stop resolving.
    //
    // The NAMESPACE rule is the historical one and is NOT the elide/assert rule used above: an
    // unqualified reference resolves by __local_id only inside the referring object's OWN
    // namespace. That keeps a bare [[#config]] at the workspace root from reaching
    // gateway.config in the `services` namespace.
    //
    // LIMIT 2 so that 0 or >1 matches return null (not found or ambiguous).
    let stmt4;
    if (elided) {
      stmt4 = this.db.prepare(
        'SELECT __global_id FROM objects WHERE __local_id = ? AND __workspace = ? LIMIT 2'
      );
      stmt4.bind([targetId, effWorkspace]);
    } else {
      const localNs = refNamespace !== null ? refNamespace : namespace;
      stmt4 = this.db.prepare(
        'SELECT __global_id FROM objects' +
          ' WHERE __local_id = ? AND __workspace = ? AND __namespace = ? LIMIT 2'
      );
      stmt4.bind([targetId, effWorkspace, localNs]);
    }
    const localRows: string[] = [];
    while (stmt4.step()) {
      localRows.push(stmt4.get()[0] as string);
    }
    stmt4.free();
    if (localRows.length === 1) {
      return localRows[0]!;
    }

    return null;
  }

  private parseAllReferences(s: string): string[] {
    // Find ALL [[#id]] patterns in the string, return list of target ids
    const targets: string[] = [];
    const pattern = /\[\[#([^\]]+)\]\]/g;
    let match;
    while ((match = pattern.exec(s)) !== null) {
      const inner = match[1];
      if (inner) {
        // QMD-69: keep the FULL reference target, qualifiers included.
        const targetRef = inner.trim();
        if (targetRef) {
          targets.push(targetRef);
        }
      }
    }
    return targets;
  }

  /**
   * Sync objects from workspace.
   * Two passes: first insert all objects, then extract and insert edges.
   * This ensures all referenced objects exist before edges are created.
   */
  syncObjects(objects: QmdcObject[]): void {
    this.clear();

    // First pass: insert all objects
    for (const obj of objects) {
      this.upsertObject(obj);
    }

    // Second pass: extract and insert edges
    for (const obj of objects) {
      if (obj.__id) {
        this.extractAndInsertEdges(obj.__id, obj);
      }
    }
  }

  /**
   * Execute a SQL query
   */
  query(sql: string): QueryResult {
    const trimmedSql = sql.trim();

    // Handle dot-commands
    if (trimmedSql.startsWith('.')) {
      return this.handleDotCommand(trimmedSql);
    }

    const stmt = this.db.prepare(trimmedSql);
    const columns = stmt.getColumnNames();
    const rows: unknown[][] = [];

    while (stmt.step()) {
      rows.push(stmt.get());
    }
    stmt.free();

    return { columns, rows };
  }

  private handleDotCommand(cmd: string): QueryResult {
    const parts = cmd.split(/\s+/);
    const command = parts[0];

    switch (command) {
      case '.tables':
        return this.query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name");
      case '.schema': {
        const table = parts[1];
        if (table) {
          return this.query(`SELECT sql FROM sqlite_master WHERE type='table' AND name='${table}'`);
        }
        return this.query("SELECT name, sql FROM sqlite_master WHERE type='table' ORDER BY name");
      }
      case '.help':
        return {
          columns: ['command', 'description'],
          rows: [
            ['.tables', 'List all tables'],
            ['.schema [table]', 'Show table schema'],
            ['.help', 'Show this help'],
          ],
        };
      default:
        throw new Error(`Unknown command: ${command}. Try .help`);
    }
  }

  /**
   * Format result as text table (full width, no truncation)
   */
  static toTableString(result: QueryResult): string {
    if (result.rows.length === 0) {
      return '(empty result)\n';
    }

    // Calculate column widths based on actual content
    const widths: number[] = result.columns.map((c) => c.length);
    for (const row of result.rows) {
      for (let i = 0; i < Math.min(row.length, widths.length); i++) {
        // Normalize whitespace
        const val = String(row[i] ?? 'NULL')
          .replace(/\s+/g, ' ')
          .trim();
        widths[i] = Math.max(widths[i]!, val.length);
      }
    }

    const formatCell = (s: string, w: number): string => {
      const clean = s.replace(/\s+/g, ' ').trim();
      return clean.padEnd(w);
    };

    let output = '';

    // Header
    const header = result.columns.map((c, i) => formatCell(c, widths[i]!));
    output += header.join(' | ') + '\n';

    // Separator
    output += widths.map((w) => '-'.repeat(w)).join('-+-') + '\n';

    // Rows
    for (const row of result.rows) {
      const rowStrs = row.map((v, i) => formatCell(String(v ?? 'NULL'), widths[i] ?? 10));
      output += rowStrs.join(' | ') + '\n';
    }

    return output;
  }

  /**
   * Close the database
   */
  close(): void {
    this.db.close();
  }
}

/**
 * Execute a query against a workspace result.
 *
 * The query can be:
 * - A SQL query (e.g., "SELECT * FROM objects")
 * - A reference to a Query object (e.g., "#get_tables")
 */
export async function executeQuery(
  workspace: WorkspaceResult,
  query: string
): Promise<QueryResult> {
  const db = await QmdcDatabase.create();

  try {
    // Sync objects
    db.syncObjects(workspace.objects as QmdcObject[]);

    // Resolve query
    let sql: string;
    if (query.startsWith('#')) {
      // Find Query object by ID
      const queryId = query.slice(1);
      const queryObj = workspace.objects.find(
        (obj) => obj.__id === queryId && obj.__kind === 'Query'
      );
      if (!queryObj) {
        throw new Error(`Query object '${queryId}' not found`);
      }
      if (typeof queryObj.sql !== 'string') {
        throw new Error(`Query object '${queryId}' has no 'sql' field`);
      }
      sql = queryObj.sql;
    } else {
      sql = query;
    }

    return db.query(sql);
  } finally {
    db.close();
  }
}

/**
 * Canonicalize a raw float token to the cross-parser form: a single value with
 * trailing zeros trimmed but at least one fractional digit kept.
 *   "1.0"   -> "1.0"
 *   "1.00"  -> "1.0"
 *   "2.000" -> "2.0"
 *   "2.50"  -> "2.5"
 * This matches how Rust (serde_json) and Python (json.dumps on a float) emit an
 * integer-valued float. The numeric value is unchanged — only redundant trailing
 * zeros are dropped. Tokens without a '.' are returned unchanged.
 */
function canonicalizeFloatToken(raw: string): string {
  if (!raw.includes('.')) return raw;
  let s = raw.replace(/0+$/, '');
  if (s.endsWith('.')) s += '0';
  return s;
}

/**
 * Serialize an object's user fields (non-`__` keys) to the canonical JSON form
 * used for the SQLite `data` column: compact, raw UTF-8, document insertion
 * order, with integer-valued float literals restored to `X.0` form from
 * `rawValues`. This is the single place the cross-parser byte contract for the
 * `data` column lives in the TS parser.
 *
 * rawValues holds raw source tokens for numbers JS would otherwise collapse:
 * scalars under `key`, array elements under `key[idx]`.
 */
function serializeCanonicalData(
  obj: Record<string, unknown>,
  rawValues: Record<string, string>
): string {
  const parts: string[] = [];
  for (const [key, value] of Object.entries(obj)) {
    if (key.startsWith('__')) continue;
    const keyJson = JSON.stringify(key);

    if (typeof value === 'number' && rawValues[key] !== undefined) {
      // Scalar integer-valued float literal (e.g. "1.00" -> "1.0").
      parts.push(`${keyJson}:${canonicalizeFloatToken(rawValues[key]!)}`);
      continue;
    }

    if (Array.isArray(value)) {
      const elems: string[] = [];
      for (let idx = 0; idx < value.length; idx++) {
        const elem = value[idx];
        const rawTok = rawValues[`${key}[${idx}]`];
        if (typeof elem === 'number' && rawTok !== undefined) {
          elems.push(canonicalizeFloatToken(rawTok));
        } else {
          const j = JSON.stringify(elem);
          // Skip undefined/function elements (JSON.stringify yields undefined);
          // JSON.stringify on an array would emit `null` for these, so mirror
          // that to keep array length stable.
          elems.push(j === undefined ? 'null' : j);
        }
      }
      parts.push(`${keyJson}:[${elems.join(',')}]`);
      continue;
    }

    const j = JSON.stringify(value);
    // Guard against undefined/function values that JSON.stringify drops — match
    // the old behavior of omitting such keys rather than emitting invalid JSON.
    if (j === undefined) continue;
    parts.push(`${keyJson}:${j}`);
  }
  return `{${parts.join(',')}}`;
}
