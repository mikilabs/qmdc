/**
 * Main QMDC parser
 */

import type Token from 'markdown-it/lib/token';
import yaml from 'js-yaml';
import { tokenize } from './tokenizer.js';
import {
  parseHeader,
  setRandomSeed,
  generateFallbackId,
  type HeaderData,
} from './parsers/header.js';
import { parseArrayItemsFromList, parseFieldsFromList, parseFieldValue } from './parsers/field.js';
import { BlockTree } from './block_tree.js';

// Output features
export const FEATURE_ID = 'id'; // Include __id even if auto-generated
export const FEATURE_KIND = 'kind'; // Include __kind even if system type
export const FEATURE_LABEL = 'label';
export const FEATURE_PARENT = 'parent';
export const FEATURE_TYPES = 'types';
export const FEATURE_SYNTAX = 'syntax';
export const FEATURE_LEVEL = 'level';
export const FEATURE_LINE = 'line';
export const FEATURE_EXPLICIT_ID = 'explicit_id';
export const FEATURE_REFERENCES = 'references'; // Include __references for LSP
export const FEATURE_POSITIONS = 'positions'; // Include __positions for LSP (field line/col)

export type OutputFormat = 'minimal' | 'standard' | 'full';

// Format presets
// minimal: pure data only, no metadata (id/kind only if explicitly set in document)
// standard: supports rebuild (includes all metadata except line)
// full: LSP support (includes line numbers and references)
export const FORMATS: Record<OutputFormat, Set<string>> = {
  minimal: new Set(),
  standard: new Set([
    FEATURE_ID,
    FEATURE_KIND,
    FEATURE_LABEL,
    FEATURE_PARENT,
    FEATURE_TYPES,
    FEATURE_SYNTAX,
    FEATURE_LEVEL,
    FEATURE_EXPLICIT_ID,
  ]),
  full: new Set([
    FEATURE_ID,
    FEATURE_KIND,
    FEATURE_LABEL,
    FEATURE_PARENT,
    FEATURE_TYPES,
    FEATURE_SYNTAX,
    FEATURE_LEVEL,
    FEATURE_EXPLICIT_ID,
    FEATURE_LINE,
    FEATURE_REFERENCES,
    FEATURE_POSITIONS,
  ]),
};

// Reference pattern for extracting [[...]] references
const REFERENCE_PATTERN = /\[\[([^\]]+)\]\]/g;

interface ParsedReference {
  target: string;
  type: string;
  line: number;
  start_col: number;
  end_col: number;
  raw: string;
}

interface CodeFenceInfo {
  lang: string;
  offset_line: number; // 0-based line offset within content
  length_lines: number; // number of lines including ``` markers
}

/**
 * Classify reference type based on content
 */
function classifyReference(inner: string): string {
  // Handle # prefix
  const content = inner.startsWith('#') ? inner.slice(1) : inner;

  // Check for crossfile references (contain / or # in middle)
  if (content.includes('/') || (!inner.startsWith('#') && content.includes('#'))) {
    return 'crossfile';
  }

  // Check for Kind:id or Kind.id format (first char is uppercase = Kind)
  // Or namespace:id format (first char is lowercase = namespace)
  if (content.includes(':') || content.includes('.')) {
    const sep = content.includes(':') ? ':' : '.';
    const parts = content.split(sep, 2);
    if (parts.length === 2) {
      const first = parts[0];
      // If first char is uppercase, assume Kind
      if (
        first &&
        first[0] &&
        first[0] === first[0].toUpperCase() &&
        first[0] !== first[0].toLowerCase()
      ) {
        return 'kind';
      } else {
        return 'namespace';
      }
    }
  }

  // hash_local vs local
  if (inner.startsWith('#')) {
    return 'hash_local';
  }
  return 'local';
}

/**
 * Extract references from text with positions
 */
/**
 * Check if position is inside backticks (inline code)
 */
export function isInsideBackticks(text: string, pos: number): boolean {
  let inBacktick = false;
  for (let i = 0; i < text.length && i < pos; i++) {
    if (text[i] === '`') {
      // Check for triple backticks (code fence) - treat entire line as code
      if (i + 2 < text.length && text[i + 1] === '`' && text[i + 2] === '`') {
        return true;
      }
      // Check for double backticks (``) - treat as single backtick pair
      if (i + 1 < text.length && text[i + 1] === '`') {
        // Skip both backticks and toggle state
        i++; // Skip second backtick
        inBacktick = !inBacktick;
      } else {
        inBacktick = !inBacktick;
      }
    }
  }
  return inBacktick;
}

function extractReferencesFromText(
  text: string,
  lineNum: number,
  colOffset: number = 0
): ParsedReference[] {
  const refs: ParsedReference[] = [];
  const pattern = new RegExp(REFERENCE_PATTERN.source, 'g');
  let match;

  while ((match = pattern.exec(text)) !== null) {
    // Skip references inside backticks (inline code)
    if (isInsideBackticks(text, match.index)) {
      continue;
    }

    const inner = match[1] ?? '';

    // Only references start with '#'
    // [[#id]], [[#ns:id]], [[#Kind.field]] - references
    // [[id]], [[id:Kind]], [[field:text]] - definitions (skip)
    if (!inner.startsWith('#')) {
      continue;
    }

    refs.push({
      target: inner,
      type: classifyReference(inner),
      line: lineNum,
      start_col: colOffset + match.index,
      end_col: colOffset + match.index + match[0].length,
      raw: match[0],
    });
  }

  return refs;
}

export interface QmdcObject {
  __id: string;
  __label?: string; // Optional for system types (__Document, __TextBlock)
  __kind?: string;
  __container?: string;
  [key: string]: unknown;
}

export type ParseResult = QmdcObject[];

export interface ParseOptions {
  randomSeed?: number;
  format?: OutputFormat;
  features?: Set<string>;
}

/**
 * Parse QMD.md to JSON
 */
