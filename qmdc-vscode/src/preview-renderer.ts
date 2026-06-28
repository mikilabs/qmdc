import { marked } from 'marked';
import * as yaml from 'js-yaml';
import * as fs from 'fs';
import * as path from 'path';

/**
 * Interface for executing SQL queries against the workspace.
 * In VS Code, this is backed by the LSP client.
 * In tests, this can be stubbed.
 */
export interface QueryExecutor {
  executeQuery(sql: string, documentUri: string, scope: string): Promise<{
    success: boolean;
    columns?: string[];
    rows?: any[][];
    error?: string;
  }>;
}

/**
 * Parse block content to extract SQL query and scope.
 * Supports:
 * - query: [[#query_id]]  → returns "#query_id" (LSP will resolve)
 * - sql: SELECT ...       → returns the SQL
 * - scope: workspace | all  → returns scope (default: "workspace")
 */
export function parseBlockContent(content: string): { sql: string | null; scope: string } {
  let scope = 'workspace';

  // Try to parse as YAML first
  try {
    const parsed = yaml.load(content) as any;

    if (parsed && typeof parsed === 'object') {
      if (parsed.scope && typeof parsed.scope === 'string') {
        scope = parsed.scope.toLowerCase() === 'all' ? 'all' : 'workspace';
      }

      if (parsed.query && typeof parsed.query === 'string') {
        const queryMatch = parsed.query.match(/\[\[#?([^\]]+)\]\]/);
        if (queryMatch) {
          return { sql: '#' + queryMatch[1], scope };
        }
      }

      if (parsed.sql && typeof parsed.sql === 'string') {
        return { sql: parsed.sql.trim(), scope };
      }
    }
  } catch {
    // YAML parse error, try regex fallback
  }

  // Fallback: regex parsing
  const lines = content.split('\n');
  for (const line of lines) {
    const trimmed = line.trim();

    const scopeMatch = trimmed.match(/^scope:\s*(.+)/i);
    if (scopeMatch) {
      scope = scopeMatch[1].trim().toLowerCase() === 'all' ? 'all' : 'workspace';
    }

    const queryMatch = trimmed.match(/^query:\s*\[\[#?([^\]]+)\]\]/);
    if (queryMatch) {
      return { sql: '#' + queryMatch[1], scope };
    }

    const sqlMatch = trimmed.match(/^sql:\s*(.+)/);
    if (sqlMatch) {
      return { sql: sqlMatch[1].trim(), scope };
    }
  }

  // Final fallback: raw SQL
  if (content.trim().toUpperCase().startsWith('SELECT')) {
    return { sql: content.trim(), scope };
  }

  return { sql: null, scope };
}

/**
 * Escape HTML special characters
 */
export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Render query result as HTML table
 */
export function renderHtmlTable(columns: string[], rows: any[][]): string {
  if (!rows || rows.length === 0) {
    return '<p><em>(empty result)</em></p>';
  }

  const headerHtml = columns.map(col => `<th>${escapeHtml(col)}</th>`).join('');
  const rowsHtml = rows.map(row => {
    const cells = row.map(cell => `<td>${escapeHtml(String(cell ?? 'NULL'))}</td>`).join('');
    return `<tr>${cells}</tr>`;
  }).join('');

  return `<table>
    <thead><tr>${headerHtml}</tr></thead>
    <tbody>${rowsHtml}</tbody>
  </table>`;
}

/**
 * Transform QMDC-specific syntax in content before markdown processing.
 * Extracts code fences first to protect them, then transforms [[id]] and [[#ref]].
 * Also converts ```mermaid code blocks into <div class="mermaid"> for rendering.
 */
export function transformQmdcSyntax(content: string): string {
  // Extract code fences to protect them from QMDC link transformation.
  // Mermaid blocks are converted to <div class="mermaid"> instead of being preserved.
  // The info string regex `([^\n]*)` captures the full line after the fence, including
  // modifiers like "example" (e.g. ```markdown example, ```json example).
  //
  // Fences use >=3 backticks and the closing fence must have the SAME number of
  // backticks (CommonMark). This lets a 4-backtick wrapper contain 3-backtick fences
  // (e.g. an `example` block that shows ```code```) without closing early — the source
  // of the old `___CODE_FENCE___` placeholder leaks on nested/unknown fences. Anchored
  // to line starts (m flag) so inline backticks never trigger a false fence.
  const codeFences: string[] = [];
  const codeFenceRegex = /^(`{3,})([^\n]*)\n([\s\S]*?)^\1[ \t]*$/gm;
  // Use a single replaceAll pass to avoid fragile content.replace(fullMatch, ...) on duplicates
  content = content.replace(codeFenceRegex, (fullMatch, _fence, infoString, body) => {
    const lang = (infoString || '').trim().toLowerCase();
    if (lang === 'mermaid') {
      codeFences.push(`<div class="mermaid">\n${body.trim()}\n</div>`);
    } else {
      codeFences.push(fullMatch);
    }
    return `___CODE_FENCE_${codeFences.length - 1}___`;
  });

  // Also protect inline code spans (`...`) from QMDC transformation
  const inlineCode: string[] = [];
  content = content.replace(/`([^`]+)`/g, (fullMatch) => {
    inlineCode.push(fullMatch);
    return `___INLINE_CODE_${inlineCode.length - 1}___`;
  });

  // 1. Transform [[id: Kind]] definitions — hide ID span but show Kind as a badge
  //    Exclude patterns starting with # (those are references, handled in step 3)
  //    System types (__Namespace, __Workspace, etc.) stay fully hidden
  //    data-pagefind-filter="kind" enables faceted search filtering by object type
  //    data-pagefind-ignore on qmdc-id prevents raw [[id: Kind]] from appearing in search snippets
  content = content.replace(
    /\[\[([^\]:#]+):\s*([^\]]+)\]\]/g,
    (_, id, kind) => {
      const trimKind = kind.trim();
      if (trimKind.startsWith('__')) {
        return `<span class="qmdc-id" data-pagefind-ignore id="${id}">[[${id}: ${trimKind}]]</span>`;
      }
      return `<span class="qmdc-id" data-pagefind-ignore id="${id}">[[${id}: ${trimKind}]]</span><span class="qmdc-kind" data-pagefind-filter="kind">${trimKind}</span>`;
    }
  );

  // 1b. Transform [[:Kind]] definitions (Kind-only, auto-generated ID) — hide them
  content = content.replace(
    /\[\[:([^\]]+)\]\]/g,
    '<span class="qmdc-id" data-pagefind-ignore>[[:$1]]</span>'
  );

  // 2. Transform [[id]] definitions (without Kind) — hide them
  content = content.replace(
    /\[\[([^\]#:]+)\]\]/g,
    '<span class="qmdc-id" data-pagefind-ignore id="$1">[[$1]]</span>'
  );

  // 3. Transform [[#ref]] references to clickable links (using data-ref + delegated handler, no inline onclick)
  content = content.replace(
    /\[\[#([^\]]+)\]\]/g,
    (_, refId) => {
      const escaped = escapeHtml(refId);
      return `<a href="#" class="qmdc-ref" data-ref="${escaped}">${escaped}</a>`;
    }
  );

  // Restore code fences (including mermaid divs)
  content = content.replace(
    /___CODE_FENCE_(\d+)___/g,
    (_, index) => codeFences[parseInt(index)]
  );

  // Restore inline code spans
  content = content.replace(
    /___INLINE_CODE_(\d+)___/g,
    (_, index) => inlineCode[parseInt(index)]
  );

  return content;
}

// ── Sidebar: page TOC + graph context ──────────────────────────────────────

/** Human-readable verb for edge types */
const OUTGOING_VERBS: Record<string, string> = {
  depends: 'depends on', validates: 'validates', about: 'describes',
  affects: 'affects', uses: 'uses', implements: 'implements',
  extends: 'extends', contains: 'contains', references: 'references',
  content: 'includes', features: 'provides', description: 'describes',
  realized_by: 'realized by', uses_lib: 'uses', uses_sdk: 'uses SDK',
  capabilities: 'capabilities', modules: 'modules', contracts: 'contracts',
  resources: 'resources',
};
const INCOMING_VERBS: Record<string, string> = {
  depends: 'needed by', validates: 'validated by', about: 'described in',
  affects: 'affected by', uses: 'used by', implements: 'implemented by',
  extends: 'extended by', contains: 'part of', references: 'referenced in',
  content: 'included in', features: 'feature of', description: 'described in',
  realized_by: 'realizes', uses_lib: 'used by', uses_sdk: 'SDK used by',
  capabilities: 'capability of', modules: 'module of', contracts: 'contract of',
  resources: 'resource of', capability: 'capability',
};

function friendlyVerb(edgeType: string, dir: 'out' | 'in'): string {
  const table = dir === 'out' ? OUTGOING_VERBS : INCOMING_VERBS;
  return table[edgeType] || edgeType.replace(/_/g, ' ');
}

/** Extract page TOC from rendered HTML (h2/h3 headings with their IDs) */
export function extractPageToc(html: string): { level: number; id: string; text: string }[] {
  const toc: { level: number; id: string; text: string }[] = [];
  const headingRegex = /<h([23])[^>]*>([\s\S]*?)<\/h\1>/gi;
  let match;
  while ((match = headingRegex.exec(html)) !== null) {
    const level = parseInt(match[1]);
    const inner = match[2];
    // Extract id from <span class="qmdc-id" ... id="xxx">
    const idMatch = inner.match(/class="qmdc-id"[^>]*\sid="([^"]+)"/);
    if (!idMatch) continue;
    const id = idMatch[1];
    // Strip .qmdc-id spans entirely (they contain [[id]] text), then strip remaining tags
    const text = inner
      .replace(/<span class="qmdc-id"[^>]*>[\s\S]*?<\/span>/g, '')
      .replace(/<span class="qmdc-kind"[^>]*>[^<]*<\/span>/g, '')
      .replace(/<[^>]+>/g, '')
      .trim();
    if (text) toc.push({ level, id, text });
  }
  return toc;
}

interface GraphContext {
  breadcrumb: { label: string; type: string; file?: string; id?: string }[];
  siblings: { label: string; kinds: string; current: boolean; file: string; id?: string }[];
  linksTo: { verb: string; label: string; kind: string; file: string; id?: string }[];
  linkedFrom: { verb: string; label: string; kind: string; file: string; id?: string }[];
  currentFile: string;
}

/** Fetch graph navigation context via SQL queries */
export async function fetchGraphContext(
  queryExecutor: QueryExecutor,
  documentUri: string,
): Promise<GraphContext | null> {
  const ctx: GraphContext = { breadcrumb: [], siblings: [], linksTo: [], linkedFrom: [], currentFile: '' };

  // Extract relative file path from URI for SQL filtering.
  // documentUri is like "file:///abs/path/to/workspace/lsp/diagnostics.qmd.md"
  // We try progressively longer suffixes until we find a unique match.
  const uriPath = documentUri.replace(/^file:\/\//, '');
  const pathParts = uriPath.split('/');

  try {
    // Try matching from the filename, then add parent dirs until unique
    let relFile: string | null = null;
    let ns = '';
    let lastMultipleResults: any[] | null = null;
    for (let i = pathParts.length - 1; i >= 0; i--) {
      const suffix = pathParts.slice(i).join('/');
      if (!suffix) continue;
      const result = await queryExecutor.executeQuery(
        `SELECT __file, __namespace FROM objects WHERE __file LIKE '%${suffix.replace(/'/g, "''")}' LIMIT 3`,
        documentUri, 'workspace'
      );
      if (result?.success && result.rows?.length === 1) {
        relFile = result.rows[0][0];
        ns = result.rows[0][1] || '';
        break;
      }
      if (result?.success && result.rows && result.rows.length >= 2) {
        lastMultipleResults = result.rows;
      }
      if (result?.success && result.rows?.length === 0 && lastMultipleResults) {
        // Previous suffix matched multiple files, this one matches none.
        // The file is ambiguous by name — pick the shortest __file (root file).
        const shortest = lastMultipleResults.sort((a: any, b: any) => a[0].length - b[0].length)[0];
        relFile = shortest[0];
        ns = shortest[1] || '';
        break;
      }
    }
    if (!relFile) return null;

    ctx.currentFile = relFile;
    const f = relFile.replace(/'/g, "''");

    // For namespace readme files, the __namespace is empty but the file IS the namespace.
    if (!ns) {
      const nsCheck = await queryExecutor.executeQuery(
        `SELECT __id FROM objects WHERE __file = '${f}' AND __kind = '__Namespace' LIMIT 1`,
        documentUri, 'workspace'
      );
      if (nsCheck?.success && nsCheck.rows?.[0]?.[0]) {
        ns = nsCheck.rows[0][0];
      }
    }

    // 1–4. Fetch all graph data in parallel (workspace, namespace, file label, siblings, edges)
    const nsFilter = ns
      ? `(__namespace = '${ns}' OR (__file = '${f}' AND substr(__kind,1,2) != '__'))`
      : `(__namespace IS NULL OR __namespace = '')`;

    const [wsResult, nsResult, fileLabelResult, sibResult, sibLabelResult, outResult, inResult] = await Promise.all([
      // Workspace label
      queryExecutor.executeQuery(
        `SELECT __label, __file, __id FROM objects WHERE __kind = '__Workspace' LIMIT 1`,
        documentUri, 'workspace'
      ),
      // Namespace label
      ns ? queryExecutor.executeQuery(
        `SELECT __label, __file, __id FROM objects WHERE __kind = '__Namespace' AND __id = '${ns}' LIMIT 1`,
        documentUri, 'workspace'
      ) : Promise.resolve(null),
      // File label
      queryExecutor.executeQuery(
        `SELECT __label FROM objects WHERE __file = '${f}' AND __level = 1 AND substr(__kind,1,2) != '__' LIMIT 1`,
        documentUri, 'workspace'
      ),
      // Siblings (kinds per file)
      queryExecutor.executeQuery(
        `SELECT __file, __kind, COUNT(*) as cnt FROM objects
         WHERE ${nsFilter} AND substr(__kind,1,2) != '__'
         GROUP BY __file, __kind ORDER BY __file, __kind`,
        documentUri, 'workspace'
      ),
      // Sibling labels (+ IDs for preview navigation)
      queryExecutor.executeQuery(
        `SELECT __file, __label, __id FROM objects
         WHERE ${nsFilter} AND __level = 1 AND substr(__kind,1,2) != '__' AND __label IS NOT NULL`,
        documentUri, 'workspace'
      ),
      // Outgoing edges
      queryExecutor.executeQuery(
        `SELECT DISTINCT e.edge_type, t.__label, t.__kind, t.__file, t.__id
         FROM edges e
         JOIN objects s ON e.source_id = s.__global_id
         JOIN objects t ON e.target_id = t.__global_id
         WHERE s.__file = '${f}' AND t.__file != '${f}' AND substr(t.__kind,1,2) != '__'
         ORDER BY e.edge_type, t.__label`,
        documentUri, 'workspace'
      ),
      // Incoming edges
      queryExecutor.executeQuery(
        `SELECT DISTINCT e.edge_type, s.__label, s.__kind, s.__file, s.__id
         FROM edges e
         JOIN objects s ON e.source_id = s.__global_id
         JOIN objects t ON e.target_id = t.__global_id
         WHERE t.__file = '${f}' AND s.__file != '${f}' AND substr(s.__kind,1,2) != '__'
         ORDER BY e.edge_type, s.__label`,
        documentUri, 'workspace'
      ),
    ]);

    // Process results: breadcrumb
    if (wsResult?.success && wsResult.rows?.[0]) {
      ctx.breadcrumb.push({ label: wsResult.rows[0][0], type: 'workspace', file: wsResult.rows[0][1] || 'readme.qmd.md', id: wsResult.rows[0][2] || '' });
    }
    if (ns && nsResult?.success && nsResult.rows?.[0]) {
      ctx.breadcrumb.push({ label: nsResult.rows[0][0], type: 'namespace', file: nsResult.rows[0][1] || '', id: nsResult.rows[0][2] || '' });
    }
    const baseName = relFile.replace(/.*\//, '').replace(/\.qmd\.md$/, '');
    let fileLabel = baseName.replace(/[-_]/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase());
    if (fileLabel.toLowerCase() === 'readme') fileLabel = 'Overview';
    if (fileLabelResult?.success && fileLabelResult.rows?.[0]?.[0]) {
      fileLabel = fileLabelResult.rows[0][0];
    }
    ctx.breadcrumb.push({ label: fileLabel, type: 'file' });

    // Process results: siblings
    const sibLabels: Record<string, string> = {};
    const sibIds: Record<string, string> = {};
    if (sibLabelResult?.success && sibLabelResult.rows) {
      for (const [sf, sl, sid] of sibLabelResult.rows) {
        if (sf && sl) sibLabels[sf] = sl;
        if (sf && sid) sibIds[sf] = sid;
      }
    }
    if (sibResult?.success && sibResult.rows) {
      const fileKinds: Record<string, string[]> = {};
      for (const [sibFile, kind, _cnt] of sibResult.rows) {
        if (!sibFile) continue;
        (fileKinds[sibFile] ??= []).push(kind);
      }
      for (const [sibFile, kinds] of Object.entries(fileKinds).sort()) {
        let label = sibLabels[sibFile] || '';
        if (!label) {
          label = sibFile.replace(/.*\//, '').replace(/\.qmd\.md$/, '');
          label = label.replace(/[-_]/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase());
          if (label.toLowerCase() === 'readme') label = 'Overview';
        }
        const isCurrent = sibFile === relFile;
        ctx.siblings.push({ label, kinds: kinds.join(', '), current: isCurrent, file: sibFile, id: sibIds[sibFile] || '' });
      }
    }

    // Process results: outgoing edges
    if (outResult?.success && outResult.rows) {
      for (const [edgeType, label, kind, file, id] of outResult.rows) {
        if (!label) continue;
        ctx.linksTo.push({ verb: friendlyVerb(edgeType, 'out'), label, kind, file: file || '', id: id || '' });
      }
    }

    // Process results: incoming edges
    if (inResult?.success && inResult.rows) {
      for (const [edgeType, label, kind, file, id] of inResult.rows) {
        if (!label) continue;
        ctx.linkedFrom.push({ verb: friendlyVerb(edgeType, 'in'), label, kind, file: file || '', id: id || '' });
      }
    }

    // Only return if we got meaningful data
    if (ctx.breadcrumb.length > 0 || ctx.siblings.length > 0 || ctx.linksTo.length > 0 || ctx.linkedFrom.length > 0) {
      return ctx;
    }
  } catch {
    // Graph queries failed — degrade gracefully
  }
  return null;
}

/** Render the sidebar HTML from page TOC and graph context.
 *  Navigation uses navigateToRef() via qmdc-ref links, with a file-href fallback
 *  when an object id is unavailable.
 */
export function renderSidebar(
  toc: { level: number; id: string; text: string }[],
  graph: GraphContext | null,
): string {
  const sections: string[] = [];

  // Breadcrumb
  if (graph?.breadcrumb.length) {
    const currentDir = graph.currentFile?.replace(/[^/]*$/, '') || '';
    const crumbs = graph.breadcrumb.map(b => {
      if (b.type !== 'file' && (b.file || b.id)) {
        if (b.id) {
          return `<a href="#" class="sb-crumb sb-crumb--${b.type} qmdc-ref" data-ref="${escapeHtml(b.id)}">${escapeHtml(b.label)}</a>`;
        } else if (b.file) {
          let href = b.file.replace(/\.qmd\.md$/, '.html');
          if (currentDir && href.startsWith(currentDir)) {
            href = href.substring(currentDir.length);
          } else if (currentDir) {
            const upCount = currentDir.split('/').filter(Boolean).length;
            href = '../'.repeat(upCount) + href;
          }
          return `<a href="${escapeHtml(href)}" class="sb-crumb sb-crumb--${b.type}">${escapeHtml(b.label)}</a>`;
        }
      }
      return `<span class="sb-crumb sb-crumb--${b.type}">${escapeHtml(b.label)}</span>`;
    }).join('<span class="sb-crumb-sep">›</span>');
    sections.push(`<nav class="sb-breadcrumb">${crumbs}</nav>`);
  }

  // Page TOC
  if (toc.length > 0) {
    const tocItems = toc.map(h => {
      const indent = h.level === 3 ? ' sb-toc-indent' : '';
      // Use JS scroll — the anchor span is display:none so native #hash won't work
      return `<a href="javascript:void(0)" class="sb-toc-link${indent}" onclick="var t=document.getElementById('${escapeHtml(h.id)}');if(t){var h=t.closest('h1,h2,h3,h4,h5,h6')||t.parentElement;if(h)h.scrollIntoView({block:'start',behavior:'smooth'})}">${escapeHtml(h.text)}</a>`;
    }).join('\n');
    sections.push(`<div class="sb-section">
      <div class="sb-section-title">On this page</div>
      <div class="sb-toc">${tocItems}</div>
    </div>`);
  }

  // Siblings
  if (graph && graph.siblings.length > 1) {
    const currentDir = graph.currentFile?.replace(/[^/]*$/, '') || '';
    const sibItems = graph.siblings.map(s => {
      if (s.current) {
        return `<div class="sb-sib sb-sib--current">${escapeHtml(s.label)}<span class="sb-sib-kinds">${escapeHtml(s.kinds)}</span></div>`;
      }
      if (s.id) {
        // Navigate to the object via navigateToRef
        return `<a href="#" class="sb-sib qmdc-ref" data-ref="${escapeHtml(s.id)}">${escapeHtml(s.label)}<span class="sb-sib-kinds">${escapeHtml(s.kinds)}</span></a>`;
      }
      // Fallback: use file href
      let href = s.file.replace(/\.qmd\.md$/, '.html');
      if (currentDir && href.startsWith(currentDir)) {
        href = href.substring(currentDir.length);
      } else if (currentDir) {
        const upCount = currentDir.split('/').filter(Boolean).length;
        href = '../'.repeat(upCount) + href;
      }
      return `<a href="${escapeHtml(href)}" class="sb-sib">${escapeHtml(s.label)}<span class="sb-sib-kinds">${escapeHtml(s.kinds)}</span></a>`;
    }).join('\n');
    sections.push(`<div class="sb-section">
      <div class="sb-section-title">In this section</div>
      <div class="sb-siblings">${sibItems}</div>
    </div>`);
  }

  // Helper: compute relative href from current file to target file
  const currentDir = graph?.currentFile?.replace(/[^/]*$/, '') || '';
  function relHref(targetFile: string): string {
    let href = targetFile.replace(/\.qmd\.md$/, '.html');
    if (currentDir && href.startsWith(currentDir)) {
      href = href.substring(currentDir.length);
    } else if (currentDir) {
      const upCount = currentDir.split('/').filter(Boolean).length;
      href = '../'.repeat(upCount) + href;
    }
    return href;
  }

  // Links to (outgoing)
  if (graph && graph.linksTo.length > 0) {
    const grouped: Record<string, typeof graph.linksTo> = {};
    for (const e of graph.linksTo) {
      (grouped[e.verb] ??= []).push(e);
    }
    let html = '';
    for (const [verb, edges] of Object.entries(grouped)) {
      html += `<div class="sb-edge-group"><span class="sb-edge-verb">${escapeHtml(verb)}</span>`;
      for (const e of edges) {
        if (e.id) {
          html += `<a href="#" class="sb-edge-item qmdc-ref" data-ref="${escapeHtml(e.id)}">${escapeHtml(e.label)} <span class="sb-edge-kind">${escapeHtml(e.kind)}</span></a>`;
        } else if (e.file) {
          html += `<a href="${escapeHtml(relHref(e.file))}" class="sb-edge-item">${escapeHtml(e.label)} <span class="sb-edge-kind">${escapeHtml(e.kind)}</span></a>`;
        } else {
          html += `<div class="sb-edge-item">${escapeHtml(e.label)} <span class="sb-edge-kind">${escapeHtml(e.kind)}</span></div>`;
        }
      }
      html += '</div>';
    }
    sections.push(`<div class="sb-section">
      <div class="sb-section-title">Links to</div>
      ${html}
    </div>`);
  }

  // Linked from (incoming)
  if (graph && graph.linkedFrom.length > 0) {
    const grouped: Record<string, typeof graph.linkedFrom> = {};
    for (const e of graph.linkedFrom) {
      (grouped[e.verb] ??= []).push(e);
    }
    let html = '';
    for (const [verb, edges] of Object.entries(grouped)) {
      html += `<div class="sb-edge-group"><span class="sb-edge-verb">${escapeHtml(verb)}</span>`;
      for (const e of edges) {
        if (e.id) {
          html += `<a href="#" class="sb-edge-item qmdc-ref" data-ref="${escapeHtml(e.id)}">${escapeHtml(e.label)} <span class="sb-edge-kind">${escapeHtml(e.kind)}</span></a>`;
        } else if (e.file) {
          html += `<a href="${escapeHtml(relHref(e.file))}" class="sb-edge-item">${escapeHtml(e.label)} <span class="sb-edge-kind">${escapeHtml(e.kind)}</span></a>`;
        } else {
          html += `<div class="sb-edge-item">${escapeHtml(e.label)} <span class="sb-edge-kind">${escapeHtml(e.kind)}</span></div>`;
        }
      }
      html += '</div>';
    }
    sections.push(`<div class="sb-section">
      <div class="sb-section-title">Linked from</div>
      ${html}
    </div>`);
  }

  if (sections.length === 0) return '';

  return `<aside class="qmdc-sidebar" id="qmdc-sidebar">
    <button class="sb-close" onclick="document.getElementById('qmdc-sidebar').classList.remove('sb-open')" aria-label="Close sidebar">✕</button>
    ${sections.join('\n')}
  </aside>`;
}

/** CSS styles for the preview — loaded from templates/preview-styles.css */
let _previewStyles: string | null = null;
export function getPreviewStyles(): string {
  if (_previewStyles) return _previewStyles;
  const stylesPath = path.join(__dirname, '..', 'templates', 'preview-styles.css');
  _previewStyles = fs.readFileSync(stylesPath, 'utf-8');
  return _previewStyles;
}

/** VS Code theme-aware overrides — loaded from templates/preview-styles-vscode.css */
let _previewStylesVscode: string | null = null;
function getPreviewStylesVscode(): string {
  if (_previewStylesVscode) return _previewStylesVscode;
  const stylesPath = path.join(__dirname, '..', 'templates', 'preview-styles-vscode.css');
  _previewStylesVscode = fs.readFileSync(stylesPath, 'utf-8');
  return _previewStylesVscode;
}


/**
 * The Mermaid enhancement script injected into the preview webview.
 *
 * SINGLE SOURCE OF TRUTH: the rendering + zoom/pan/toolbar logic lives in
 * `templates/qmdc-mermaid-core.js`, shared verbatim with the MkDocs SSG
 * (qmdc-mkdocs ships the same file). This module only reads that file and
 * prepends the VS Code host contract — see qmdc-mermaid-core.js for the globals.
 *
 * The webview is always dark, so we pin `__qmdcMermaidTheme = 'dark'` (unlike the
 * MkDocs site, which follows the Material palette). mermaid itself is loaded
 * separately as `<script src=mermaid.min.js>` before this runs; the core no-ops
 * if the global `mermaid` is absent.
 *
 * To keep the VS Code copy of the core in sync with the canonical one in
 * qmdc-mkdocs, run `make mermaid-sync` (a parity test guards against drift).
 */
let _mermaidCore: string | null = null;
function getMermaidCore(): string {
  if (_mermaidCore === null) {
    const corePath = path.join(__dirname, '..', 'templates', 'qmdc-mermaid-core.js');
    _mermaidCore = fs.readFileSync(corePath, 'utf-8');
  }
  return _mermaidCore;
}

/**
 * Build the inline enhancement script: the VS Code host contract (dark theme)
 * followed by the shared core. Exported so e2e tests can exercise it directly
 * (they load mermaid.min.js and this script manually rather than via the
 * file:// webview URI).
 */
export function getMermaidEnhanceScript(): string {
  return `window.__qmdcMermaidTheme = 'dark';\n` + getMermaidCore();
}

/**
 * Generate full HTML preview for a QMD.md document.
 *
 * @param content - Raw QMD.md content
 * @param queryExecutor - Optional executor for SQL table blocks (null = render placeholder)
 * @param documentUri - Document URI for query scoping
 * @param options - Additional options
 */
export async function generatePreviewHtml(
  content: string,
  queryExecutor: QueryExecutor | null,
  documentUri: string,
  options: { includeVscodeApi?: boolean; mermaidScript?: string; scrollToId?: string; resolveImageSrc?: (src: string) => string } = {}
): Promise<string> {
  const { includeVscodeApi = true, mermaidScript, scrollToId, resolveImageSrc } = options;
  const _t: Record<string, number> = { start: Date.now() };
  // Use delegated click handler instead of inline onclick to avoid XSS via ref IDs

  // 1. Process table blocks (SQL queries).
  //    Replace in reverse order so earlier match indices stay valid after splicing.
  const tableBlockRegex = /```table\n([\s\S]*?)```/g;
  let processedContent = content;
  const matches = [...content.matchAll(tableBlockRegex)];

  for (let i = matches.length - 1; i >= 0; i--) {
    const match = matches[i];
    const blockContent = match[1].trim();
    const { sql, scope } = parseBlockContent(blockContent);

    let tableHtml = '';
    if (sql && queryExecutor) {
      try {
        const result = await queryExecutor.executeQuery(sql, documentUri, scope);
        if (result?.success && result.columns && result.rows) {
          tableHtml = renderHtmlTable(result.columns, result.rows);
        } else {
          tableHtml = `<div class="error">Error: ${result?.error || 'Unknown error'}</div>`;
        }
      } catch (error) {
        tableHtml = `<div class="error">Error: ${error}</div>`;
      }
    } else if (sql) {
      tableHtml = `<div class="error">Query executor not available</div>`;
    } else {
      tableHtml = `<div class="error">No query found in block</div>`;
    }

    const start = match.index!;
    const end = start + match[0].length;
    processedContent = processedContent.slice(0, start) + tableHtml + processedContent.slice(end);
  }

  // 2. Transform QMD.md syntax (this converts mermaid fences to <div class="mermaid">)
  processedContent = transformQmdcSyntax(processedContent);

  // 2b. Extract mermaid divs before markdown processing — marked would mangle
  //     indented content and HTML entities inside them (e.g. <br/>, -->).
  //     Use HTML comments as placeholders so marked passes them through untouched.
  const mermaidDivs: string[] = [];
  processedContent = processedContent.replace(
    /<div class="mermaid">\n([\s\S]*?)\n<\/div>/g,
    (fullMatch) => {
      mermaidDivs.push(fullMatch);
      return `<!--MERMAID_PLACEHOLDER_${mermaidDivs.length - 1}-->`;
    }
  );

  // 3. Convert markdown to HTML
  _t.markdown0 = Date.now();
  const htmlContent = await marked(processedContent);

  // 3b. Restore mermaid divs after markdown processing
  const finalHtml = htmlContent.replace(
    /<!--MERMAID_PLACEHOLDER_(\d+)-->/g,
    (_, index) => mermaidDivs[parseInt(index)]
  );

  // 3c. Extract page TOC from rendered HTML
  _t.markdown1 = Date.now();
  const pageToc = extractPageToc(finalHtml);

  // 3d. Fetch graph context (if executor available)
  let graphCtx: GraphContext | null = null;
  if (queryExecutor) {
    graphCtx = await fetchGraphContext(queryExecutor, documentUri);
  }

  // 3d1. For short pages, inject navigable cards into the body so the page isn't a dead end.
  //       Shows sibling files as a grid, plus "linked from" items as a related section.
  let resolvedHtml = finalHtml;
  if (graphCtx) {
    const textLength = finalHtml.replace(/<[^>]+>/g, '').trim().length;
    const hasEnoughNav = graphCtx.siblings.length > 1 || graphCtx.linkedFrom.length > 0;
    if (textLength < 800 && hasEnoughNav) {
      const currentDir = graphCtx.currentFile.replace(/[^/]*$/, '');
      const relHref = (targetFile: string) => {
        let href = targetFile.replace(/\.qmd\.md$/, '.html');
        if (currentDir && href.startsWith(currentDir)) href = href.substring(currentDir.length);
        else if (currentDir) href = '../'.repeat(currentDir.split('/').filter(Boolean).length) + href;
        return href;
      };

      let extraHtml = '';

      // Sibling cards (grouped by subdirectory)
      if (graphCtx.siblings.length > 1) {
        const groups: Record<string, typeof graphCtx.siblings> = {};
        for (const s of graphCtx.siblings) {
          if (s.current) continue;
          const fileDir = s.file.replace(/[^/]*$/, '');
          let groupName = '';
          if (fileDir !== currentDir && fileDir.startsWith(currentDir)) {
            groupName = fileDir.substring(currentDir.length).replace(/\/$/, '');
          }
          (groups[groupName] ??= []).push(s);
        }
        for (const [group, items] of Object.entries(groups).sort()) {
          if (group) {
            const title = group.replace(/[-_]/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase());
            extraHtml += `<h2>${escapeHtml(title)}</h2>`;
          }
          extraHtml += '<div class="qmdc-index-grid">';
          for (const s of items) {
            if (s.id) {
              extraHtml += `<a href="#" class="qmdc-index-card qmdc-ref" data-ref="${escapeHtml(s.id)}">
                <span class="qmdc-index-title">${escapeHtml(s.label)}</span>
                <span class="qmdc-index-kind">${escapeHtml(s.kinds)}</span></a>`;
            } else {
              extraHtml += `<a href="${escapeHtml(relHref(s.file))}" class="qmdc-index-card">
                <span class="qmdc-index-title">${escapeHtml(s.label)}</span>
                <span class="qmdc-index-kind">${escapeHtml(s.kinds)}</span></a>`;
            }
          }
          extraHtml += '</div>';
        }
      }

      // Linked-from cards
      if (graphCtx.linkedFrom.length > 0) {
        extraHtml += '<h2>Related</h2><div class="qmdc-index-grid">';
        const seen = new Set<string>();
        for (const e of graphCtx.linkedFrom) {
          if (seen.has(e.file) || !e.file) continue;
          seen.add(e.file);
          if (e.id) {
            extraHtml += `<a href="#" class="qmdc-index-card qmdc-ref" data-ref="${escapeHtml(e.id)}">
              <span class="qmdc-index-title">${escapeHtml(e.label)}</span>
              <span class="qmdc-index-kind">${escapeHtml(e.verb)}</span></a>`;
          } else {
            extraHtml += `<a href="${escapeHtml(relHref(e.file))}" class="qmdc-index-card">
              <span class="qmdc-index-title">${escapeHtml(e.label)}</span>
              <span class="qmdc-index-kind">${escapeHtml(e.verb)}</span></a>`;
          }
        }
        extraHtml += '</div>';
      }

      if (extraHtml) resolvedHtml = finalHtml + extraHtml;
    }
  }

  // 3d2. Resolve [[#ref]] links to actual file URLs
  if (queryExecutor && graphCtx?.currentFile) {
    // Collect all ref IDs from the HTML
    const refIds = new Set<string>();
    const refRegex = /class="qmdc-ref"\s+data-ref="([^"]+)"/g;
    let refMatch;
    while ((refMatch = refRegex.exec(finalHtml)) !== null) {
      refIds.add(refMatch[1]);
    }
    if (refIds.size > 0) {
      // Build a map of refId -> file by querying with proper qualification.
      // Ref formats: "id", "namespace:id", "Kind:id", "namespace:Kind:id"
      // We query __id + __namespace to resolve correctly without ambiguity.
      const refFileMap: Record<string, string> = {};

      // Split refs into unqualified (just id) and qualified (has colons)
      const unqualified: string[] = [];
      const qualified: string[] = [];
      for (const refId of refIds) {
        if (refId.includes(':')) qualified.push(refId);
        else unqualified.push(refId);
      }

      // Batch-resolve unqualified refs: just match __id
      if (unqualified.length > 0) {
        const uqResult = await queryExecutor.executeQuery(
          `SELECT __id, __file FROM objects WHERE __id IN (${
            unqualified.map(id => `'${id.replace(/'/g, "''")}'`).join(',')
          })`,
          documentUri, 'workspace'
        );
        if (uqResult?.success && uqResult.rows) {
          for (const [id, file] of uqResult.rows) {
            if (id && file) refFileMap[id] = file;
          }
        }
      }

      // Resolve qualified refs: parse namespace/Kind/id and query precisely
      if (qualified.length > 0) {
        // Batch-query all possible bare IDs + namespaces in one go
        const bareIds = new Set<string>();
        for (const refId of qualified) {
          const parts = refId.split(':');
          bareIds.add(parts[parts.length - 1]); // last segment is always the id
        }
        const qResult = await queryExecutor.executeQuery(
          `SELECT __id, __namespace, __kind, __file FROM objects WHERE __id IN (${
            [...bareIds].map(id => `'${id.replace(/'/g, "''")}'`).join(',')
          })`,
          documentUri, 'workspace'
        );
        if (qResult?.success && qResult.rows) {
          // Index by namespace:id and Kind:id for precise matching
          const byNsId: Record<string, string> = {};
          const byKindId: Record<string, string> = {};
          const byId: Record<string, string> = {};
          for (const [id, ns, kind, file] of qResult.rows) {
            if (!id || !file) continue;
            byId[id] = file;
            if (ns) byNsId[`${ns}:${id}`] = file;
            if (kind) byKindId[`${kind}:${id}`] = file;
            if (ns && kind) byNsId[`${ns}:${kind}:${id}`] = file;
          }
          for (const refId of qualified) {
            // Try exact match in order: ns:Kind:id, ns:id, Kind:id, bare id
            refFileMap[refId] = byNsId[refId] || byKindId[refId] || byId[refId.split(':').pop()!] || '';
          }
        }
      }

      // Compute relative paths and rewrite hrefs
      const currentDir = graphCtx.currentFile.replace(/[^/]*$/, '');
      resolvedHtml = resolvedHtml.replace(
          /(<a\s+href=")#("\s+class="qmdc-ref"\s+data-ref=")([^"]+)(")/g,
          (_match, pre1, pre2, refId, post) => {
            const targetFile = refFileMap[refId];
            if (!targetFile) return `${pre1}#${pre2}${refId}${post}`;
            let href = targetFile.replace(/\.qmd\.md$/, '.html');
            // Add anchor to the object ID
            const bareId = refId.split(':').pop() || refId;
            href += '#' + bareId;
            // Make relative to current file
            if (currentDir && href.startsWith(currentDir)) {
              href = href.substring(currentDir.length);
            } else if (currentDir) {
              const upCount = currentDir.split('/').filter(Boolean).length;
              href = '../'.repeat(upCount) + href;
            }
            return `${pre1}${escapeHtml(href)}${pre2}${refId}${post}`;
          }
        );
    }
  }

  // 3e. Render sidebar
  const sidebarHtml = renderSidebar(pageToc, graphCtx);

  // 3e2. Build search index from all workspace objects (for previewer search)
  //       Cached because it doesn't depend on the current file and is expensive.
  let searchIndex: { id: string; label: string; kind: string; file: string; ns: string }[] = [];
  if (queryExecutor) {
    if (_searchIndexCache) {
      searchIndex = _searchIndexCache;
    } else {
      try {
        const searchResult = await queryExecutor.executeQuery(
          `SELECT __id, __label, __kind, __file, __namespace FROM objects WHERE substr(__kind,1,2) != '__' AND __label IS NOT NULL ORDER BY __label`,
          documentUri, 'workspace'
        );
        if (searchResult?.success && searchResult.rows) {
          searchIndex = searchResult.rows.map(([id, label, kind, file, ns]) => ({
            id: id || '', label: label || '', kind: kind || '', file: file || '', ns: ns || ''
          }));
          _searchIndexCache = searchIndex;
        }
      } catch { /* degrade gracefully */ }
    }
  }

  // 4. Build mermaid script tag.
  //    Load mermaid.min.js, then the shared enhancement core: it disables
  //    useMaxWidth (so diagrams render at natural, readable size) and wraps each
  //    diagram in a zoom/pan viewport. See getMermaidEnhanceScript / the shared
  //    templates/qmdc-mermaid-core.js.
  const mermaidTag = mermaidScript
    ? `<script src="${mermaidScript}"></script>\n  <script>${getMermaidEnhanceScript()}</script>`
    : '';

  // 5. Build navigation script (delegated click handler — no inline onclick)
  const navigateHandler = includeVscodeApi
    ? `const vscode = acquireVsCodeApi();
    function navigateToRef(refId) {
      vscode.postMessage({ type: 'navigateToRef', refId: refId });
    }
    function navigateBack() {
      vscode.postMessage({ type: 'navigateBack' });
    }
    function navigateForward() {
      vscode.postMessage({ type: 'navigateForward' });
    }`
    : `function navigateToRef(refId) {
      console.log('navigateToRef:', refId);
    }
    function navigateBack() {
      console.log('navigateBack');
    }
    function navigateForward() {
      console.log('navigateForward');
    }`;

  const vscodeScript = `${navigateHandler}
    document.addEventListener('click', function(e) {
      var ref = e.target.closest('.qmdc-ref');
      if (ref) {
        e.preventDefault();
        navigateToRef(ref.dataset.ref);
      }
    });
    // Mouse back button (button 3) and forward button (button 4)
    document.addEventListener('mouseup', function(e) {
      if (e.button === 3) { e.preventDefault(); navigateBack(); }
      if (e.button === 4) { e.preventDefault(); navigateForward(); }
    });
    // Keyboard: Alt+Left for back, Alt+Right for forward
    document.addEventListener('keydown', function(e) {
      if (e.altKey && e.key === 'ArrowLeft') { e.preventDefault(); navigateBack(); }
      if (e.altKey && e.key === 'ArrowRight') { e.preventDefault(); navigateForward(); }
    });`;

  // 6. Build scroll-to-anchor script (for navigation within preview)
  //    Use requestAnimationFrame to defer scroll until after layout is complete.
  //    VS Code webviews may not have final dimensions when inline scripts run.
  //    If mermaid is present, also scroll after mermaid finishes rendering.
  const scrollScript = scrollToId
    ? `(function() {
      function doScroll() {
        var target = document.getElementById(${JSON.stringify(scrollToId)});
        if (target) {
          var heading = target.closest('h1, h2, h3, h4, h5, h6') || target.parentElement;
          if (heading) { heading.scrollIntoView({ block: 'start' }); }
        }
      }
      // Double-rAF ensures layout is complete before scrolling
      requestAnimationFrame(function() { requestAnimationFrame(doScroll); });
      // If mermaid is loaded, scroll again after it finishes rendering (layout may shift)
      if (typeof mermaid !== 'undefined') {
        document.addEventListener('DOMContentLoaded', function() {
          setTimeout(doScroll, 300);
        });
      }
    })();`
    : '';

  // 7. Rewrite <img src> for local images so the webview can load them.
  //    Markdown `![](assets/x.png)` → `<img src="assets/x.png">`, but a webview
  //    can't resolve a relative/disk path — the host must map it through
  //    webview.asWebviewUri() (and add the dir to localResourceRoots). Non-local
  //    srcs (http(s)/data/already-webview) are left untouched.
  if (resolveImageSrc) {
    resolvedHtml = rewriteImageSources(resolvedHtml, resolveImageSrc);
  }

  return renderPreviewFromTemplate(resolvedHtml, sidebarHtml, searchIndex, {
    styles: getPreviewStyles() + '\n' + getPreviewStylesVscode(),
    mermaidTag,
    vscodeScript,
    scrollScript,
  });
}

/**
 * Rewrite local `<img src>` values through a resolver (e.g. webview.asWebviewUri).
 * Leaves absolute/remote sources (http(s):, data:, already-webview, #anchors) intact.
 * Exported for testing.
 */
export function rewriteImageSources(html: string, resolve: (src: string) => string): string {
  return html.replace(
    /(<img\b[^>]*?\bsrc=)(["'])(.*?)\2/gi,
    (full, pre, quote, src) => {
      if (/^(https?:|data:|vscode-webview:|vscode-resource:|#|\/\/)/i.test(src)) {
        return full;
      }
      return `${pre}${quote}${resolve(src)}${quote}`;
    }
  );
}

// ── Preview template rendering ─────────────────────────────────────────────

let _previewTemplate: string | null = null;
let _searchIndexCache: { id: string; label: string; kind: string; file: string; ns: string }[] | null = null;

/** Clear the search index cache (call when workspace changes) */
export function clearSearchIndexCache(): void {
  _searchIndexCache = null;
}

function loadPreviewTemplate(): string {
  if (_previewTemplate) return _previewTemplate;
  const templatePath = path.join(__dirname, '..', 'templates', 'preview-template.html');
  _previewTemplate = fs.readFileSync(templatePath, 'utf-8');
  return _previewTemplate;
}

function renderPreviewFromTemplate(
  contentHtml: string,
  sidebarHtml: string,
  searchIndex: { id: string; label: string; kind: string; file: string; ns: string }[],
  options: { styles: string; mermaidTag: string; vscodeScript: string; scrollScript: string },
): string {
  const template = loadPreviewTemplate();
  return template
    .replace('{{STYLES}}', options.styles)
    .replace('{{MERMAID_TAG}}', options.mermaidTag)
    .replace('{{CONTENT}}', contentHtml)
    .replace('{{SIDEBAR}}', sidebarHtml)
    .replace('{{VSCODE_SCRIPT}}', options.vscodeScript)
    .replace('{{SCROLL_SCRIPT}}', options.scrollScript)
    .replace('{{SEARCH_INDEX}}', JSON.stringify(searchIndex));
}

// ── Template-based rendering for static site generation ────────────────────
// (removed — superseded by the qmdc-mkdocs site generator)