export function parse(markdown: string, options: ParseOptions | number = {}): ParseResult {
  // Support legacy signature: parse(markdown, randomSeed)
  const opts: ParseOptions = typeof options === 'number' ? { randomSeed: options } : options;
  const { randomSeed = 666, format = 'standard', features } = opts;

  const activeFeatures = features ?? FORMATS[format];

  // Set random seed for deterministic fallback IDs
  setRandomSeed(randomSeed);

  const tokens: Token[] = tokenize(markdown);
  const blockTree = new BlockTree(markdown);
  const objects: Record<string, QmdcObject> = {};
  const duplicateObjects: QmdcObject[] = [];
  const firstSeenLines: Record<string, number> = {}; // Track true first line for each duplicate ID

  // Stack of [object_id, heading_level] for tracking nesting
  const objectStack: Array<[string, number]> = [];

  // Track pending array field from [[field: array]] heading
  let pendingArrayField: [string, string] | null = null; // [parent_id, field_name]

  // Track pending text field from [[field]] or [[field: text]] heading (for multiline text)
  // [parent_id, field_name, field_level, field_label]
  let pendingTextField: [string, string, number, string] | null = null;
  let pendingTextFieldStartLine: number | null = null; // Content start line for raw-slice

  // Track pending object array from [[field: [Kind]]] heading
  // [parent_id, field_name, array_kind, level]
  let pendingObjectArray: [string, string, string, number] | null = null;

  // Track pending YAML field from [[field: yaml]] heading
  // [parent_id, field_name, field_label]
  let pendingYamlField: [string, string, string] | null = null;

  // Track pending JSON field from [[field: json]] heading
  // [parent_id, field_name, field_label]
  let pendingJsonField: [string, string, string] | null = null;

  // Track last anchor for comments (field name or "__self")
  let commentAnchor: string = '__self';

  // Track text blocks for document structure
  const textBlocks: Array<{
    __id: string;
    __kind: string;
    content: string;
    __line?: number;
    __code_fences?: CodeFenceInfo[];
  }> = [];
  let textBlockCounter = 0;
  const contentOrder: string[] = []; // Order of top-level elements (text blocks and objects)
  let pendingTextBlockContent: string[] = [];
  let pendingTextBlockStarted = false;
  let pendingTextBlockLine = 0;
  let pendingTextBlockLevel = 0; // Level of the TextBlock heading
  let pendingCodeFences: CodeFenceInfo[] = [];

  // Track parsing errors (structured_in_textblock, invalid_field_key, etc.)
  interface ParsingError {
    __id: string;
    __kind: '__ParsingError';
    type: string;
    message?: string;
    reference?: string;
    key?: string;
    field?: string;
    field_type?: string;
    object?: string;
    definitions?: string[];
    line: number | null;
    __file?: string;
  }
  const parsingErrors: ParsingError[] = [];

  // Pre-compiled regexes for mixed_field_keys scanner (avoid re-creation in loop)
  const invalidFieldLikeRe = /^([^:]+):\s+(.*)$/;
  const validKeyRe = /^[a-zA-Z_][a-zA-Z0-9_]*$/;
  const fieldStrictRe = /^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$/;
  const backtickStripRe = /`[^`]+`/g;
  const boldStripRe = /\*\*([^*]*)\*\*/g;
  const italicStripRe = /\*([^*]*)\*/g;
  const strikethroughStripRe = /~~([^~]*)~~/g;

  function getCurrentObjectId(): string | null {
    const top = objectStack[objectStack.length - 1];
    return top ? top[0] : null;
  }

  function getHeadingLevel(tag: string): number {
    if (tag.startsWith('h') && tag.length >= 2) {
      const level = parseInt(tag.substring(1), 10);
      return isNaN(level) ? 0 : level;
    }
    return 0;
  }

  // Compiled once — used by bulletListHasFields and hasFieldsAfterHeading
  const qmdcFieldPattern = /^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*/;

  /**
   * Check if a bullet_list starting at `listOpenIdx` contains at least one
   * valid QMD.md field (- key: value where key matches [a-zA-Z_][a-zA-Z0-9_]*).
   * Scans ALL items in the list, not just the first.
   */
  function bulletListHasFields(listOpenIdx: number): boolean {
    let k = listOpenIdx + 1;
    while (k < tokens.length && tokens[k]?.type !== 'bullet_list_close') {
      if (tokens[k]?.type === 'inline') {
        if (qmdcFieldPattern.test(tokens[k]?.content || '')) {
          return true;
        }
      }
      k++;
    }
    return false;
  }

  function hasNestedStructuredHeadings(startIdx: number, currentLevel: number): boolean {
    /**
     * Look-ahead to check if there are nested headings with [[...]] at a deeper level.
     * Returns true if any heading at a deeper level contains [[...]] bracket syntax.
     * Stops at headings at same or higher level.
     */
    const bracketRe = /\[\[[^\]]+\]\]/;
    let j = startIdx + 3; // Skip heading_open, inline, heading_close
    while (j < tokens.length) {
      const tok = tokens[j];
      if (!tok) break;
      if (tok.type === 'heading_open') {
        const nextLevel = getHeadingLevel(tok.tag);
        if (nextLevel <= currentLevel) {
          return false;
        }
        // Check if the heading text contains [[...]]
        if (j + 1 < tokens.length && tokens[j + 1]?.type === 'inline') {
          const headingContent = tokens[j + 1]?.content || '';
          if (bracketRe.test(headingContent)) {
            return true;
          }
        }
        j += 3; // Skip heading_open, inline, heading_close
        continue;
      }
      j++;
    }
    return false;
  }

  function hasFieldsAfterHeading(startIdx: number): boolean {
    /**
     * Look-ahead to check if there are field lists DIRECTLY after a heading.
     * Returns true if bullet_list with valid fields found before any other heading.
     * Only checks immediate content, not nested sections.
     *
     * A valid field list must contain at least one item matching `- key: value`
     * where key is a valid QMD.md identifier (starts with letter or _, contains only
     * letters, digits, _).
     */

    let j = startIdx + 3; // Skip heading_open, inline, heading_close
    while (j < tokens.length) {
      const tok = tokens[j];
      if (!tok) break;
      if (tok.type === 'heading_open') {
        // Any heading ends the search - no direct fields found
        return false;
      } else if (tok.type === 'table_open') {
        // Tables are just text content, not fields
        // Continue looking for actual field lists
        j++;
        continue;
      } else if (tok.type === 'bullet_list_open') {
        return bulletListHasFields(j);
      } else if (tok.type === 'fence') {
        // Fences are just code blocks in text, not fields.
        // yaml/json fences are only fields when the heading
        // explicitly declares [[field: yaml]] or [[field: json]].
        j++;
        continue;
      }
      j++;
    }
    return false;
  }

  function resolveChildId(
    parentId: string,
    localId: string,
    arrField?: string
  ): { composedId: string; localId: string | null } {
    const parentKind = objects[parentId]?.__kind || '';
    if (parentKind === '__Workspace' || parentKind === '__Namespace') {
      return { composedId: localId, localId: null };
    }
    const parentFullId = objects[parentId]?.__id || parentId;
    let composedId: string;
    if (arrField) {
      if (arrField.includes('.')) {
        // Dot-ID in array field: use dot-ID directly as prefix
        composedId = `${arrField}.${localId}`;
      } else if (arrField === parentFullId) {
        // Top-level array: parent IS the array, skip extra field name
        composedId = `${parentFullId}.${localId}`;
      } else {
        composedId = `${parentFullId}.${arrField}.${localId}`;
      }
    } else {
      composedId = `${parentFullId}.${localId}`;
    }
    return { composedId, localId };
  }

  let i: number = 0;
  while (i < tokens.length) {
    const token: Token | undefined = tokens[i];
    if (!token) break;

    if (token.type === 'heading_open') {
      const level = getHeadingLevel(token.tag);
      const header: HeaderData | null = parseHeader(tokens, i);

      if (header) {
        // Get line number (1-based for LSP)
        const lineNum = token.map ? token.map[0] + 1 : undefined;

        // Emit multiple_definitions error if heading has 2+ [[...]]
        if (header.multipleDefinitions) {
          parsingErrors.push({
            __id: `error_${parsingErrors.length}`,
            __kind: '__ParsingError',
            type: 'multiple_definitions',
            definitions: header.multipleDefinitions,
            object: `[[#${header.id}]]`,
            line: lineNum ?? null,
          });
        }

        // Handle pending text field context - [[field: text]] collects all content including headings
        if (pendingTextField) {
          const [pfParentId, pfFieldName, pfLevel, pfFieldLabel] = pendingTextField;

          // If this heading is deeper than the text field AND has no explicit [[id]],
          // it's part of the text content, not a new object
          if (level > pfLevel && !header.hasExplicitId) {
            // Use raw-slice: scan forward to find end boundary, then extract raw content
            let scanIdx = i + 3; // After heading_open, inline, heading_close
            while (scanIdx < tokens.length) {
              const tok = tokens[scanIdx];
              if (!tok) break;
              if (tok.type === 'heading_open') {
                const nextLevel = getHeadingLevel(tok.tag);
                if (nextLevel <= pfLevel) {
                  break;
                }
                const nextHeader = parseHeader(tokens, scanIdx);
                if (nextHeader && nextHeader.hasExplicitId) {
                  break;
                }
                scanIdx += 3; // Skip heading_open, inline, heading_close
                continue;
              }
              scanIdx++;
            }

            // Extract raw content from heading start to end boundary
            const headingStartLine = token.map ? token.map[0] : 0;
            let endLine = headingStartLine + 1;
            if (scanIdx < tokens.length && tokens[scanIdx]?.map) {
              endLine = tokens[scanIdx]!.map![0];
            } else {
              endLine = blockTree.lineCount;
            }
            const rawText = blockTree.getLinesRaw(headingStartLine, endLine).trim();

            // Save text to parent object
            const pfParentObj = objects[pfParentId];
            if (pfParentObj) {
              const existing = pfParentObj[pfFieldName];
              const existingText = typeof existing === 'string' ? existing : '';
              pfParentObj[pfFieldName] = existingText ? existingText + '\n\n' + rawText : rawText;

              if (!pfParentObj.__types) {
                pfParentObj.__types = {};
              }
              (pfParentObj.__types as Record<string, string>)[pfFieldName] = 'string';
              if (!pfParentObj.__syntax) {
                pfParentObj.__syntax = {};
              }
              (pfParentObj.__syntax as Record<string, string>)[pfFieldName] = 'multiline_text';
              if (!pfParentObj.__labels) {
                pfParentObj.__labels = {};
              }
              (pfParentObj.__labels as Record<string, string>)[pfFieldName] = pfFieldLabel;
            }

            i = scanIdx;
            continue; // Don't process this heading as a new object
          }

          // Close the text field - heading at same/higher level or has [[id]]
          // Use raw-slice extraction for lossless content (preserves fenced blocks)
          const pfParentObj = objects[pfParentId];
          if (pfParentObj) {
            const endLine = token.map ? token.map[0] : 0;
            if (pendingTextFieldStartLine !== null && endLine > pendingTextFieldStartLine) {
              const rawText = blockTree.getLinesRaw(pendingTextFieldStartLine, endLine).trim();
              pfParentObj[pfFieldName] = rawText;
            } else {
              // Fallback: use existing content or empty
              const existing = pfParentObj[pfFieldName];
              if (existing === undefined) {
                pfParentObj[pfFieldName] = '';
              }
            }
            // Add __types for string field
            if (!pfParentObj.__types) {
              pfParentObj.__types = {};
            }
            (pfParentObj.__types as Record<string, string>)[pfFieldName] = 'string';

            // Add __syntax for multiline_text (for rebuild)
            if (!pfParentObj.__syntax) {
              pfParentObj.__syntax = {};
            }
            (pfParentObj.__syntax as Record<string, string>)[pfFieldName] = 'multiline_text';

            // Add __labels for field label
            if (!pfParentObj.__labels) {
              pfParentObj.__labels = {};
            }
            (pfParentObj.__labels as Record<string, string>)[pfFieldName] = pfFieldLabel;
          }
          pendingTextField = null;
          pendingTextFieldStartLine = null;
          // Update comment anchor so subsequent comments are "after" this field
          commentAnchor = pfFieldName;
        }

        // Pop objects from stack that are at same or deeper level
        let top = objectStack[objectStack.length - 1];
        while (top && top[1] >= level) {
          const poppedId = top[0];
          objectStack.pop();
          top = objectStack[objectStack.length - 1];
          // Reset commentAnchor to the field that references the popped object
          // on the new parent (so subsequent comments anchor correctly)
          if (top) {
            const newParentId = top[0];
            const newParentObj = objects[newParentId];
            if (newParentObj) {
              for (const [fk, fv] of Object.entries(newParentObj)) {
                if (fk.startsWith('__')) continue;
                if (
                  (typeof fv === 'string' && fv === `[[#${poppedId}]]`) ||
                  (Array.isArray(fv) && fv.includes(`[[#${poppedId}]]`))
                ) {
                  commentAnchor = fk;
                  break;
                }
              }
            }
          }
        }

        const parentId = getCurrentObjectId();

        if (pendingArrayField) {
          const [, pafFieldName] = pendingArrayField;
          commentAnchor = pafFieldName;
          pendingArrayField = null;
        }

        // Check if we're exiting an object array context
        if (pendingObjectArray) {
          const [, , , arrLevel] = pendingObjectArray;
          if (level <= arrLevel) {
            // Exiting object array context
            pendingObjectArray = null;
          }
        }

        // Check if this is a primitive array [[field: array]]
        if (header.fieldType === 'array' && parentId) {
          // This is a field array, not an object
          // Mark for next list to be parsed as array items
          pendingArrayField = [parentId, header.id];
        }
        // Check if this is an object array [[field: [Kind]]]
        else if (header.fieldType === 'object_array' && parentId) {
          const arrayKind = header.arrayKind || '';
          const parentObj = objects[parentId];
          if (parentObj) {
            // Initialize empty array in parent
            parentObj[header.id] = [];
            // Add __syntax for headers
            if (!parentObj.__syntax) {
              parentObj.__syntax = {};
            }
            (parentObj.__syntax as Record<string, string>)[header.id] = 'headers';
            // Store label
            if (!parentObj.__labels) {
              parentObj.__labels = {};
            }
            (parentObj.__labels as Record<string, string>)[header.id] = header.label;
          }
          // Mark context for following headings
          pendingObjectArray = [parentId, header.id, arrayKind, level];
        }
        // Top-level object array [[field: [Kind]]] without structural parent
        else if (header.fieldType === 'object_array' && !parentId) {
          const arrayKind = header.arrayKind || '';
          const objId = header.id;
          const obj: QmdcObject = {
            __id: objId,
            __kind: '__Object',
            __level: level,
            __line: lineNum,
            [objId]: [],
          } as any;
          if (header.label) {
            obj.__label = header.label;
          }
          if (!header.hasExplicitId) {
            (obj as any).__has_explicit_id = false;
          }
          (obj as any).__syntax = { [objId]: 'headers', __array_kind: arrayKind };
          (obj as any).__labels = { [objId]: header.label };
          objects[objId] = obj;
          objectStack.push([objId, level]);
          contentOrder.push(objId);
          // Mark context for following headings
          pendingObjectArray = [objId, objId, arrayKind, level];
          i += 3;
          continue;
        }
        // Check if this is a YAML field [[field: yaml]]
        else if (header.fieldType === 'yaml' && parentId) {
          // Mark for next fence to be parsed as YAML
          pendingYamlField = [parentId, header.id, header.label];
        }
        // Check if this is a JSON field [[field: json]]
        else if (header.fieldType === 'json' && parentId) {
          // Mark for next fence to be parsed as JSON
          pendingJsonField = [parentId, header.id, header.label];
        }
        // Check if this is a text field [[field: text]]
        else if (header.fieldType === 'text' && parentId) {
          // Mark for multiline text field, store the level for context tracking
          pendingTextField = [parentId, header.id, level, header.label];
          pendingTextFieldStartLine = tokens[i + 2]?.map ? tokens[i + 2]!.map![1] : null;
        }
        // Check if this is a map field [[field: map]]
        else if (header.fieldType === 'map' && parentId) {
          // Scan forward to find end boundary
          let scanIdx = i + 3;
          while (scanIdx < tokens.length) {
            const scanTok = tokens[scanIdx];
            if (scanTok && scanTok.type === 'heading_open') {
              const nextLevel = getHeadingLevel(scanTok.tag);
              if (nextLevel <= level) break;
            }
            scanIdx++;
          }
          // Find bullet_list_open between heading and end
          const mapData: Record<string, string> = {};
          let listScan = i + 3;
          let foundList = false;
          while (listScan < scanIdx) {
            const tok = tokens[listScan];
            if (tok?.type === 'bullet_list_open') {
              if (foundList) {
                // Second bullet list — invalid
                const errLine = tok.map ? tok.map[0] + 1 : 0;
                parsingErrors.push({
                  __id: `error_${parsingErrors.length}`,
                  __kind: '__ParsingError',
                  type: 'invalid_map_content',
                  field: header.id,
                  object: `[[#${parentId}]]`,
                  line: errLine,
                });
                listScan++;
                continue;
              }
              foundList = true;
              const [fields, , , invalidItems, nextI] = parseFieldsFromList(
                tokens,
                listScan,
                blockTree,
                { rawStrings: true }
              );
              Object.assign(mapData, fields as Record<string, string>);
              for (const inv of invalidItems) {
                parsingErrors.push({
                  __id: `error_${parsingErrors.length}`,
                  __kind: '__ParsingError',
                  type: 'invalid_map_entry',
                  field: header.id,
                  object: `[[#${parentId}]]`,
                  line: inv.line ?? 0,
                });
              }
              listScan = nextI;
              continue;
            } else if (
              tok?.type === 'paragraph_open' ||
              tok?.type === 'fence' ||
              tok?.type === 'code_block' ||
              tok?.type === 'ordered_list_open'
            ) {
              const errLine = tok.map ? tok.map[0] + 1 : 0;
              parsingErrors.push({
                __id: `error_${parsingErrors.length}`,
                __kind: '__ParsingError',
                type: 'invalid_map_content',
                field: header.id,
                object: `[[#${parentId}]]`,
                line: errLine,
              });
              if (tok.type === 'ordered_list_open') {
                while (listScan < scanIdx && tokens[listScan]?.type !== 'ordered_list_close') {
                  listScan++;
                }
              }
            }
            listScan++;
          }
          const mapParentObj = objects[parentId];
          if (mapParentObj) {
            mapParentObj[header.id] = mapData;
            if (!mapParentObj.__types) mapParentObj.__types = {};
            (mapParentObj.__types as Record<string, string>)[header.id] = 'map';
            if (!mapParentObj.__syntax) mapParentObj.__syntax = {};
            (mapParentObj.__syntax as Record<string, string>)[header.id] = 'map';
            if (!mapParentObj.__labels) mapParentObj.__labels = {};
            (mapParentObj.__labels as Record<string, string>)[header.id] = header.label;
          }
          commentAnchor = header.id;
          i = scanIdx;
          continue;
        }
        // Check if we're inside an object array context
        else if (pendingObjectArray && level > pendingObjectArray[3]) {
          const [arrParentId, arrField, arrKind] = pendingObjectArray;
          const arrParentFullId = objects[arrParentId]?.__id || arrParentId;
          // This is an element of the object array
          const resolved = resolveChildId(arrParentId, header.id, arrField);
          const objId = resolved.composedId;
          const obj: QmdcObject = {} as QmdcObject;
          obj.__id = objId;
          if (resolved.localId !== null) {
            obj.__local_id = resolved.localId;
          }
          obj.__kind = arrKind;
          obj.__parent = `[[#${arrParentFullId}]]`;
          obj.__parent_field = arrField;
          obj.__line = lineNum;
          if (header.label) {
            obj.__label = header.label;
          }
          // Add reference to parent's array
          const parentObj = objects[arrParentId];
          if (parentObj) {
            (parentObj[arrField] as string[]).push(`[[#${objId}]]`);
          }
          objects[objId] = obj;
          objectStack.push([objId, level]);
        } else if (parentId && !header.kind && header.hasExplicitId) {
          // Heading with [[id]] inside object but without :Kind
          // Could be text field or nested object - check next token

          // BR-16: Dot in nested child's explicit ID is an error
          if (header.id.includes('.')) {
            parsingErrors.push({
              __id: header.id,
              __kind: '__ParsingError',
              type: 'invalid_id_character',
              reference: `[[${header.id}]]`,
              line: lineNum ?? null,
            });
            // Skip heading tokens and content until next heading
            let skipIdx = i + 3;
            while (skipIdx < tokens.length) {
              if (tokens[skipIdx]?.type === 'heading_open') break;
              skipIdx++;
            }
            i = skipIdx;
            continue;
          }

          const nextIdx = i + 3; // After heading_open, inline, heading_close
          const nextToken = tokens[nextIdx];

          if (nextToken && nextToken.type === 'paragraph_open') {
            // Text content follows - this is a text field
            pendingTextField = [parentId, header.id, level, header.label];
            pendingTextFieldStartLine = tokens[i + 2]?.map ? tokens[i + 2]!.map![1] : null;
          } else if (nextToken && nextToken.type === 'bullet_list_open') {
            // List follows - check if it has fields (- key: value)
            const hasFields = hasFieldsAfterHeading(i);
            if (hasFields) {
              // Nested object with fields
              const parentFullId = objects[parentId]?.__id || parentId;
              const resolved = resolveChildId(parentId, header.id);
              const objId = resolved.composedId;
              const obj: QmdcObject = {} as QmdcObject;
              obj.__id = objId;
              if (resolved.localId !== null) {
                obj.__local_id = resolved.localId;
              }
              obj.__kind = '__Object';
              obj.__level = level;
              obj.__line = lineNum;
              if (header.label) {
                obj.__label = header.label;
              }
              obj.__parent = `[[#${parentFullId}]]`;
              obj.__parent_field = header.id;
              const parentObj = objects[parentId];
              if (parentObj) {
                parentObj[header.id] = `[[#${objId}]]`;
              }
              objects[objId] = obj;
              objectStack.push([objId, level]);
            } else {
              // List without fields - text field
              pendingTextField = [parentId, header.id, level, header.label];
              pendingTextFieldStartLine = tokens[i + 2]?.map ? tokens[i + 2]!.map![1] : null;
            }
          } else if (nextToken && nextToken.type === 'heading_open') {
            // Another heading follows - check if it's a child
            const nextLevel = getHeadingLevel(nextToken.tag);
            if (nextLevel > level) {
              // Child heading - this is a nested object
              const parentFullId = objects[parentId]?.__id || parentId;
              const resolved = resolveChildId(parentId, header.id);
              const objId = resolved.composedId;
              const obj: QmdcObject = {} as QmdcObject;
              obj.__id = objId;
              if (resolved.localId !== null) {
                obj.__local_id = resolved.localId;
              }
              obj.__kind = '__Object';
              obj.__level = level;
              obj.__line = lineNum;
              if (header.label) {
                obj.__label = header.label;
              }
              obj.__parent = `[[#${parentFullId}]]`;
              obj.__parent_field = header.id;
              const parentObj = objects[parentId];
              if (parentObj) {
                parentObj[header.id] = `[[#${objId}]]`;
              }
              objects[objId] = obj;
              objectStack.push([objId, level]);
            } else {
              // Same or higher level - empty text field
              pendingTextField = [parentId, header.id, level, header.label];
              pendingTextFieldStartLine = tokens[i + 2]?.map ? tokens[i + 2]!.map![1] : null;
            }
          } else {
            // Default: treat as text field
            pendingTextField = [parentId, header.id, level, header.label];
            pendingTextFieldStartLine = tokens[i + 2]?.map ? tokens[i + 2]!.map![1] : null;
          }
        } else if (parentId && !header.hasExplicitId) {
          // Heading WITHOUT [[id]] inside object = COMMENT (always)
          // Use raw-slice extraction instead of rebuilding from tokens
          const startLine = token.map ? token.map[0] : 0;
          let endLine = startLine + 1;
          let j = i + 3; // Skip heading_open, inline, heading_close

          // Scan to find end boundary
          while (j < tokens.length) {
            const tok = tokens[j];
            if (!tok) break;

            if (tok.type === 'heading_open') {
              const nextLevel = getHeadingLevel(tok.tag);
              if (nextLevel <= level) {
                // Same or higher level heading - stop before it
                endLine = tok.map ? tok.map[0] : endLine;
                break;
              }
              // Deeper heading - check for [[id: Kind]] or [[id]]
              const nextHeader = parseHeader(tokens, j);
              if (nextHeader) {
                const nextHasExplicit = nextHeader.hasExplicitId === true;
                const nextHasKind = !!nextHeader.kind;
                if (nextHasExplicit && nextHasKind) {
                  // Heading with Kind = nested object - stop before it
                  endLine = tok.map ? tok.map[0] : endLine;
                  break;
                } else if (nextHasExplicit) {
                  // ERROR: structured element inside comment/textblock
                  const errorId = `error_${parsingErrors.length}`;
                  let refPattern: string;
                  if (nextHeader.fieldType) {
                    refPattern = `[[${nextHeader.id}: ${nextHeader.fieldType}]]`;
                  } else {
                    refPattern = `[[${nextHeader.id}]]`;
                  }
                  const nextLine = tok.map ? tok.map[0] + 1 : null;
                  parsingErrors.push({
                    __id: errorId,
                    __kind: '__ParsingError',
                    type: 'structured_in_textblock',
                    reference: refPattern,
                    line: nextLine,
                  });
                  // Continue scanning - error heading is part of comment
                }
              }
              // Update endLine to include this heading's content
              if (tok.map) {
                endLine = tok.map[1];
              }
              j += 3; // Skip heading tokens
            } else {
              // Field-like bullet lists inside comment headings are
              // part of the comment content, not parent object fields.
              // Don't stop here — include them in the raw slice.
              // Update endLine based on token's map
              if (tok.map) {
                endLine = tok.map[1];
              }
              j++;
            }
          }

          // Extract raw markdown slice
          const rawComment = blockTree.getLinesRaw(startLine, endLine).trim();

          // Add to parent's __comments
          const parentObj = objects[parentId];
          if (parentObj) {
            if (!parentObj.__comments) {
              parentObj.__comments = [];
            }
            (parentObj.__comments as Array<{ after: string; content: string }>).push({
              after: commentAnchor,
              content: rawComment,
            });
          }

          i = j;
          continue;
        } else {
          // Check if this should be an object or a TextBlock
          // Heading without [[id]] and without direct fields = TextBlock
          // All other headings = objects
          const hasExplicit = header.hasExplicitId === true;
          const hasKind = !!header.kind;
          const hasFields = hasFieldsAfterHeading(i);

          // Heading without explicit id and without fields and without kind = TextBlock
          // Also check for nested structured headings (children with [[...]])
          // Only for H2+ headings — H1 headings are document titles
          const hasStructuredChildren = level >= 2 && hasNestedStructuredHeadings(i, level);
          const isTextBlock = !hasExplicit && !hasKind && !hasFields && !hasStructuredChildren;

          if (!isTextBlock) {
            // Check for explicit system type error
            // [[id: __Document]], [[id: __TextBlock]], [[id: __Object]] are not allowed
            // But __Workspace and __Namespace are valid kinds (explicit declaration in anchor files)
            const forbiddenExplicit = ['__Document', '__TextBlock', '__Object'];
            if (hasKind && forbiddenExplicit.includes(header.kind!)) {
              const errorId = header.id;
              const kindStr = header.kind!;
              const refPattern = `[[${header.id}: ${kindStr}]]`;
              parsingErrors.push({
                __id: errorId,
                __kind: '__ParsingError',
                type: 'explicit_system_type',
                reference: refPattern,
                line: lineNum ?? null,
              });
              // Skip heading tokens and non-heading content
              let skipIdx = i + 3;
              while (skipIdx < tokens.length) {
                const skipTok = tokens[skipIdx];
                if (skipTok && skipTok.type === 'heading_open') {
                  break;
                }
                skipIdx++;
              }
              i = skipIdx;
              continue;
            }

            // Check for structured_in_textblock error
            // Error occurs when:
            // 1. TextBlock has content (not just the heading)
            // 2. New heading is deeper than TextBlock level
            // 3. TextBlock started at level >= 2 (nested inside document)
            const textBlockHasContent =
              pendingTextBlockStarted && pendingTextBlockContent.length > 1;
            if (
              textBlockHasContent &&
              hasExplicit &&
              pendingTextBlockLevel >= 2 &&
              level > pendingTextBlockLevel
            ) {
              // ERROR: Cannot create structured element inside TextBlock
              const errorId = `error_${parsingErrors.length}`;
              let refPattern: string;
              if (hasKind) {
                refPattern = `[[${header.id}:${header.kind}]]`;
              } else if (header.fieldType) {
                refPattern = `[[${header.id}: ${header.fieldType}]]`;
              } else {
                refPattern = `[[${header.id}]]`;
              }
              parsingErrors.push({
                __id: errorId,
                __kind: '__ParsingError',
                type: 'structured_in_textblock',
                reference: refPattern,
                line: lineNum ?? null,
              });
              // Add heading to text block content and continue
              const headingText = '#'.repeat(level) + ' ' + header.label;
              pendingTextBlockContent.push('');
              pendingTextBlockContent.push(headingText);
              i += 3; // Skip heading tokens
              continue;
            }

            // Save any pending text block first
            if (pendingTextBlockStarted && pendingTextBlockContent.length > 0) {
              const textBlockId = `text_${textBlockCounter}`;
              textBlockCounter++;
              const tb: {
                __id: string;
                __kind: string;
                content: string;
                __line?: number;
                __code_fences?: CodeFenceInfo[];
              } = {
                __id: textBlockId,
                __kind: '__TextBlock',
                content: pendingTextBlockContent.join('\n\n'),
              };
              if (format === 'full') {
                tb.__line = pendingTextBlockLine;
                if (pendingCodeFences.length > 0) {
                  tb.__code_fences = [...pendingCodeFences];
                }
              }
              textBlocks.push(tb);
              contentOrder.push(textBlockId);
              pendingTextBlockContent = [];
              pendingTextBlockStarted = false;
              pendingTextBlockLevel = 0;
              pendingCodeFences = [];
            }

            // Create object
            const obj: QmdcObject = {
              __id: header.id,
              __label: header.label,
              __level: level, // For lossless rebuild
              __line: lineNum,
            };

            // Store if [[id]] was explicit (for lossless rebuild)
            if (!hasExplicit) {
              obj.__has_explicit_id = false;
            }

            if (!header.label) {
              delete obj.__label;
            }

            if (hasKind) {
              obj.__kind = header.kind;
            } else {
              obj.__kind = '__Object';
            }

            if (parentId) {
              const parentFullId = objects[parentId]?.__id || parentId;
              const resolved = resolveChildId(parentId, header.id);
              obj.__id = resolved.composedId;
              if (resolved.localId !== null) {
                obj.__local_id = resolved.localId;
              }
              obj.__parent = `[[#${parentFullId}]]`;
              obj.__parent_field = header.id;
              const parentObj = objects[parentId];
              if (parentObj) {
                parentObj[header.id] = `[[#${resolved.composedId}]]`;
              }
              objects[resolved.composedId] = obj;
              objectStack.push([resolved.composedId, level]);
            } else {
              // Top-level object - add to content order
              contentOrder.push(header.id);
              // Phase 3: Detect dot-ID parent declaration (BR-7)
              if (hasExplicit && header.id.includes('.')) {
                obj.__local_id = header.id;
              }
              // Detect true duplicates: if ID already exists with __line, it's a real duplicate
              // During parsing, let the new object overwrite (so fields parse correctly).
              // Track the true first line for error messages.
              const objIdToStore = obj.__id;
              if (objIdToStore in objects && '__line' in (objects[objIdToStore] ?? {})) {
                const existing = objects[objIdToStore]!;
                // Track the true first line (from first occurrence, not current occupant)
                if (!(objIdToStore in firstSeenLines)) {
                  firstSeenLines[objIdToStore] = existing.__line as number;
                }
                const firstLine = firstSeenLines[objIdToStore];
                // Save the OLD (current occupant) object — it will be output
                duplicateObjects.push(existing);
                // Emit __ParsingError for the NEW (incoming) occurrence
                parsingErrors.push({
                  __id: `__error_dup_${objIdToStore}`,
                  __kind: '__ParsingError',
                  type: 'duplicate_id',
                  message: `Duplicate ID '${objIdToStore}' (first defined on line ${firstLine})`,
                  object: `[[#${objIdToStore}]]`,
                  line: lineNum ?? null,
                });
              }
              objects[obj.__id] = obj;
              objectStack.push([obj.__id, level]);
            }

            // If heading had field_type but no parent, add __syntax metadata
            // and capture following content as __comments
            if (header.fieldType && !parentId) {
              // Emit dangling_field error
              parsingErrors.push({
                __id: `error_${parsingErrors.length}`,
                __kind: '__ParsingError',
                type: 'dangling_field',
                field: header.id,
                field_type: header.fieldType,
                object: `[[#${header.id}]]`,
                line: lineNum ?? null,
              });
              if (!obj.__syntax) {
                obj.__syntax = {};
              }
              const syntaxMap = obj.__syntax as Record<string, string>;
              if (header.fieldType === 'text') {
                syntaxMap[header.id] = 'multiline_text';
              } else if (header.fieldType === 'array') {
                syntaxMap[header.id] = 'markdown_list';
              } else if (header.fieldType === 'yaml') {
                syntaxMap[header.id] = 'yaml_object';
              } else if (header.fieldType === 'json') {
                syntaxMap[header.id] = 'json_object';
              } else if (header.fieldType === 'object_array') {
                syntaxMap[header.id] = 'headers';
                // Store array kind for heading rebuild
                const arrayKind = header.arrayKind || '';
                if (arrayKind) {
                  syntaxMap['__array_kind'] = arrayKind;
                }
              } else if (header.fieldType === 'map') {
                syntaxMap[header.id] = 'map';
              }

              // Capture content after heading as __comments (raw slice)
              if (
                header.fieldType === 'text' ||
                header.fieldType === 'array' ||
                header.fieldType === 'yaml' ||
                header.fieldType === 'json' ||
                header.fieldType === 'object_array' ||
                header.fieldType === 'map'
              ) {
                const contentStartLine = token.map ? token.map[1] : 0;
                let contentEndLine = blockTree.lineCount;
                let scanIdx = i + 3;
                while (scanIdx < tokens.length) {
                  const scanTok = tokens[scanIdx];
                  if (scanTok && scanTok.type === 'heading_open') {
                    const nextLevel = getHeadingLevel(scanTok.tag);
                    if (nextLevel <= level) {
                      contentEndLine = scanTok.map ? scanTok.map[0] : contentEndLine;
                      break;
                    }
                    // For object_array, capture ALL deeper content as comments
                    // (don't stop at child headings)
                    if (header.fieldType === 'object_array') {
                      scanIdx++;
                      continue;
                    }
                    // Also stop at deeper headings with explicit [[id]] that create structure
                    const deeperHeader = parseHeader(tokens, scanIdx);
                    if (deeperHeader && deeperHeader.hasExplicitId) {
                      const dhFt = deeperHeader.fieldType;
                      if (dhFt !== 'text' && dhFt !== 'array') {
                        contentEndLine = scanTok.map ? scanTok.map[0] : contentEndLine;
                        break;
                      }
                    }
                  }
                  scanIdx++;
                }
                const rawContent = blockTree.getLinesRaw(contentStartLine, contentEndLine).trim();
                if (rawContent) {
                  if (!obj.__comments) {
                    obj.__comments = [];
                  }
                  (obj.__comments as Array<{ after: string; content: string }>).push({
                    after: '__self',
                    content: rawContent,
                  });
                }
                i = scanIdx;
                continue;
              }
            }
          } else {
            // TextBlock - each heading without [[id]] starts a NEW text block
            // First, save any pending text block
            if (pendingTextBlockStarted && pendingTextBlockContent.length > 0) {
              const textBlockId = `text_${textBlockCounter}`;
              textBlockCounter++;
              const tb: {
                __id: string;
                __kind: string;
                content: string;
                __line?: number;
                __code_fences?: CodeFenceInfo[];
              } = {
                __id: textBlockId,
                __kind: '__TextBlock',
                content: pendingTextBlockContent.join('\n\n'),
              };
              if (format === 'full') {
                tb.__line = pendingTextBlockLine;
                if (pendingCodeFences.length > 0) {
                  tb.__code_fences = [...pendingCodeFences];
                }
              }
              textBlocks.push(tb);
              contentOrder.push(textBlockId);
              pendingTextBlockContent = [];
              pendingCodeFences = [];
            }

            // Start new text block with the heading
            const headingText = '#'.repeat(level) + ' ' + header.label;
            pendingTextBlockContent = [headingText];
            pendingTextBlockStarted = true;
            pendingTextBlockLine = lineNum ?? 0;
            pendingTextBlockLevel = level;
          }
        }

        // Reset comment anchor for new object
        commentAnchor = '__self';
      }

      i += 3; // Skip heading_open, inline, heading_close
    } else if (token.type === 'bullet_list_open') {
      if (pendingTextField) {
        // Collect bullet list as text for [[field: text]] section using raw-slice
        const [parentId, fieldName, fieldLevel, fieldLabel] = pendingTextField;
        const startLine = token.map ? token.map[0] : 0;
        let endLine = token.map ? token.map[1] : startLine + 1;
        let scanIdx = i + 1;

        // Scan to find end boundary
        while (scanIdx < tokens.length) {
          const tok = tokens[scanIdx];
          if (!tok) break;

          // Stop at next heading that ends the text field context
          if (tok.type === 'heading_open') {
            const nextLevel = getHeadingLevel(tok.tag);
            if (nextLevel <= fieldLevel) {
              endLine = tok.map ? tok.map[0] : endLine;
              break;
            }
            const nextHeader = parseHeader(tokens, scanIdx);
            if (nextHeader && nextHeader.hasExplicitId) {
              endLine = tok.map ? tok.map[0] : endLine;
              break;
            }
          }

          // Update endLine
          if (tok.map) {
            endLine = tok.map[1];
          }
          scanIdx++;
        }

        // Extract raw slice
        const rawText = blockTree.getLinesRaw(startLine, endLine).trim();
        const parentObj = objects[parentId];
        if (parentObj) {
          parentObj[fieldName] = rawText;

          // Add __types for string field
          if (!parentObj.__types) {
            parentObj.__types = {};
          }
          (parentObj.__types as Record<string, string>)[fieldName] = 'string';

          // Add __syntax for multiline_text
          if (!parentObj.__syntax) {
            parentObj.__syntax = {};
          }
          (parentObj.__syntax as Record<string, string>)[fieldName] = 'multiline_text';

          // Add __labels for field label
          if (!parentObj.__labels) {
            parentObj.__labels = {};
          }
          (parentObj.__labels as Record<string, string>)[fieldName] = fieldLabel;
        }

        pendingTextField = null;
        pendingTextFieldStartLine = null;
        i = scanIdx;
      } else if (pendingArrayField) {
        // This list is for a [[field: array]] section
        const [parentId, fieldName] = pendingArrayField;
        const [items, nextI] = parseArrayItemsFromList(tokens, i);
        const parentObj = objects[parentId];
        if (parentObj) {
          parentObj[fieldName] = items;

          // Add __syntax for markdown_list
          if (!parentObj.__syntax) {
            parentObj.__syntax = {};
          }
          (parentObj.__syntax as Record<string, string>)[fieldName] = 'markdown_list';
        }
        pendingArrayField = null;
        commentAnchor = fieldName;
        i = nextI;
      } else {
        const currentId = getCurrentObjectId();
        if (currentId) {
          // Parse fields from list
          const [
            fields,
            fieldTypes,
            fieldSyntax,
            invalidItems,
            nextI,
            rawValues,
            nestedSubitemsErrors,
          ] = parseFieldsFromList(tokens, i, blockTree);
          const currentObj = objects[currentId];
          if (currentObj) {
            // Check if any field keys already exist in the object.
            // If so, this bullet list is a DUPLICATE — treat it as
            // comment content instead of overwriting existing fields.
            const fieldKeys = Object.keys(fields).filter((k) => !k.startsWith('__'));
            const hasDuplicateKeys = fieldKeys.length > 0 && fieldKeys.some((k) => k in currentObj);
            if (hasDuplicateKeys) {
              // Treat entire bullet list as comment content
              if (token.map) {
                let scanJ = i + 1;
                while (scanJ < tokens.length && tokens[scanJ]?.type !== 'bullet_list_close') {
                  scanJ++;
                }
                const endLine =
                  scanJ < tokens.length && tokens[scanJ]?.map
                    ? tokens[scanJ]!.map![1]
                    : token.map[1];
                const rawList = blockTree.getLinesRaw(token.map[0], endLine).trim();
                if (rawList) {
                  if (!currentObj.__comments) {
                    currentObj.__comments = [];
                  }
                  (currentObj.__comments as Array<{ after: string; content: string }>).push({
                    after: commentAnchor,
                    content: rawList,
                  });
                }
                i = scanJ + 1;
              } else {
                i = nextI;
              }
              continue;
            }

            // No valid fields AND no invalid items — bullets without colons
            // (e.g. "- **bold** — text"). Treat entire list as comment content.
            if (fieldKeys.length === 0 && invalidItems.length === 0) {
              if (token.map) {
                let scanJ = i + 1;
                while (scanJ < tokens.length && tokens[scanJ]?.type !== 'bullet_list_close') {
                  scanJ++;
                }
                const endLine =
                  scanJ < tokens.length && tokens[scanJ]?.map
                    ? tokens[scanJ]!.map![1]
                    : token.map[1];
                const rawList = blockTree.getLinesRaw(token.map[0], endLine).trim();
                if (rawList) {
                  if (!currentObj.__comments) {
                    currentObj.__comments = [];
                  }
                  (currentObj.__comments as Array<{ after: string; content: string }>).push({
                    after: commentAnchor,
                    content: rawList,
                  });
                }
                i = scanJ + 1;
              } else {
                i = nextI;
              }
              continue;
            }

            Object.assign(currentObj, fields);

            // Store raw values as non-enumerable (invisible to JSON/comparison, visible to rebuild)
            if (Object.keys(rawValues).length > 0) {
              const existing =
                (Object.getOwnPropertyDescriptor(currentObj, '__raw_values')?.value as Record<
                  string,
                  string
                >) || {};
              Object.defineProperty(currentObj, '__raw_values', {
                value: { ...existing, ...rawValues },
                enumerable: false,
                configurable: true,
              });
            }

            // Merge __types (don't overwrite)
            if (Object.keys(fieldTypes).length > 0) {
              if (!currentObj.__types) {
                currentObj.__types = {};
              }
              Object.assign(currentObj.__types as Record<string, string>, fieldTypes);
            }

            // Merge __syntax (don't overwrite)
            if (Object.keys(fieldSyntax).length > 0) {
              if (!currentObj.__syntax) {
                currentObj.__syntax = {};
              }
              Object.assign(currentObj.__syntax as Record<string, string>, fieldSyntax);
            }

            // Handle invalid field items — treat as comment text, not errors
            if (invalidItems.length > 0) {
              if (!currentObj.__comments) {
                currentObj.__comments = [];
              }
              const existingComments = currentObj.__comments as Array<{
                after: string;
                content: string;
              }>;
              // Group invalid items by their "after" anchor
              let hasNewInvalid = false;
              for (const inv of invalidItems) {
                const invAfter = inv.after || '__self';
                const alreadyCaptured = existingComments.some(
                  (c) => c.content === inv.content || c.content.includes(inv.content)
                );
                if (!alreadyCaptured) {
                  // Plain non-field items (empty key) use \n join; invalid-key items use \n\n
                  const joinSep = inv.key === '' ? '\n' : '\n\n';
                  // Try to merge with existing comment with same anchor
                  const existing = existingComments.find((c) => c.after === invAfter);
                  if (existing) {
                    existing.content = existing.content + joinSep + inv.content;
                  } else {
                    existingComments.push({
                      after: invAfter,
                      content: inv.content,
                    });
                  }
                  // Only count as "invalid" for mixed_field_keys if it has an actual invalid key
                  if (inv.key !== '') {
                    hasNewInvalid = true;
                  }
                }
              }

              // mixed_field_keys error: valid + invalid keys in same list
              // Only when invalid items are genuinely mixed with valid fields
              if (Object.keys(fields).length > 0 && hasNewInvalid) {
                const errorLine = invalidItems[0]?.line ?? 0;
                parsingErrors.push({
                  __id: `error_${parsingErrors.length}`,
                  __kind: '__ParsingError',
                  type: 'mixed_field_keys',
                  object: `[[#${currentId}]]`,
                  line: errorLine,
                });
              }
            }

            // nested_subitems errors
            for (const nsErr of nestedSubitemsErrors) {
              parsingErrors.push({
                __id: `error_${parsingErrors.length}`,
                __kind: '__ParsingError',
                type: 'nested_subitems',
                field: nsErr.key,
                object: `[[#${currentId}]]`,
                line: nsErr.line,
              });
            }

            // Update comment anchor to last field
            const fieldNames = Object.keys(fields);
            if (fieldNames.length > 0) {
              commentAnchor = fieldNames[fieldNames.length - 1]!;
            }
          }
          i = nextI;
        } else if (pendingTextBlockStarted) {
          // Collect bullet list as markdown text for TextBlock
          const listItems: string[] = [];
          i++; // Skip bullet_list_open
          while (i < tokens.length && tokens[i]?.type !== 'bullet_list_close') {
            if (tokens[i]?.type === 'inline') {
              listItems.push('- ' + (tokens[i]?.content || ''));
            }
            i++;
          }
          if (i < tokens.length && tokens[i]?.type === 'bullet_list_close') {
            i++;
          }
          if (listItems.length > 0) {
            pendingTextBlockContent.push(listItems.join('\n'));
          }
        } else {
          i++;
        }
      }
    } else if (token.type === 'ordered_list_open') {
      if (pendingArrayField) {
        // Ordered lists are forbidden in array fields (rule_no_ordered_list_array).
        // Emit error, keep array empty, preserve content in __comments.
        const [parentId, fieldName] = pendingArrayField;
        const parentObj = objects[parentId];

        // Initialize empty array and syntax (normally done by parseArrayItemsFromList)
        if (parentObj) {
          parentObj[fieldName] = [];
          if (!parentObj.__syntax) {
            parentObj.__syntax = {};
          }
          (parentObj.__syntax as Record<string, string>)[fieldName] = 'markdown_list';
        }

        // Capture raw content as __comments for lossless round-trip
        let scanJ = i + 1;
        while (scanJ < tokens.length && tokens[scanJ]?.type !== 'ordered_list_close') {
          scanJ++;
        }
        if (parentObj && token.map) {
          const rawEnd =
            scanJ < tokens.length && tokens[scanJ]?.map ? tokens[scanJ]!.map![1] : token.map[1];
          const rawList = blockTree.getLinesRaw(token.map[0], rawEnd).trim();
          if (rawList) {
            if (!parentObj.__comments) {
              parentObj.__comments = [];
            }
            const existingComments = parentObj.__comments as Array<{
              after: string;
              content: string;
            }>;
            const existing = existingComments.find((c) => c.after === fieldName);
            if (existing) {
              existing.content = existing.content + '\n\n' + rawList;
            } else {
              existingComments.push({
                after: fieldName,
                content: rawList,
              });
            }
          }
        }

        // Emit ordered_list_in_array error
        const errorLine = token.map ? token.map[0] + 1 : null;
        parsingErrors.push({
          __id: `error_${parsingErrors.length}`,
          __kind: '__ParsingError',
          type: 'ordered_list_in_array',
          field: fieldName,
          object: `[[#${parentId}]]`,
          line: errorLine,
        });

        pendingArrayField = null;
        commentAnchor = fieldName;
        i = scanJ + 1; // skip past ordered_list_close
      } else if (pendingTextField) {
        // Collect ordered list as text for [[field: text]] section
        const [parentId, fieldName, _fieldLevel, fieldLabel] = pendingTextField;
        const textParts: string[] = [];
        let itemNum = 1;

        // Collect all list items as text
        while (i < tokens.length && tokens[i]?.type !== 'ordered_list_close') {
          const tok = tokens[i];
          if (tok?.type === 'inline' && tok.content) {
            textParts.push(`${itemNum}. ${tok.content}`);
            itemNum++;
          }
          i++;
        }
        i++; // skip ordered_list_close

        // Check if there's more content after the list (paragraphs until next heading)
        const moreText: string[] = [];
        while (i < tokens.length) {
          const tok = tokens[i];
          if (!tok) break;
          if (tok.type === 'heading_open') {
            break;
          }
          if (tok.type === 'inline' && tok.content) {
            moreText.push(tok.content);
          }
          i++;
        }

        // Combine all text
        let allText = textParts.join('\n');
        if (moreText.length > 0) {
          allText += '\n\n' + moreText.join('\n\n');
        }

        const parentObj = objects[parentId];
        if (parentObj) {
          parentObj[fieldName] = allText;

          // Add __types for string field
          if (!parentObj.__types) {
            parentObj.__types = {};
          }
          (parentObj.__types as Record<string, string>)[fieldName] = 'string';

          // Add __syntax for multiline_text
          if (!parentObj.__syntax) {
            parentObj.__syntax = {};
          }
          (parentObj.__syntax as Record<string, string>)[fieldName] = 'multiline_text';

          // Add __labels for field label
          if (!parentObj.__labels) {
            parentObj.__labels = {};
          }
          (parentObj.__labels as Record<string, string>)[fieldName] = fieldLabel;
        }

        pendingTextField = null;
        pendingTextFieldStartLine = null;
      } else if (!pendingTextBlockStarted) {
        // Ordered list as comment inside an object (e.g. trailing ordered list after array)
        const currentId = getCurrentObjectId();
        if (currentId && commentAnchor) {
          // Capture as single merged comment using raw slice
          if (token.map) {
            let scanJ = i + 1;
            while (scanJ < tokens.length && tokens[scanJ]?.type !== 'ordered_list_close') {
              scanJ++;
            }
            const endLine =
              scanJ < tokens.length && tokens[scanJ]?.map ? tokens[scanJ]!.map![1] : token.map[1];
            const rawList = blockTree.getLinesRaw(token.map[0], endLine).trim();
            if (rawList) {
              const obj = objects[currentId];
              if (obj) {
                if (!obj.__comments) {
                  obj.__comments = [];
                }
                (obj.__comments as Array<{ after: string; content: string }>).push({
                  after: commentAnchor,
                  content: rawList,
                });
              }
            }
            i = scanJ + 1;
          } else {
            const listItems: string[] = [];
            let itemNum = 1;
            i++; // skip ordered_list_open
            while (i < tokens.length && tokens[i]?.type !== 'ordered_list_close') {
              if (tokens[i]?.type === 'inline') {
                listItems.push(`${itemNum}. ` + (tokens[i]?.content || ''));
                itemNum++;
              }
              i++;
            }
            i++; // skip ordered_list_close
            if (listItems.length > 0) {
              const obj = objects[currentId];
              if (obj) {
                if (!obj.__comments) {
                  obj.__comments = [];
                }
                (obj.__comments as Array<{ after: string; content: string }>).push({
                  after: commentAnchor,
                  content: listItems.join('\n'),
                });
              }
            }
          }
        } else {
          i++;
        }
      } else if (pendingTextBlockStarted) {
        // Collect ordered list as raw markdown text for TextBlock (preserves sub-items)
        if (token.map) {
          let scanJ = i + 1;
          while (scanJ < tokens.length && tokens[scanJ]?.type !== 'ordered_list_close') {
            scanJ++;
          }
          const endLine =
            scanJ < tokens.length && tokens[scanJ]?.map ? tokens[scanJ]!.map![1] : token.map[1];
          const rawList = blockTree.getLinesRaw(token.map[0], endLine).trim();
          if (rawList) {
            pendingTextBlockContent.push(rawList);
          }
          i = scanJ + 1;
        } else {
          const listItems: string[] = [];
          let itemNum = 1;
          i++; // Skip ordered_list_open
          while (i < tokens.length && tokens[i]?.type !== 'ordered_list_close') {
            if (tokens[i]?.type === 'inline') {
              listItems.push(`${itemNum}. ` + (tokens[i]?.content || ''));
              itemNum++;
            }
            i++;
          }
          if (i < tokens.length && tokens[i]?.type === 'ordered_list_close') {
            i++;
          }
          if (listItems.length > 0) {
            pendingTextBlockContent.push(listItems.join('\n'));
          }
        }
      } else {
        i++;
      }
    } else if (token.type === 'table_open' && pendingTextBlockStarted) {
      // Collect table as raw markdown text for TextBlock
      if (token.map) {
        let scanJ = i + 1;
        while (scanJ < tokens.length && tokens[scanJ]?.type !== 'table_close') {
          scanJ++;
        }
        const endLine =
          scanJ < tokens.length && tokens[scanJ]?.map ? tokens[scanJ]!.map![1] : token.map[1];
        const rawTable = blockTree.getLinesRaw(token.map[0], endLine).trim();
        if (rawTable) {
          pendingTextBlockContent.push(rawTable);
        }
        i = scanJ + 1;
      } else {
        i++;
      }
    } else if (token.type === 'table_open' && pendingTextField) {
      // Table inside [[field:text]] - use raw-slice extraction to preserve formatting
      const [tfParentId, tfFieldName, tfFieldLevel, tfFieldLabel] = pendingTextField;
      const startLine = token.map ? token.map[0] : 0;
      let endLine = token.map ? token.map[1] : startLine + 1;
      let scanIdx = i + 1;

      // Scan to find end boundary - collect all content until next heading or end of text field
      while (scanIdx < tokens.length) {
        const tok = tokens[scanIdx];
        if (!tok) break;

        // Stop at next heading that ends the text field context
        if (tok.type === 'heading_open') {
          const nextLevel = getHeadingLevel(tok.tag);
          if (nextLevel <= tfFieldLevel) {
            endLine = tok.map ? tok.map[0] : endLine;
            break;
          }
          const nextHeader = parseHeader(tokens, scanIdx);
          if (nextHeader && nextHeader.hasExplicitId) {
            endLine = tok.map ? tok.map[0] : endLine;
            break;
          }
        }

        // Update endLine
        if (tok.map) {
          endLine = tok.map[1];
        }
        scanIdx++;
      }

      // Extract raw slice
      const rawText = blockTree.getLinesRaw(startLine, endLine).trim();
      const tfParentObj = objects[tfParentId];
      if (tfParentObj && rawText) {
        const existing = tfParentObj[tfFieldName];
        const existingText = typeof existing === 'string' ? existing : '';
        tfParentObj[tfFieldName] = existingText ? existingText + '\n\n' + rawText : rawText;

        // Add __types for string field
        if (!tfParentObj.__types) {
          tfParentObj.__types = {};
        }
        (tfParentObj.__types as Record<string, string>)[tfFieldName] = 'string';

        // Add __syntax for multiline_text
        if (!tfParentObj.__syntax) {
          tfParentObj.__syntax = {};
        }
        (tfParentObj.__syntax as Record<string, string>)[tfFieldName] = 'multiline_text';

        // Add __labels for field label
        if (!tfParentObj.__labels) {
          tfParentObj.__labels = {};
        }
        (tfParentObj.__labels as Record<string, string>)[tfFieldName] = tfFieldLabel;
      }

      i = scanIdx;
    } else if (token.type === 'table_open' && pendingObjectArray) {
      // Table after [[field: [Kind]]] heading
      const [arrParentId, arrField, arrKind] = pendingObjectArray;
      const parentObj = objects[arrParentId];

      if (parentObj) {
        // Change syntax from "headers" to "table"
        if (parentObj.__syntax) {
          (parentObj.__syntax as Record<string, string>)[arrField] = 'table';
        }

        // Add __types for array field
        if (!parentObj.__types) {
          parentObj.__types = {};
        }
        (parentObj.__types as Record<string, string>)[arrField] = 'array';
      }

      // Parse table
      const columnNames: string[] = [];
      const rows: string[][] = [];
      i++; // skip table_open

      // Parse header row (thead)
      while (i < tokens.length && tokens[i]?.type !== 'thead_close') {
        if (tokens[i]?.type === 'inline') {
          columnNames.push(tokens[i]?.content || '');
        }
        i++;
      }
      i++; // skip thead_close

      // Parse body rows (tbody)
      let currentRow: string[] = [];
      while (i < tokens.length && tokens[i]?.type !== 'table_close') {
        if (tokens[i]?.type === 'tr_open') {
          currentRow = [];
        } else if (tokens[i]?.type === 'inline') {
          currentRow.push(tokens[i]?.content || '');
        } else if (tokens[i]?.type === 'tr_close') {
          rows.push(currentRow);
        }
        i++;
      }
      i++; // skip table_close

      // Create objects from rows
      const arrParentFullId = objects[arrParentId]?.__id || arrParentId;
      for (let rowIdx = 0; rowIdx < rows.length; rowIdx++) {
        const row = rows[rowIdx];
        if (!row) continue;

        const localId = `${arrField}_${rowIdx}`;
        const resolved = resolveChildId(arrParentId, localId, arrField);
        const objId = resolved.composedId;
        const obj: QmdcObject = {} as QmdcObject;
        obj.__id = objId;
        if (resolved.localId !== null) {
          obj.__local_id = resolved.localId;
        }
        obj.__label = '';
        obj.__kind = arrKind;
        obj.__parent = `[[#${arrParentFullId}]]`;
        obj.__parent_field = arrField;
        const fieldTypes: Record<string, string> = {};

        for (let colIdx = 0; colIdx < columnNames.length; colIdx++) {
          const colName = columnNames[colIdx];
          if (colName && colIdx < row.length) {
            const valueStr = row[colIdx] || '';
            const [value, typeName] = parseFieldValue(valueStr);
            obj[colName] = value;
            fieldTypes[colName] = typeName;

            // Set label from first column
            if (colIdx === 0 && typeof value === 'string') {
              obj.__label = value;
            }
          }
        }

        if (Object.keys(fieldTypes).length > 0) {
          obj.__types = fieldTypes;
        }

        objects[objId] = obj;
        if (parentObj) {
          (parentObj[arrField] as string[]).push(`[[#${objId}]]`);
        }
      }

      pendingObjectArray = null;
    } else if (token.type === 'fence' && pendingYamlField) {
      // YAML fence after [[field: yaml]] heading
      const [yamlParentId, yamlFieldName, yamlFieldLabel] = pendingYamlField;
      const yamlContent = token.content || '';
      const parentObj = objects[yamlParentId];

      if (parentObj) {
        // Parse YAML using js-yaml
        let yamlData: unknown;
        try {
          yamlData = yaml.load(yamlContent);
        } catch {
          yamlData = yamlContent; // Fallback to raw string
        }

        parentObj[yamlFieldName] = yamlData;

        // Add __syntax
        if (!parentObj.__syntax) {
          parentObj.__syntax = {};
        }
        (parentObj.__syntax as Record<string, string>)[yamlFieldName] = 'yaml_object';

        // Add __labels
        if (!parentObj.__labels) {
          parentObj.__labels = {};
        }
        (parentObj.__labels as Record<string, string>)[yamlFieldName] = yamlFieldLabel;
      }

      pendingYamlField = null;
      i++;
    } else if (token.type === 'fence' && pendingJsonField) {
      // JSON fence after [[field: json]] heading
      const [jsonParentId, jsonFieldName, jsonFieldLabel] = pendingJsonField;
      const jsonContent = token.content || '';
      const parentObj = objects[jsonParentId];

      if (parentObj) {
        // Parse JSON
        let jsonData: unknown;
        try {
          jsonData = JSON.parse(jsonContent);
        } catch {
          jsonData = jsonContent; // Fallback to raw string
        }

        parentObj[jsonFieldName] = jsonData;

        // Add __syntax
        if (!parentObj.__syntax) {
          parentObj.__syntax = {};
        }
        (parentObj.__syntax as Record<string, string>)[jsonFieldName] = 'json_object';

        // Add __labels
        if (!parentObj.__labels) {
          parentObj.__labels = {};
        }
        (parentObj.__labels as Record<string, string>)[jsonFieldName] = jsonFieldLabel;
      }

      pendingJsonField = null;
      i++;
    } else if (token.type === 'fence' && pendingTextField) {
      // Code fence inside text field - use raw-slice for lossless content
      const [parentId, fieldName, _fieldLevel, fieldLabel] = pendingTextField;

      let fenceText: string;
      if (token.map) {
        fenceText = blockTree.getLinesRaw(token.map[0], token.map[1]).trim();
      } else {
        const lang = token.info || '';
        const fenceContent = (token.content || '').replace(/\n$/, '');
        fenceText = lang
          ? `\`\`\`${lang}\n${fenceContent}\n\`\`\``
          : `\`\`\`\n${fenceContent}\n\`\`\``;
      }

      const parentObj = objects[parentId];
      if (parentObj) {
        if (!(fieldName in parentObj)) {
          parentObj[fieldName] = '';
        }
        const existing = parentObj[fieldName];
        const existingText = typeof existing === 'string' ? existing : '';
        parentObj[fieldName] = existingText ? existingText + '\n\n' + fenceText : fenceText;

        if (!parentObj.__types) {
          parentObj.__types = {};
        }
        (parentObj.__types as Record<string, string>)[fieldName] = 'string';
        if (!parentObj.__syntax) {
          parentObj.__syntax = {};
        }
        (parentObj.__syntax as Record<string, string>)[fieldName] = 'multiline_text';
        if (!parentObj.__labels) {
          parentObj.__labels = {};
        }
        (parentObj.__labels as Record<string, string>)[fieldName] = fieldLabel;
      }

      i++;
    } else if (
      (token.type === 'bullet_list_open' || token.type === 'ordered_list_open') &&
      pendingTextField
    ) {
      // List inside text field - use raw-slice extraction
      const [parentId, fieldName, fieldLevel, fieldLabel] = pendingTextField;
      const startLine = token.map ? token.map[0] : 0;
      let endLine = token.map ? token.map[1] : startLine + 1;
      let scanIdx = i + 1;

      // Scan to find end boundary
      while (scanIdx < tokens.length) {
        const tok = tokens[scanIdx];
        if (!tok) break;

        // Stop at next heading that ends the text field context
        if (tok.type === 'heading_open') {
          const nextLevel = getHeadingLevel(tok.tag);
          if (nextLevel <= fieldLevel) {
            endLine = tok.map ? tok.map[0] : endLine;
            break;
          }
          const nextHeader = parseHeader(tokens, scanIdx);
          if (nextHeader && nextHeader.hasExplicitId) {
            endLine = tok.map ? tok.map[0] : endLine;
            break;
          }
        }

        // Update endLine
        if (tok.map) {
          endLine = tok.map[1];
        }
        scanIdx++;
      }

      // Extract raw slice
      const rawText = blockTree.getLinesRaw(startLine, endLine).trim();
      const parentObj = objects[parentId];
      if (parentObj) {
        const existing = parentObj[fieldName];
        const existingText = typeof existing === 'string' ? existing : '';
        parentObj[fieldName] = existingText ? existingText + '\n\n' + rawText : rawText;

        if (!parentObj.__types) {
          parentObj.__types = {};
        }
        (parentObj.__types as Record<string, string>)[fieldName] = 'string';

        if (!parentObj.__syntax) {
          parentObj.__syntax = {};
        }
        (parentObj.__syntax as Record<string, string>)[fieldName] = 'multiline_text';

        if (!parentObj.__labels) {
          parentObj.__labels = {};
        }
        (parentObj.__labels as Record<string, string>)[fieldName] = fieldLabel;
      }

      i = scanIdx;
    } else if (token.type === 'paragraph_open' && pendingTextField) {
      // Collect text for multiline text field using raw-slice extraction
      const [parentId, fieldName, fieldLevel, fieldLabel] = pendingTextField;
      const startLine = token.map ? token.map[0] : 0;
      let endLine = token.map ? token.map[1] : startLine + 1;
      let scanIdx = i + 1; // After paragraph_open

      // Scan to find end boundary
      while (scanIdx < tokens.length) {
        const tok = tokens[scanIdx];
        if (!tok) break;

        // Stop at next heading that ends the text field context
        if (tok.type === 'heading_open') {
          const nextLevel = getHeadingLevel(tok.tag);
          // Check if this heading ends the text field
          if (nextLevel <= fieldLevel) {
            // Same or higher level - stop before it
            endLine = tok.map ? tok.map[0] : endLine;
            break;
          }
          // Check if heading has [[id]]
          const nextHeader = parseHeader(tokens, scanIdx);
          if (nextHeader && nextHeader.hasExplicitId) {
            // Has explicit [[id]] - stop before it
            endLine = tok.map ? tok.map[0] : endLine;
            break;
          }
          // Heading without [[id]] at deeper level - include in text content
        }

        // Update endLine based on token's map
        if (tok.map) {
          endLine = tok.map[1];
        }
        scanIdx++;
      }

      // Extract raw slice
      const rawText = blockTree.getLinesRaw(startLine, endLine).trim();
      const parentObj = objects[parentId];
      if (parentObj) {
        // Append to existing content if any
        const existing = parentObj[fieldName];
        const existingText = typeof existing === 'string' ? existing : '';
        parentObj[fieldName] = existingText ? existingText + '\n\n' + rawText : rawText;

        // Add __types for string field
        if (!parentObj.__types) {
          parentObj.__types = {};
        }
        (parentObj.__types as Record<string, string>)[fieldName] = 'string';

        // Add __syntax for multiline_text (for rebuild)
        if (!parentObj.__syntax) {
          parentObj.__syntax = {};
        }
        (parentObj.__syntax as Record<string, string>)[fieldName] = 'multiline_text';

        // Add __labels for field label
        if (!parentObj.__labels) {
          parentObj.__labels = {};
        }
        (parentObj.__labels as Record<string, string>)[fieldName] = fieldLabel;
      }

      i = scanIdx;

      // Don't clear pendingTextField here - headings inside text are handled in heading_open
    } else if (token.type === 'paragraph_open' && !pendingTextField) {
      // This is a comment paragraph or text block content
      const currentId = getCurrentObjectId();
      if (currentId) {
        // Use raw-slice extraction for comment paragraphs
        const paraStartLine = token.map ? token.map[0] : 0;
        let contentEndLine = token.map ? token.map[1] : paraStartLine + 1;
        const currentLevel = objectStack.length > 0 ? objectStack[objectStack.length - 1]![1] : 0;
        let listNesting = 0;
        let paraCount = 1; // We're starting with a paragraph
        let scanIdx = i + 1; // After paragraph_open

        // Scan to find end boundary
        while (scanIdx < tokens.length) {
          const scanTok = tokens[scanIdx];
          if (!scanTok) break;

          // Track nesting
          if (
            scanTok.type === 'bullet_list_open' ||
            scanTok.type === 'ordered_list_open' ||
            scanTok.type === 'blockquote_open'
          ) {
            listNesting++;
          } else if (
            scanTok.type === 'bullet_list_close' ||
            scanTok.type === 'ordered_list_close' ||
            scanTok.type === 'blockquote_close'
          ) {
            listNesting--;
          }

          // Stop at heading
          if (scanTok.type === 'heading_open') {
            const nextLevel = getHeadingLevel(scanTok.tag);
            if (nextLevel <= currentLevel) {
              contentEndLine = scanTok.map ? scanTok.map[0] : contentEndLine;
              break;
            }
            // Check if nested heading creates object or field
            const nextHeader = parseHeader(tokens, scanIdx);
            if (nextHeader) {
              if (nextHeader.kind || nextHeader.fieldType) {
                contentEndLine = scanTok.map ? scanTok.map[0] : contentEndLine;
                break;
              }
              if (nextHeader.hasExplicitId) {
                contentEndLine = scanTok.map ? scanTok.map[0] : contentEndLine;
                break;
              }
            }
            // Nested heading without [[id]] - include in comment
          }

          // Stop at field list (only at top level)
          if (scanTok.type === 'bullet_list_open' && listNesting === 1) {
            // Check if first item is a field
            let checkIdx = scanIdx + 1;
            let firstItemIsField = false;
            let firstFieldLine: number | null = null;
            let itemStartLine: number | null = null;

            while (checkIdx < tokens.length && tokens[checkIdx]?.type !== 'bullet_list_close') {
              if (tokens[checkIdx]?.type === 'list_item_open') {
                itemStartLine = tokens[checkIdx]?.map ? tokens[checkIdx]!.map![0] : null;
              }
              if (tokens[checkIdx]?.type === 'inline') {
                const listContent = tokens[checkIdx]?.content || '';
                if (qmdcFieldPattern.test(listContent)) {
                  if (firstFieldLine === null) {
                    firstFieldLine = itemStartLine;
                  }
                  const listStartLine = scanTok.map ? scanTok.map[0] : null;
                  if (!firstItemIsField && firstFieldLine === listStartLine) {
                    firstItemIsField = true;
                  }
                }
              }
              checkIdx++;
            }

            if (firstItemIsField) {
              // Whole list is field list - stop before it
              contentEndLine = scanTok.map ? scanTok.map[0] : contentEndLine;
              listNesting--;
              break;
            } else if (firstFieldLine !== null) {
              // Mixed list - stop at first field item line
              contentEndLine = firstFieldLine;
              listNesting--;
              break;
            }
            // else: no field items, continue including this list
          }

          // Count TOP-LEVEL paragraphs only - stop at SECOND one
          if (scanTok.type === 'paragraph_open' && listNesting === 0) {
            paraCount++;
            if (paraCount >= 2) {
              // Second top-level paragraph - stop here, it starts a new comment
              contentEndLine = scanTok.map ? scanTok.map[0] : contentEndLine;
              break;
            }
          }

          // Update endLine based on token's map
          if (scanTok.map) {
            contentEndLine = scanTok.map[1];
          }
          scanIdx++;
        }

        // Extract raw slice
        let rawContent = blockTree.getLinesRaw(paraStartLine, contentEndLine).trim();

        // Normalize: if paragraph directly followed by list without blank line, add blank line
        // This ensures proper rebuild. Only apply when the list is included in the raw content.
        if (rawContent) {
          // Find paragraph_close token
          let normIdx = i + 1;
          while (normIdx < tokens.length && tokens[normIdx]?.type !== 'paragraph_close') {
            normIdx++;
          }
          if (normIdx < tokens.length) {
            normIdx++; // Skip paragraph_close
            if (
              normIdx < tokens.length &&
              (tokens[normIdx]?.type === 'bullet_list_open' ||
                tokens[normIdx]?.type === 'ordered_list_open')
            ) {
              // Check if there's a blank line between paragraph and list
              const paraEnd = token.map ? token.map[1] : 0;
              const listStart = tokens[normIdx]?.map ? tokens[normIdx]!.map![0] : 0;
              // Only normalize if the list is actually included in the raw content
              if (listStart === paraEnd && contentEndLine > listStart) {
                // No blank line - add one after first line
                rawContent = rawContent.replace(/^([^\n]+)\n/, '$1\n\n');
              }
            }
          }
        }

        if (rawContent) {
          // Add to __comments
          const currentObj = objects[currentId];
          if (currentObj) {
            if (!currentObj.__comments) {
              currentObj.__comments = [];
            }
            (currentObj.__comments as Array<{ after: string; content: string }>).push({
              after: commentAnchor,
              content: rawContent,
            });
          }
        }

        // Check for mixed_field_keys: bullet list items with invalid field-like keys
        // Strip backtick spans before checking — colons inside code are not fields
        if (currentId && objects[currentId]?.__types) {
          let mixedScan = i;
          while (mixedScan < scanIdx) {
            const mt = tokens[mixedScan];
            if (mt?.type === 'bullet_list_open' && mt.map && mt.map[0] >= paraStartLine) {
              let ms = mixedScan + 1;
              while (ms < tokens.length && tokens[ms]?.type !== 'bullet_list_close') {
                if (tokens[ms]?.type === 'inline') {
                  const mc = (tokens[ms]?.content || '').trim();
                  const mfl = mc.split('\n')[0] ?? '';
                  let sanitized = mfl.replace(backtickStripRe, '');
                  sanitized = sanitized.replace(boldStripRe, '$1');
                  sanitized = sanitized.replace(italicStripRe, '$1');
                  sanitized = sanitized.replace(strikethroughStripRe, '$1');
                  const invM = sanitized.match(invalidFieldLikeRe);
                  if (invM && !fieldStrictRe.test(sanitized)) {
                    const pk = (invM[1] ?? '').trim();
                    if (pk && !validKeyRe.test(pk)) {
                      const errLine = tokens[ms]?.map ? tokens[ms]!.map![0] + 1 : 0;
                      parsingErrors.push({
                        __id: `error_${parsingErrors.length}`,
                        __kind: '__ParsingError',
                        type: 'mixed_field_keys',
                        object: `[[#${currentId}]]`,
                        line: errLine,
                      });
                      break;
                    }
                  }
                }
                ms++;
              }
            }
            mixedScan++;
          }
        }

        i = scanIdx;
      } else if (pendingTextBlockStarted) {
        // Collect text for pending TextBlock
        i++; // skip paragraph_open
        const inlineToken = tokens[i];
        if (inlineToken && inlineToken.type === 'inline') {
          const content = inlineToken.content || '';
          i++; // skip inline
          const closeToken = tokens[i];
          if (closeToken && closeToken.type === 'paragraph_close') {
            i++; // skip paragraph_close
          }
          pendingTextBlockContent.push(content);
        }
      } else {
        // Text before any object - start a text block
        const paraLine = token.map ? token.map[0] + 1 : 1;
        i++; // skip paragraph_open
        const inlineToken = tokens[i];
        if (inlineToken && inlineToken.type === 'inline') {
          const content = inlineToken.content || '';
          i++; // skip inline
          const closeToken = tokens[i];
          if (closeToken && closeToken.type === 'paragraph_close') {
            i++; // skip paragraph_close
          }
          pendingTextBlockContent.push(content);
          pendingTextBlockStarted = true;
          pendingTextBlockLine = paraLine;
        }
      }
    } else if (
      (token.type === 'table_open' || token.type === 'blockquote_open' || token.type === 'hr') &&
      !pendingTextField &&
      !pendingObjectArray
    ) {
      // Block-level content as comment inside object - use raw slice
      // Capture when: after fields (commentAnchor !== '__self') OR object has explicit Kind
      const currentId = getCurrentObjectId();
      const currentObj = currentId ? objects[currentId] : null;
      const hasExplicitKind =
        currentObj && typeof currentObj.__kind === 'string' && currentObj.__kind !== '__Object';
      if (currentId && (commentAnchor !== '__self' || hasExplicitKind)) {
        const startLine = token.map ? token.map[0] : 0;
        let endLine = token.map ? token.map[1] : startLine + 1;
        const currentLevel = objectStack.length > 0 ? objectStack[objectStack.length - 1]![1] : 0;
        let scanIdx = i + 1;

        // Scan to find end boundary
        while (scanIdx < tokens.length) {
          const scanTok = tokens[scanIdx];
          if (!scanTok) break;

          // Stop at heading
          if (scanTok.type === 'heading_open') {
            const nextLevel = getHeadingLevel(scanTok.tag);
            if (nextLevel <= currentLevel) {
              endLine = scanTok.map ? scanTok.map[0] : endLine;
              break;
            }
            // Check if nested heading creates object or field
            const nextHeader = parseHeader(tokens, scanIdx);
            if (nextHeader) {
              if (nextHeader.kind || nextHeader.fieldType) {
                endLine = scanTok.map ? scanTok.map[0] : endLine;
                break;
              }
              if (nextHeader.hasExplicitId && hasFieldsAfterHeading(scanIdx)) {
                endLine = scanTok.map ? scanTok.map[0] : endLine;
                break;
              }
            }
          }

          // Stop at field list
          if (scanTok.type === 'bullet_list_open' && bulletListHasFields(scanIdx)) {
            endLine = scanTok.map ? scanTok.map[0] : endLine;
            break;
          }

          // Update endLine
          if (scanTok.map) {
            endLine = scanTok.map[1];
          }
          scanIdx++;
        }

        // Extract raw slice
        const rawContent = blockTree.getLinesRaw(startLine, endLine).trim();
        if (rawContent) {
          const currentObj = objects[currentId];
          if (currentObj) {
            if (!currentObj.__comments) {
              currentObj.__comments = [];
            }
            (currentObj.__comments as Array<{ after: string; content: string }>).push({
              after: commentAnchor,
              content: rawContent,
            });
          }
        }
        i = scanIdx;
      } else {
        i++;
      }
    } else if (
      token.type === 'fence' &&
      !pendingYamlField &&
      !pendingJsonField &&
      !pendingTextField &&
      !getCurrentObjectId()
    ) {
      // Code fence outside of QMD.md object - add to text block
      const lang = token.info || '';
      const fenceContent = (token.content || '').replace(/\n$/, '');
      const fenceText = lang
        ? `\`\`\`${lang}\n${fenceContent}\n\`\`\``
        : `\`\`\`\n${fenceContent}\n\`\`\``;
      const fenceLines = fenceText.split('\n').length;

      // Calculate offset within content (0-based line number)
      const existingContent = pendingTextBlockContent.join('\n\n');
      const offsetLine = existingContent.length === 0 ? 0 : existingContent.split('\n').length + 1; // +1 for blank line separator

      // Initialize text block if needed
      if (!pendingTextBlockStarted) {
        pendingTextBlockStarted = true;
        pendingTextBlockLine = token.map ? token.map[0] + 1 : 1;
      }

      // Add code fence metadata
      pendingCodeFences.push({
        lang,
        offset_line: offsetLine,
        length_lines: fenceLines,
      });

      // Add code fence text to pending text block
      pendingTextBlockContent.push(fenceText);
      i++;
    } else if (token.type === 'fence') {
      // Fence inside object — capture as comment using raw slice
      // Capture when: after fields (commentAnchor !== '__self') OR object has explicit Kind
      const currentId = getCurrentObjectId();
      const currentObj = currentId ? objects[currentId] : null;
      const hasExplicitKind =
        currentObj && typeof currentObj.__kind === 'string' && currentObj.__kind !== '__Object';
      if (currentId && (commentAnchor !== '__self' || hasExplicitKind)) {
        const startLine = token.map ? token.map[0] : 0;
        let endLine = token.map ? token.map[1] : startLine + 1;
        const currentLevel = objectStack.length > 0 ? objectStack[objectStack.length - 1]![1] : 0;
        let scanIdx = i + 1;

        // Scan to find end boundary
        while (scanIdx < tokens.length) {
          const scanTok = tokens[scanIdx];
          if (!scanTok) break;

          if (scanTok.type === 'heading_open') {
            const nextLevel = getHeadingLevel(scanTok.tag);
            if (nextLevel <= currentLevel) {
              endLine = scanTok.map ? scanTok.map[0] : endLine;
              break;
            }
            const nextHeader = parseHeader(tokens, scanIdx);
            if (nextHeader) {
              if (nextHeader.kind || nextHeader.fieldType) {
                endLine = scanTok.map ? scanTok.map[0] : endLine;
                break;
              }
              if (nextHeader.hasExplicitId) {
                endLine = scanTok.map ? scanTok.map[0] : endLine;
                break;
              }
            }
          }

          // Stop at field list
          // Stop at field list
          if (scanTok.type === 'bullet_list_open' && bulletListHasFields(scanIdx)) {
            endLine = scanTok.map ? scanTok.map[0] : endLine;
            break;
          }

          if (scanTok.map) {
            endLine = scanTok.map[1];
          }
          scanIdx++;
        }

        const rawContent = blockTree.getLinesRaw(startLine, endLine).trim();
        if (rawContent) {
          if (currentObj) {
            if (!currentObj.__comments) {
              currentObj.__comments = [];
            }
            (currentObj.__comments as Array<{ after: string; content: string }>).push({
              after: commentAnchor,
              content: rawContent,
            });
          }
        }
        i = scanIdx;
      } else {
        i++;
      }
    } else {
      i++;
    }
  }

  // Handle any remaining pendingTextField at end of file
  if (pendingTextField) {
    const [pfParentId, pfFieldName, _pfLevel, pfFieldLabel] = pendingTextField;
    const pfParentObj = objects[pfParentId];
    if (pfParentObj) {
      // Only set empty string if field wasn't already set
      if (pfParentObj[pfFieldName] === undefined) {
        pfParentObj[pfFieldName] = '';
      }
      // Add __types for string field
      if (!pfParentObj.__types) {
        pfParentObj.__types = {};
      }
      (pfParentObj.__types as Record<string, string>)[pfFieldName] = 'string';

      // Add __syntax for multiline_text (for rebuild)
      if (!pfParentObj.__syntax) {
        pfParentObj.__syntax = {};
      }
      (pfParentObj.__syntax as Record<string, string>)[pfFieldName] = 'multiline_text';

      // Add __labels for field label
      if (!pfParentObj.__labels) {
        pfParentObj.__labels = {};
      }
      (pfParentObj.__labels as Record<string, string>)[pfFieldName] = pfFieldLabel;
    }
  }

  // Handle any remaining pending text field at end of file
  if (pendingTextField && pendingTextFieldStartLine !== null) {
    const [pfParentId, pfFieldName, , pfFieldLabel] = pendingTextField;
    const pfParentObj = objects[pfParentId];
    if (pfParentObj) {
      const endLine = blockTree.lineCount;
      const rawText = blockTree.getLinesRaw(pendingTextFieldStartLine, endLine).trim();
      if (rawText) {
        const existing = pfParentObj[pfFieldName];
        const existingText = typeof existing === 'string' ? existing : '';
        pfParentObj[pfFieldName] = existingText ? existingText + '\n\n' + rawText : rawText;
      } else if (!(pfFieldName in pfParentObj)) {
        pfParentObj[pfFieldName] = '';
      }
      if (!pfParentObj.__types) pfParentObj.__types = {};
      (pfParentObj.__types as Record<string, string>)[pfFieldName] = 'string';
      if (!pfParentObj.__syntax) pfParentObj.__syntax = {};
      (pfParentObj.__syntax as Record<string, string>)[pfFieldName] = 'multiline_text';
      if (!pfParentObj.__labels) pfParentObj.__labels = {};
      (pfParentObj.__labels as Record<string, string>)[pfFieldName] = pfFieldLabel;
    }
    pendingTextField = null;
    pendingTextFieldStartLine = null;
  }

  // Handle any remaining pending text block at end of file
  if (pendingTextBlockStarted && pendingTextBlockContent.length > 0) {
    const textBlockId = `text_${textBlockCounter}`;
    const tb: {
      __id: string;
      __kind: string;
      content: string;
      __line?: number;
      __code_fences?: CodeFenceInfo[];
    } = {
      __id: textBlockId,
      __kind: '__TextBlock',
      content: pendingTextBlockContent.join('\n\n'),
    };
    if (format === 'full') {
      tb.__line = pendingTextBlockLine;
      if (pendingCodeFences.length > 0) {
        tb.__code_fences = [...pendingCodeFences];
      }
    }
    textBlocks.push(tb);
    contentOrder.push(textBlockId);
  }

  // Build result list
  const result: QmdcObject[] = [];

  // Check if we need a __Document (if there are text blocks)
  if (textBlocks.length > 0) {
    // Generate document ID using stable random suffix
    const fallback = generateFallbackId(); // returns object_xyz123
    const docId = `doc_${fallback.substring(7)}`; // strip "object_" prefix

    // Build content array with references
    const docContent = contentOrder.map((itemId) => `[[#${itemId}]]`);

    // Create __Document object (no __label for system types)
    const docObj = {
      __id: docId,
      __kind: '__Document',
      content: docContent,
    } as QmdcObject;
    result.push(docObj);

    // Add text blocks with __container (no __label for system types)
    for (const tb of textBlocks) {
      const tbObj: QmdcObject = {
        __id: tb.__id,
        __kind: tb.__kind,
        content: tb.content,
      };
      if (tb.__line !== undefined) {
        tbObj.__line = tb.__line;
      }
      tbObj.__container = `[[#${docId}]]`;
      if (tb.__code_fences && tb.__code_fences.length > 0) {
        tbObj.__code_fences = tb.__code_fences;
      }
      result.push(tbObj);
    }

    // Add objects with __container: regular objects then duplicates
    for (const obj of Object.values(objects)) {
      obj.__container = `[[#${docId}]]`;
      result.push(obj);
    }
    for (const obj of duplicateObjects) {
      obj.__container = `[[#${docId}]]`;
      result.push(obj);
    }
  } else {
    // No text blocks - regular objects then duplicates
    for (const obj of Object.values(objects)) {
      result.push(obj);
    }
    for (const obj of duplicateObjects) {
      result.push(obj);
    }
  }

  // Extract references for full format
  if (activeFeatures.has(FEATURE_REFERENCES)) {
    const lines = markdown.split('\n');
    // Operate on result objects (which contain the normalized objects)
    for (const obj of result) {
      extractReferencesForObject(obj, lines);
    }
  }

  // Extract field positions for full format (after references)
  if (activeFeatures.has(FEATURE_POSITIONS)) {
    const lines = markdown.split('\n');
    for (const obj of result) {
      extractFieldPositions(obj, lines, result);
    }
  }

  // Normalize field order (after all metadata is added) — only for regular objects
  for (let i = 0; i < result.length; i++) {
    const kind = result[i]!.__kind as string;
    if (kind !== '__Document' && kind !== '__TextBlock') {
      result[i] = normalizeFieldOrder(result[i]!);
    }
  }

  // Add parsing errors to result
  // Errors appear after all objects, sorted by line number among themselves
  if (parsingErrors.length > 0) {
    for (const error of parsingErrors) {
      result.push(error as unknown as QmdcObject);
    }
    result.sort((a, b) => {
      const getKey = (obj: QmdcObject): [number, number] => {
        const kind = obj.__kind as string;
        if (kind === '__ParsingError') return [1, (obj.line as number) ?? 0];
        if (kind === '__Document') return [-1, 0];
        return [0, (obj.__line as number) ?? 0];
      };
      const [groupA, lineA] = getKey(a);
      const [groupB, lineB] = getKey(b);
      if (groupA !== groupB) return groupA - groupB;
      return lineA - lineB;
    });
  }

  // Filter by active features
  return result.map((obj) => filterByFeatures(obj, activeFeatures));
}

/**
 * Filter object fields based on active features.
 * Preserves original key order from the input object.
 */
function filterByFeatures(obj: QmdcObject, features: Set<string>): QmdcObject {
  // Define which keys to skip based on features
  const skipKeys = new Set<string>();

  // In minimal mode (no FEATURE_ID), skip __id if it was auto-generated
  const skipId = !features.has(FEATURE_ID) && obj.__has_explicit_id === false;

  // In minimal mode (no FEATURE_KIND), skip __kind if it's a system type (starts with __)
  const skipKind =
    !features.has(FEATURE_KIND) && (typeof obj.__kind !== 'string' || obj.__kind.startsWith('__'));

  if (!features.has(FEATURE_LABEL)) {
    skipKeys.add('__label');
  }

  if (!features.has(FEATURE_PARENT)) {
    skipKeys.add('__container');
    skipKeys.add('__parent');
    skipKeys.add('__parent_field');
  }

  if (!features.has(FEATURE_TYPES)) {
    skipKeys.add('__types');
  }

  if (!features.has(FEATURE_SYNTAX)) {
    skipKeys.add('__syntax');
  }

  if (!features.has(FEATURE_LEVEL)) {
    skipKeys.add('__level');
  }

  if (!features.has(FEATURE_LINE)) {
    skipKeys.add('__line');
  }

  if (!features.has(FEATURE_EXPLICIT_ID)) {
    skipKeys.add('__has_explicit_id');
  }

  if (!features.has(FEATURE_REFERENCES)) {
    skipKeys.add('__references');
  }

  if (!features.has(FEATURE_POSITIONS)) {
    skipKeys.add('__positions');
  }

  // Copy keys in original order, skipping disabled ones
  const result: QmdcObject = {} as QmdcObject;

  // Add __id first if not skipped
  if (!skipId) {
    result.__id = obj.__id;
  }

  for (const key of Object.keys(obj)) {
    if (key === '__id') continue; // Already handled
    if (key === '__kind' && skipKind) continue;
    if (!skipKeys.has(key)) {
      result[key] = obj[key];
    }
  }

  // Propagate non-enumerable __raw_values for rebuild
  const rawDesc = Object.getOwnPropertyDescriptor(obj, '__raw_values');
  if (rawDesc) {
    Object.defineProperty(result, '__raw_values', rawDesc);
  }

  return result;
}

/**
 * Normalize field order in object:
 * 1. __id, __label, __kind, __container, __parent, __parent_field
 * 2. __comments (before data fields per spec)
 * 3. Data fields
 * 4. __types, __syntax, __level, __has_explicit_id
 */
function normalizeFieldOrder(obj: QmdcObject): QmdcObject {
  // Canonical order: __id, __local_id, __label, __kind, __container, __parent, __parent_field, __comments, data, __types, __syntax, __level, __line, __has_explicit_id, __references, __positions, __labels
  const result: QmdcObject = { __id: obj.__id };

  // Identity fields
  if (obj.__local_id !== undefined) result.__local_id = obj.__local_id;
  if (obj.__label !== undefined) result.__label = obj.__label;
  if (obj.__kind !== undefined) result.__kind = obj.__kind;
  if (obj.__container !== undefined) result.__container = obj.__container;
  if (obj.__parent !== undefined) result.__parent = obj.__parent;
  if (obj.__parent_field !== undefined) result.__parent_field = obj.__parent_field;

  // Comments before data
  if (obj.__comments !== undefined) result.__comments = obj.__comments;

  // Data fields
  for (const key of Object.keys(obj)) {
    if (!key.startsWith('__')) {
      result[key] = obj[key];
    }
  }

  // Metadata fields
  if (obj.__types !== undefined) result.__types = obj.__types;
  if (obj.__syntax !== undefined) result.__syntax = obj.__syntax;
  if (obj.__level !== undefined) result.__level = obj.__level;
  if (obj.__line !== undefined) result.__line = obj.__line;
  if (obj.__has_explicit_id !== undefined) result.__has_explicit_id = obj.__has_explicit_id;
  if (obj.__references !== undefined) result.__references = obj.__references;
  if (obj.__positions !== undefined) result.__positions = obj.__positions;
  if (obj.__labels !== undefined) result.__labels = obj.__labels;

  // Propagate non-enumerable __raw_values for rebuild
  const rawDesc = Object.getOwnPropertyDescriptor(obj, '__raw_values');
  if (rawDesc) {
    Object.defineProperty(result, '__raw_values', rawDesc);
  }

  return result;
}

/**
 * Extract references from a single object and add __references field
 */
function extractReferencesForObject(obj: QmdcObject, lines: string[]): void {
  const refs: ParsedReference[] = [];
  const objLine = obj.__line;
  if (typeof objLine !== 'number') return;

  // Search starts at the object's own line (not the file top) and advances
  // monotonically, so a reference value that also appears in an earlier object
  // is attributed to THIS object's actual occurrence, not the first in the file.
  let objSearchStart = objLine > 0 ? objLine - 1 : 0;

  // Process string fields
  for (const [key, value] of Object.entries(obj)) {
    if (key.startsWith('__')) {
      // Check comments
      if (key === '__comments' && Array.isArray(value)) {
        for (const comment of value) {
          if (typeof comment === 'object' && comment !== null) {
            const content = (comment as Record<string, unknown>).content;
            if (typeof content === 'string' && content.includes('[[')) {
              // Find this comment in source to get line number
              for (let lineIdx = objSearchStart; lineIdx < lines.length; lineIdx++) {
                const line = lines[lineIdx];
                if (line && line.includes(content)) {
                  const lineNum = lineIdx + 1;
                  const colOffset = line.indexOf(content);
                  refs.push(...extractReferencesFromText(content, lineNum, colOffset));
                  objSearchStart = lineIdx + 1;
                  break;
                }
              }
            }
          }
        }
      }
      continue;
    }

    if (typeof value === 'string' && value.includes('[[')) {
      // For multiline text fields, search each line in original markdown
      // Track search position to handle duplicate lines in content
      // Track if we're inside a code fence
      let inCodeFence = false;
      let exampleFenceDepth = 0;

      for (const contentLine of value.split('\n')) {
        // Check for code fence markers
        const stripped = contentLine.trim();
        if (stripped.startsWith('```')) {
          if (exampleFenceDepth > 0) {
            const fenceContent = stripped.substring(3);
            if (fenceContent) {
              exampleFenceDepth++;
            } else {
              exampleFenceDepth--;
            }
          } else {
            const fenceContent = stripped.substring(3);
            if (fenceContent.includes('example')) {
              exampleFenceDepth = 1;
            }
            inCodeFence = !inCodeFence;
          }
          continue;
        }

        // Skip references inside example code fences
        if (exampleFenceDepth > 0) {
          continue;
        }

        if (!contentLine.includes('[[')) continue;
        const contentTrimmed = stripped;
        if (!contentTrimmed) continue;

        // Find this line in original markdown, starting from last found position
        for (let lineIdx = objSearchStart; lineIdx < lines.length; lineIdx++) {
          const origLine = lines[lineIdx];
          if (origLine && origLine.includes(contentTrimmed) && origLine.includes('[[')) {
            const lineNum = lineIdx + 1;
            refs.push(...extractReferencesFromText(origLine, lineNum, 0));
            // Move search position forward for next content line
            objSearchStart = lineIdx + 1;
            break;
          }
        }
      }
    } else if (Array.isArray(value)) {
      // Check array items for references
      // Check if this is a YAML array (via __syntax)
      const syntax = obj.__syntax as Record<string, string> | undefined;
      const isYamlArray = syntax?.[key] === 'yaml_array';

      if (isYamlArray) {
        // For YAML arrays, extract references from each array element
        // Find the line containing this field
        let fieldLineIdx: number | undefined;
        let fieldLine: string | undefined;
        for (let lineIdx = 0; lineIdx < lines.length; lineIdx++) {
          const line = lines[lineIdx];
          if (line && line.includes(`${key}:`) && line.includes('[[')) {
            fieldLineIdx = lineIdx;
            fieldLine = line;
            break;
          }
        }

        if (fieldLine && fieldLineIdx !== undefined) {
          // Find where the value starts (after "key: ")
          const colonPos = fieldLine.indexOf(':');
          if (colonPos >= 0) {
            let valueStart = colonPos + 1;
            while (valueStart < fieldLine.length && fieldLine[valueStart] === ' ') {
              valueStart++;
            }

            // Extract value string from markdown
            const valueStr = fieldLine.slice(valueStart).trim();

            // For each array item, find its position in the value string
            let searchPos = 0;
            for (const item of value) {
              if (typeof item === 'string' && item.includes('[[')) {
                // Find this item in the value string
                const itemPos = valueStr.slice(searchPos).indexOf(item);
                if (itemPos >= 0) {
                  const absolutePos = valueStart + searchPos + itemPos;
                  const lineNum = fieldLineIdx + 1;
                  refs.push(...extractReferencesFromText(item, lineNum, absolutePos));
                  searchPos += itemPos + item.length;
                }
              }
            }
          }
        }
      } else {
        // For non-YAML arrays (markdown lists), use original logic
        for (const item of value) {
          if (typeof item === 'string' && item.includes('[[')) {
            // Find this item in source
            for (let lineIdx = 0; lineIdx < lines.length; lineIdx++) {
              const line = lines[lineIdx];
              if (line && line.includes(item)) {
                const lineNum = lineIdx + 1;
                const colOffset = line.indexOf(item);
                refs.push(...extractReferencesFromText(item, lineNum, colOffset));
                break;
              }
            }
          }
        }
      }
    }
  }

  if (refs.length > 0) {
    // Remove duplicate references (same target, line, start_col, end_col)
    const seen = new Set<string>();
    const uniqueRefs: ParsedReference[] = [];
    for (const ref of refs) {
      const key = `${ref.target}:${ref.line}:${ref.start_col}:${ref.end_col}`;
      if (!seen.has(key)) {
        seen.add(key);
        uniqueRefs.push(ref);
      }
    }
    obj.__references = uniqueRefs;
  }
}

/**
 * Extract field positions and add __positions field
 */
function extractFieldPositions(obj: QmdcObject, lines: string[], allObjects: QmdcObject[]): void {
  const positions: Record<string, { line: number; col: number }> = {};
  const objLine = obj.__line;
  if (objLine === undefined || objLine === null || typeof objLine !== 'number') return;

  // Get field names (non-meta keys)
  const fieldNames = Object.keys(obj).filter((k) => !k.startsWith('__'));

  for (const fieldName of fieldNames) {
    // Skip parent fields - these are fields that contain only [[#id]]
    // and have a child object with __parent_field equal to this field name
    const fieldValue = obj[fieldName];
    if (
      typeof fieldValue === 'string' &&
      fieldValue.trim().startsWith('[[#') &&
      fieldValue.trim().endsWith(']]')
    ) {
      // Check if any object has __parent_field == fieldName
      const isParentField = allObjects.some((child) => child.__parent_field === fieldName);
      if (isParentField) {
        continue;
      }
    }

    // Search for field definition in lines starting from object line
    for (let lineIdx = objLine - 1; lineIdx < lines.length; lineIdx++) {
      const line = lines[lineIdx];
      if (!line) continue;

      // Check for list item field: `- field: value` or `- **field**: value`
      const listPattern = new RegExp(
        `^\\s*-\\s+\\**${fieldName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\**\\s*:`
      );
      if (listPattern.test(line)) {
        const colChar = line.indexOf(fieldName);
        // Convert character position to byte position (for UTF-8)
        const col = Buffer.byteLength(line.slice(0, colChar), 'utf8');
        positions[fieldName] = { line: lineIdx + 1, col };
        break;
      }

      // Check for heading text field: `## Label [[id:text]]` or `## Label [[id]]`
      const headingPattern = new RegExp(
        `^#+\\s+.*\\[\\[${fieldName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?::.*?)?\\]\\]`
      );
      if (headingPattern.test(line)) {
        const colChar = line.indexOf(`[[${fieldName}`);
        // Convert character position to byte position (for UTF-8)
        const col = Buffer.byteLength(line.slice(0, colChar), 'utf8');
        positions[fieldName] = { line: lineIdx + 1, col };
        break;
      }
    }
  }

  if (Object.keys(positions).length > 0) {
    obj.__positions = positions;
  }
}

/**
 * Rebuild a map field as QMD.md lines: heading + bullet list of key: value pairs.
 */
function rebuildMapLines(
  key: string,
  value: Record<string, unknown>,
  level: number,
  getFieldLabel: (k: string) => string
): string[] {
  const lines: string[] = ['', `${'#'.repeat(level)} ${getFieldLabel(key)} [[${key}: map]]`];
  if (Object.keys(value).length > 0) {
    lines.push('');
    for (const [mk, mv] of Object.entries(value as Record<string, string>)) {
      const mvStr = String(mv);
      if (mvStr.includes('\n')) {
        lines.push(`- ${mk}: |`);
        for (const ml of mvStr.split('\n')) {
          lines.push(`    ${ml}`);
        }
      } else {
        lines.push(`- ${mk}: ${mvStr}`);
      }
    }
  }
  return lines;
}

/**
 * Helper function to rebuild a single object to lines
 */
function rebuildObjectToLines(
  obj: QmdcObject,
  objectsById: Record<string, QmdcObject>,
  lines: string[],
  childrenMap: Record<string, string[]>
): void {
  const objId = obj.__id || '';

  // Use __level from object if present
  const actualLevel = (obj.__level as number) ?? 2;

  // Render heading
  const heading = rebuildHeading(obj, actualLevel);
  lines.push(heading);

  // Get comments and syntax info for this object
  const objComments = (obj.__comments as Array<{ after: string; content: string }>) || [];
  const objSyntax = (obj.__syntax as Record<string, string>) || {};
  const objLabels = (obj.__labels as Record<string, string>) || {};
  const objRawValues =
    (Object.getOwnPropertyDescriptor(obj, '__raw_values')?.value as Record<string, string>) ?? {};

  // Helper to get field label from __labels or generate from key
  function getFieldLabel(key: string): string {
    if (objLabels[key]) return objLabels[key];
    return key.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
  }

  // Helper to add comment by anchor
  function addCommentAfter(anchor: string): void {
    for (const comment of objComments) {
      if (comment.after === anchor) {
        lines.push('');
        lines.push(comment.content);
      }
    }
  }

  // Add comment after __self (before any fields)
  addCommentAfter('__self');

  // Single-pass: output fields in insertion order, preserving original field order.
  // Heading-syntax fields and child ref headings are output inline.
  // Child ref primitive lines (when __types entry exists) stay in the primitive run;
  // the child heading is output after the primitive run ends.
  const childIds = childrenMap[objId] || [];
  const objTypes = (obj.__types as Record<string, string>) || {};
  const renderedChildren = new Set<string>();
  let inPrimitiveRun = false;
  // Buffer child refs encountered during a primitive run — their headings
  // are output once the primitive run ends.
  let pendingChildHeadings: string[] = [];

  function flushPendingChildHeadings(): void {
    for (const refId of pendingChildHeadings) {
      renderedChildren.add(refId);
      const childObj = objectsById[refId];
      if (childObj) {
        lines.push('');
        rebuildObjectToLines(childObj, objectsById, lines, childrenMap);
      }
    }
    pendingChildHeadings = [];
  }

  for (const key in obj) {
    if (key.startsWith('__')) continue;

    const value = obj[key];
    const syntax = objSyntax[key] || '';

    // Classify field
    const isHeadingSyntax =
      syntax === 'headers' ||
      syntax === 'table' ||
      syntax === 'markdown_list' ||
      syntax === 'multiline_text' ||
      syntax === 'yaml_object' ||
      syntax === 'json_object' ||
      syntax === 'map';

    // Check for child reference
    if (
      !isHeadingSyntax &&
      typeof value === 'string' &&
      value.startsWith('[[#') &&
      value.endsWith(']]')
    ) {
      const refId = value.slice(3, -2);
      if (childIds.includes(refId)) {
        // Output as primitive if it has a __types entry
        if (key in objTypes) {
          if (!inPrimitiveRun) {
            lines.push('');
            inPrimitiveRun = true;
          }
          lines.push(...formatPrimitiveField(key, value, objSyntax, objRawValues));
          addCommentAfter(key);
          // Buffer child heading to render after primitive run
          pendingChildHeadings.push(refId);
        } else {
          // No __types entry — end primitive run and render child immediately
          if (inPrimitiveRun) {
            inPrimitiveRun = false;
            flushPendingChildHeadings();
          }
          renderedChildren.add(refId);
          const childObj = objectsById[refId];
          if (childObj) {
            lines.push('');
            rebuildObjectToLines(childObj, objectsById, lines, childrenMap);
          }
          addCommentAfter(key);
        }
        continue;
      }
    }

    if (isHeadingSyntax) {
      // End primitive run and flush pending child headings
      if (inPrimitiveRun) {
        inPrimitiveRun = false;
        flushPendingChildHeadings();
      }

      if (syntax === 'headers' && Array.isArray(value)) {
        const refs = value as string[];
        if (refs.length > 0) {
          const firstRefId = refs[0]!.slice(3, -2);
          const firstObj = objectsById[firstRefId];
          const kind = firstObj ? (firstObj.__kind as string) || '' : '';

          lines.push('');
          const fieldLabel = getFieldLabel(key);
          lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: [${kind}]]]`);

          for (const ref of refs) {
            const refId = ref.slice(3, -2);
            renderedChildren.add(refId);
            const refObj = objectsById[refId];
            if (refObj) {
              lines.push('');
              rebuildObjectToLines(refObj, objectsById, lines, childrenMap);
            }
          }
        }
      } else if (syntax === 'table' && Array.isArray(value)) {
        const refs = value as string[];
        let kind = '';
        const columnNames: string[] = [];
        const firstRef = refs[0];
        if (firstRef) {
          const firstRefId = firstRef.slice(3, -2);
          const firstObj = objectsById[firstRefId];
          if (firstObj) {
            kind = (firstObj.__kind as string) || '';
            for (const k in firstObj) {
              if (!k.startsWith('__')) {
                columnNames.push(k);
              }
            }
          }
        }

        lines.push('');
        const fieldLabel = getFieldLabel(key);
        lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: [${kind}]]]`);
        lines.push('');

        if (columnNames.length > 0) {
          lines.push('| ' + columnNames.join(' | ') + ' |');
          lines.push('|' + columnNames.map(() => '---').join('|') + '|');

          for (const ref of refs) {
            const refId = ref.slice(3, -2);
            renderedChildren.add(refId);
            const refObj = objectsById[refId];
            if (refObj) {
              const rowValues = columnNames.map((col) => formatValue(refObj[col]));
              lines.push('| ' + rowValues.join(' | ') + ' |');
            }
          }
        }
      } else if (syntax === 'markdown_list' && Array.isArray(value)) {
        lines.push('');
        const fieldLabel = getFieldLabel(key);
        lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: array]]`);
        lines.push('');
        for (const item of value as unknown[]) {
          lines.push(`- ${formatValue(item)}`);
        }
        addCommentAfter(key);
      } else if (syntax === 'multiline_text' && typeof value === 'string') {
        lines.push('');
        const fieldLabel = getFieldLabel(key);
        lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: text]]`);
        lines.push('');
        if (value) {
          lines.push(value);
        }
        addCommentAfter(key);
      } else if (
        syntax === 'yaml_object' &&
        typeof value === 'object' &&
        value !== null &&
        !Array.isArray(value)
      ) {
        lines.push('');
        const fieldLabel = getFieldLabel(key);
        lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: yaml]]`);
        lines.push('');
        lines.push('```yaml');
        const yamlStr = yaml.dump(value as Record<string, unknown>, {
          indent: 2,
          lineWidth: -1,
          sortKeys: false,
        });
        lines.push(yamlStr.trimEnd());
        lines.push('```');
      } else if (syntax === 'json_object') {
        lines.push('');
        const fieldLabel = getFieldLabel(key);
        lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: json]]`);
        lines.push('');
        lines.push('```json');
        lines.push(JSON.stringify(value, null, 2));
        lines.push('```');
      } else if (
        // TODO: deduplicate map rebuild with rebuild()
        syntax === 'map' &&
        typeof value === 'object' &&
        value !== null &&
        !Array.isArray(value)
      ) {
        lines.push(
          ...rebuildMapLines(key, value as Record<string, unknown>, actualLevel + 1, getFieldLabel)
        );
        addCommentAfter(key);
      }
    } else {
      // Primitive field
      if (!inPrimitiveRun) {
        lines.push('');
        inPrimitiveRun = true;
      }
      lines.push(...formatPrimitiveField(key, value, objSyntax, objRawValues));
      addCommentAfter(key);
    }
  }

  // Flush any remaining pending child headings
  flushPendingChildHeadings();

  // Rebuild remaining child objects (excluding those already rendered)
  for (const childId of childIds) {
    if (!renderedChildren.has(childId)) {
      const childObj = objectsById[childId];
      if (childObj) {
        lines.push('');
        rebuildObjectToLines(childObj, objectsById, lines, childrenMap);
      }
    }
  }
}

/**
 * Rebuild QMD.md from JSON
 *
 * Uses __parent to determine document structure and heading levels.
 * Handles __Document and __TextBlock system types.
 */
export function rebuild(data: ParseResult): string {
  const lines: string[] = [];

  // Build index of objects by ID
  const objectsById: Record<string, QmdcObject> = {};
  for (const obj of data) {
    if (obj.__id) {
      objectsById[obj.__id] = obj;
    }
  }

  // Check for __Document (first element with __kind == "__Document")
  let docObj: QmdcObject | null = null;
  for (const obj of data) {
    if (obj.__kind === '__Document') {
      docObj = obj;
      break;
    }
  }

  // If we have a __Document, use its content order
  if (docObj) {
    // Build children map for regular objects
    const childrenMap: Record<string, string[]> = {};
    for (const obj of data) {
      const kind = typeof obj.__kind === 'string' ? obj.__kind : '';
      // Skip system types except __Object, __Workspace, __Namespace
      if (
        !obj.__id ||
        (kind.startsWith('__') &&
          kind !== '__Object' &&
          kind !== '__Workspace' &&
          kind !== '__Namespace')
      ) {
        continue;
      }
      const parentRef = obj.__parent;
      if (
        typeof parentRef === 'string' &&
        parentRef.startsWith('[[#') &&
        parentRef.endsWith(']]')
      ) {
        const parentId = parentRef.slice(3, -2);
        if (!childrenMap[parentId]) {
          childrenMap[parentId] = [];
        }
        childrenMap[parentId].push(obj.__id);
      }
    }

    const contentRefs = (docObj.content as string[]) || [];
    for (const ref of contentRefs) {
      const refId = ref.slice(3, -2); // Extract ID from [[#id]]
      const item = objectsById[refId];
      if (!item) continue;

      if (item.__kind === '__TextBlock') {
        // Output text block content as-is
        if (lines.length > 0) {
          lines.push('');
        }
        lines.push(item.content as string);
      } else {
        // Output regular object
        if (lines.length > 0) {
          lines.push('');
        }
        rebuildObjectToLines(item, objectsById, lines, childrenMap);
      }
    }

    // Remove trailing empty lines, ensure exactly one trailing newline
    while (lines.length > 0 && lines[lines.length - 1] === '') {
      lines.pop();
    }

    return lines.join('\n') + '\n';
  }

  // Filter out system types (except __Object, __Workspace, __Namespace which should be rebuilt)
  const isSystemType = (kind: string | undefined): boolean => {
    return (
      typeof kind === 'string' &&
      kind.startsWith('__') &&
      kind !== '__Object' &&
      kind !== '__Workspace' &&
      kind !== '__Namespace'
    );
  };

  const actualObjects = data.filter((obj) => !isSystemType(obj.__kind));

  // Track order of objects in original data
  const objectOrder: Record<string, number> = {};
  for (let idx = 0; idx < data.length; idx++) {
    const obj = data[idx];
    if (obj && obj.__id) {
      objectOrder[obj.__id] = idx;
    }
  }

  // Build parent->children map
  const childrenMap: Record<string, string[]> = {}; // parent_id -> [child_ids]
  for (const obj of actualObjects) {
    if (!obj.__id) continue;
    const parentRef = obj.__parent;
    if (typeof parentRef === 'string' && parentRef.startsWith('[[#') && parentRef.endsWith(']]')) {
      const parentId = parentRef.slice(3, -2);
      if (!childrenMap[parentId]) {
        childrenMap[parentId] = [];
      }
      childrenMap[parentId].push(obj.__id);
    }
  }

  // Sort children by their order in original data
  for (const parentId in childrenMap) {
    const children = childrenMap[parentId];
    if (children) {
      children.sort((a, b) => (objectOrder[a] ?? 999999) - (objectOrder[b] ?? 999999));
    }
  }

  // Find root objects (no __parent and no __container), preserving order from data
  // Use original data order, not actualObjects order
  const rootIds: string[] = [];
  for (const obj of data) {
    if (obj && obj.__id && !obj.__parent) {
      const kind = typeof obj.__kind === 'string' ? obj.__kind : '';
      if (!isSystemType(kind)) {
        rootIds.push(obj.__id);
      }
    }
  }

  function rebuildObject(objId: string, level: number): void {
    const obj = objectsById[objId];
    if (!obj) return;

    // Get children for this object
    const childIds = childrenMap[objId] || [];

    // Use __level from object if present
    const actualLevel = (obj.__level as number) ?? level;

    // Render heading
    const heading = rebuildHeading(obj, actualLevel);
    lines.push(heading);

    // Get comments and syntax info for this object
    const objComments = (obj.__comments as Array<{ after: string; content: string }>) || [];
    const objSyntax = (obj.__syntax as Record<string, string>) || {};
    const objLabels = (obj.__labels as Record<string, string>) || {};
    const objRawValues =
      (Object.getOwnPropertyDescriptor(obj, '__raw_values')?.value as Record<string, string>) ?? {};

    // Helper to get field label from __labels or generate from key
    function getFieldLabel(key: string): string {
      if (objLabels[key]) return objLabels[key];
      return key.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
    }

    // Helper to add comment by anchor
    function addCommentAfter(anchor: string): void {
      for (const comment of objComments) {
        if (comment.after === anchor) {
          lines.push('');
          lines.push(comment.content);
        }
      }
    }

    // Add comment after __self (before any fields)
    addCommentAfter('__self');

    // Collect child IDs and track which fields are object arrays/tables
    const objectArrayFields: Record<string, string[]> = {}; // field_name -> [ref_ids]
    const tableFields: Record<string, string[]> = {}; // field_name -> [ref_ids]

    for (const key in obj) {
      if (key.startsWith('__')) continue;
      const value = obj[key];
      if (Array.isArray(value) && objSyntax[key] === 'headers') {
        objectArrayFields[key] = value as string[];
      } else if (Array.isArray(value) && objSyntax[key] === 'table') {
        tableFields[key] = value as string[];
      }
    }

    // Single-pass for field ordering with buffered child headings.
    const renderedChildren = new Set<string>();
    let pendingChildHeadings: string[] = [];
    const objTypes = (obj.__types as Record<string, string>) || {};
    let inPrimitiveRun = false;

    function flushPendingChildHeadings(): void {
      for (const refId of pendingChildHeadings) {
        renderedChildren.add(refId);
        const childObj = objectsById[refId];
        if (childObj) {
          lines.push('');
          rebuildObject(refId, actualLevel + 1);
        }
      }
      pendingChildHeadings = [];
    }

    for (const key in obj) {
      if (key.startsWith('__')) continue;

      const value = obj[key];
      const syntax = objSyntax[key] || '';

      const isHeadingSyntax =
        syntax === 'headers' ||
        syntax === 'table' ||
        syntax === 'markdown_list' ||
        syntax === 'multiline_text' ||
        syntax === 'yaml_object' ||
        syntax === 'json_object' ||
        syntax === 'map';

      // Check for child reference
      if (
        !isHeadingSyntax &&
        typeof value === 'string' &&
        value.startsWith('[[#') &&
        value.endsWith(']]')
      ) {
        const refId = value.slice(3, -2);
        if (childIds.includes(refId)) {
          if (key in objTypes) {
            if (!inPrimitiveRun) {
              lines.push('');
              inPrimitiveRun = true;
            }
            lines.push(...formatPrimitiveField(key, value, objSyntax, objRawValues));
            addCommentAfter(key);
            // Buffer child heading to render after primitive run
            pendingChildHeadings.push(refId);
          } else {
            // No __types entry — end primitive run and render child immediately
            if (inPrimitiveRun) {
              inPrimitiveRun = false;
              flushPendingChildHeadings();
            }
            renderedChildren.add(refId);
            const childObj = objectsById[refId];
            if (childObj) {
              lines.push('');
              rebuildObject(refId, actualLevel + 1);
            }
            addCommentAfter(key);
          }
          continue;
        }
      }

      if (isHeadingSyntax) {
        if (inPrimitiveRun) {
          inPrimitiveRun = false;
          flushPendingChildHeadings();
        }

        if (syntax === 'headers' && Array.isArray(value)) {
          const refs = objectArrayFields[key] || [];
          if (refs.length > 0) {
            const firstRefId = refs[0]!.slice(3, -2);
            const firstObj = objectsById[firstRefId];
            const kind = firstObj ? (firstObj.__kind as string) || '' : '';

            // Self-array pattern: field key == object ID means the
            // heading already includes [Kind] annotation — skip sub-heading
            const isSelfArray = key === objId;
            if (!isSelfArray) {
              lines.push('');
              const fieldLabel = getFieldLabel(key);
              lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: [${kind}]]]`);
            }

            for (const ref of refs) {
              const refId = ref.slice(3, -2);
              renderedChildren.add(refId);
              lines.push('');
              const childLevel = isSelfArray ? actualLevel + 1 : actualLevel + 2;
              rebuildObject(refId, childLevel);
            }
          }
        } else if (syntax === 'table') {
          const refs = tableFields[key] || [];
          let kind = '';
          const columnNames: string[] = [];
          const firstRef = refs[0];
          if (firstRef) {
            const firstRefId = firstRef.slice(3, -2);
            const firstObj = objectsById[firstRefId];
            if (firstObj) {
              kind = (firstObj.__kind as string) || '';
              for (const k in firstObj) {
                if (!k.startsWith('__')) {
                  columnNames.push(k);
                }
              }
            }
          }

          lines.push('');
          const fieldLabel = getFieldLabel(key);
          lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: [${kind}]]]`);
          lines.push('');

          if (columnNames.length > 0) {
            lines.push('| ' + columnNames.join(' | ') + ' |');
            lines.push('|' + columnNames.map(() => '---').join('|') + '|');
            for (const ref of refs) {
              const refId = ref.slice(3, -2);
              renderedChildren.add(refId);
              const refObj = objectsById[refId];
              if (refObj) {
                const rowValues = columnNames.map((col) => formatValue(refObj[col]));
                lines.push('| ' + rowValues.join(' | ') + ' |');
              }
            }
          }
        } else if (syntax === 'markdown_list' && Array.isArray(value)) {
          lines.push('');
          const fieldLabel = getFieldLabel(key);
          lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: array]]`);
          lines.push('');
          for (const item of value as unknown[]) {
            lines.push(`- ${formatValue(item)}`);
          }
          addCommentAfter(key);
        } else if (syntax === 'multiline_text' && typeof value === 'string') {
          lines.push('');
          const fieldLabel = getFieldLabel(key);
          lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: text]]`);
          lines.push('');
          if (value) {
            lines.push(value);
          }
          addCommentAfter(key);
        } else if (
          syntax === 'yaml_object' &&
          typeof value === 'object' &&
          value !== null &&
          !Array.isArray(value)
        ) {
          lines.push('');
          const fieldLabel = getFieldLabel(key);
          lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: yaml]]`);
          lines.push('');
          lines.push('```yaml');
          const yamlStr = yaml.dump(value, { indent: 2, lineWidth: -1, sortKeys: false });
          lines.push(yamlStr.trimEnd());
          lines.push('```');
        } else if (syntax === 'json_object') {
          lines.push('');
          const fieldLabel = getFieldLabel(key);
          lines.push(`${'#'.repeat(actualLevel + 1)} ${fieldLabel} [[${key}: json]]`);
          lines.push('');
          lines.push('```json');
          lines.push(JSON.stringify(value, null, 2));
          lines.push('```');
        } else if (
          // TODO: deduplicate map rebuild with rebuildObjectToLines()
          syntax === 'map' &&
          typeof value === 'object' &&
          value !== null &&
          !Array.isArray(value)
        ) {
          lines.push(
            ...rebuildMapLines(
              key,
              value as Record<string, unknown>,
              actualLevel + 1,
              getFieldLabel
            )
          );
          addCommentAfter(key);
        }
      } else {
        if (!inPrimitiveRun) {
          lines.push('');
          inPrimitiveRun = true;
        }
        lines.push(...formatPrimitiveField(key, value, objSyntax, objRawValues));
        addCommentAfter(key);
      }
    }

    // Flush any remaining pending child headings
    flushPendingChildHeadings();

    // Render remaining children not referenced by fields
    for (const childId of childIds) {
      if (renderedChildren.has(childId)) continue;
      const childObj = objectsById[childId];
      if (childObj) {
        lines.push('');
        rebuildObject(childId, actualLevel + 1);
      }
    }
  }

  // Rebuild all root objects
  for (const rootId of rootIds) {
    rebuildObject(rootId, 1);
    lines.push('');
  }

  // Remove trailing empty lines
  while (lines.length > 0 && lines[lines.length - 1] === '') {
    lines.pop();
  }

  return lines.join('\n') + '\n';
}

/**
 * Format a primitive field for output in QMD.md, handling YAML multiline syntax
 */
function formatPrimitiveField(
  key: string,
  value: unknown,
  syntax: Record<string, string>,
  rawValues?: Record<string, string>
): string[] {
  // Check if it's a YAML multiline field
  if (syntax[key] === 'yaml_multiline' && typeof value === 'string') {
    const lines = [`- ${key}: |`];
    // Indent each line with 4 spaces
    for (const line of value.split('\n')) {
      lines.push(`    ${line}`);
    }
    return lines;
  }
  // yaml_multiline_array: multiline bracket format
  if (syntax[key] === 'yaml_multiline_array' && Array.isArray(value)) {
    const items = value.map((item) => formatValue(item));
    const lines = [`- ${key}: [`];
    for (let idx = 0; idx < items.length; idx++) {
      const comma = idx < items.length - 1 ? ',' : '';
      lines.push(`    ${items[idx]}${comma}`);
    }
    lines.push('  ]');
    return lines;
  }
  // comma_refs: comma-separated references without outer brackets
  if (syntax[key] === 'comma_refs' && Array.isArray(value)) {
    const formatted = value.map((item) => formatValue(item)).join(', ');
    return [`- ${key}: ${formatted}`];
  }
  // Use raw value if available (preserves e.g. "1.0" that JS would lose as 1)
  if (rawValues && rawValues[key] !== undefined) {
    return [`- ${key}: ${rawValues[key]}`];
  }
  return [`- ${key}: ${formatValue(value)}`];
}

/**
 * Format a value for output in QMD.md
 */
function formatValue(value: unknown): string {
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  if (value === null) {
    return 'null';
  }
  if (Array.isArray(value)) {
    // Format as YAML array
    const formattedItems = value.map((item) => formatValue(item));
    return '[' + formattedItems.join(', ') + ']';
  }
  if (typeof value === 'string') {
    // Quote strings with leading/trailing whitespace to preserve them
    if (value !== value.trim()) {
      const escaped = value.replace(/"/g, '\\"');
      return `"${escaped}"`;
    }
    return value;
  }
  return String(value);
}

function rebuildHeading(obj: QmdcObject, level: number = 2): string {
  const label: string = obj.__label || '';
  const objId: string = obj.__id || '';
  let kind: string | undefined = obj.__kind;
  const hasExplicitId: boolean = obj.__has_explicit_id !== false; // Default: explicit
  const objSyntax = (obj.__syntax as Record<string, string>) || {};

  // BR-12: Use __local_id for heading reconstruction when present
  const headingId: string = (obj.__local_id as string) || objId;

  // __Object is the default kind - don't output it
  if (kind === '__Object') {
    kind = undefined;
  }

  // Use __level from object if present, otherwise use computed level
  const actualLevel = (obj.__level as number) ?? level;

  const parts: string[] = [];

  if (label) {
    parts.push(label);
  }

  // Check for standalone field type (syntax key matches heading ID)
  const selfSyntax = objSyntax[headingId];
  let fieldTypeHint: string | undefined;
  if (selfSyntax === 'multiline_text') {
    fieldTypeHint = 'text';
  } else if (selfSyntax === 'markdown_list') {
    fieldTypeHint = 'array';
  } else if (selfSyntax === 'yaml_object') {
    fieldTypeHint = 'yaml';
  } else if (selfSyntax === 'json_object') {
    fieldTypeHint = 'json';
  } else if (selfSyntax === 'map') {
    fieldTypeHint = 'map';
  } else if (selfSyntax === 'headers') {
    // Object array: [[id: [Kind]]]
    const arrayKind = objSyntax['__array_kind'] || '';
    if (arrayKind) {
      fieldTypeHint = `[${arrayKind}]`;
    }
  }

  // Build the ID part (only if has explicit ID or has kind)
  if (fieldTypeHint && headingId) {
    // Pattern: [[id: text]] or [[id: array]] for standalone field types
    parts.push(`[[${headingId}: ${fieldTypeHint}]]`);
  } else if (kind && !label) {
    // Pattern: [[: Kind]] (no label, no explicit ID)
    parts.push(`[[:${kind}]]`);
  } else if (kind && headingId) {
    // Pattern: [[id: Kind]] or Label [[id: Kind]]
    parts.push(`[[${headingId}: ${kind}]]`);
  } else if (hasExplicitId && headingId) {
    // Pattern: [[id]] or Label [[id]] (only if explicit)
    parts.push(`[[${headingId}]]`);
  }
  // else: no [[id]] - heading without explicit ID

  const headingPrefix = '#'.repeat(actualLevel);
  return `${headingPrefix} ${parts.join(' ')}`;
}
