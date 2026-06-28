/**
 * Field parser - extracts fields from list items
 */

import type Token from 'markdown-it/lib/token';
import type { BlockTree } from '../block_tree.js';

/**
 * Parse YAML-style array like [a, b, c] or [1, 2, 3]
 *
 * Returns: [items_list, item_types, item_raw_tokens]
 * item_raw_tokens[idx] holds the raw source token for integer-valued float
 * literals (e.g. "1.0", "2.000") that JS collapses to an int; undefined for
 * every other element. Used to restore canonical `X.0` form in the JSON data
 * column without changing the parsed numeric value.
 */
function parseYamlArray(
  valueStr: string
): [unknown[], Record<string, string>, (string | undefined)[]] {
  // Remove brackets
  const inner = valueStr.slice(1, -1).trim();

  if (!inner) {
    return [[], {}, []];
  }

  const items: unknown[] = [];
  const types: Record<string, string> = {};
  const rawTokens: (string | undefined)[] = [];

  // Split by comma, handle quoted strings
  const parts = splitYamlArray(inner);

  for (let idx = 0; idx < parts.length; idx++) {
    const part = parts[idx];
    if (part !== undefined) {
      const [val, typeName, rawTok] = parseFieldValue(part);
      items.push(val);
      types[String(idx)] = typeName;
      rawTokens.push(rawTok);
    }
  }

  return [items, types, rawTokens];
}

/**
 * Split YAML array content by commas, respecting quotes
 */
function splitYamlArray(s: string): string[] {
  const result: string[] = [];
  let current = '';
  let inQuotes = false;
  let quoteChar = '';

  for (const char of s) {
    if ((char === '"' || char === "'") && !inQuotes) {
      inQuotes = true;
      quoteChar = char;
      current += char;
    } else if (char === quoteChar && inQuotes) {
      inQuotes = false;
      current += char;
      quoteChar = '';
    } else if (char === ',' && !inQuotes) {
      result.push(current.trim());
      current = '';
    } else {
      current += char;
    }
  }

  if (current.trim()) {
    result.push(current.trim());
  }

  return result;
}

/**
 * Parse field value and auto-detect type
 *
 * Returns: [value, type_name]
 *
 * Types:
 * - array → [array, "array"]
 * - null → [null, "null"]
 * - true/false → [boolean, "boolean"]
 * - number → [number, "number"]
 * - string → [string, "string"]
 */
export function parseFieldValue(
  valueStr: string
): [unknown, string, string | undefined, (string | undefined)[]?] {
  const value = valueStr.trim();

  // Empty array
  if (value === '[]') {
    return [[], 'array', undefined];
  }

  // Multiple comma-separated references: [[#a]], [[#b]], [[#c]]
  // NOT a YAML array (which would be [[[#a]], [[#b]]])
  if (value.startsWith('[[') && !value.startsWith('[[[') && value.includes(']], [[')) {
    const items: string[] = [];
    for (let part of value.split(']], [[')) {
      part = part.trim();
      // Restore brackets
      if (!part.startsWith('[[')) {
        part = '[[' + part;
      }
      if (!part.endsWith(']]')) {
        part = part + ']]';
      }
      items.push(part);
    }
    return [items, 'ref_array', undefined];
  }

  // YAML array [a, b, c] - but not single references [[#id]]
  // Array of refs [[[#id1]], [[#id2]]] should be parsed as array
  const isArrayBracket = value.startsWith('[') && value.endsWith(']');
  const isSingleRef = value.startsWith('[[') && !value.startsWith('[[[');
  if (isArrayBracket && !isSingleRef) {
    const [items, , itemRawTokens] = parseYamlArray(value);
    // Surface per-element raw float tokens only when at least one is present,
    // so callers can restore canonical `X.0` form in the JSON data column.
    const hasRaw = itemRawTokens.some((t) => t !== undefined);
    return [items, 'array', undefined, hasRaw ? itemRawTokens : undefined];
  }

  // null
  if (value === 'null') {
    return [null, 'null', undefined];
  }

  // boolean
  if (value === 'true') {
    return [true, 'boolean', undefined];
  }
  if (value === 'false') {
    return [false, 'boolean', undefined];
  }

  // number (int or float)
  if (/^-?\d+(\.\d+)?$/.test(value)) {
    const num = parseFloat(value);
    const raw = value.includes('.') && Number.isInteger(num) ? value : undefined;
    return [num, 'number', raw];
  }

  // string (default) - remove quotes if present
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return [value.slice(1, -1), 'string', undefined];
  }

  return [value, 'string', undefined];
}

export interface InvalidFieldItem {
  key: string;
  content: string;
  line: number;
  after: string;
}

export interface NestedSubitemsError {
  key: string;
  line: number;
}

/**
 * Parse fields from markdown list starting at start_idx
 *
 * Returns: [fields_dict, types_dict, syntax_dict, invalid_items, next_index]
 */
export function parseFieldsFromList(
  tokens: Token[],
  startIdx: number,
  blockTree?: BlockTree,
  options?: { rawStrings?: boolean }
): [
  Record<string, unknown>,
  Record<string, string>,
  Record<string, string>,
  InvalidFieldItem[],
  number,
  Record<string, string>,
  NestedSubitemsError[],
] {
  const fields: Record<string, unknown> = {};
  const types: Record<string, string> = {};
  const syntax: Record<string, string> = {};
  const invalidItems: InvalidFieldItem[] = [];
  const nestedSubitemsErrors: NestedSubitemsError[] = [];
  const rawValues: Record<string, string> = {};
  let i = startIdx;

  // Pattern: `- key: value` or `- key:value`
  const fieldPattern = /^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$/;
  // Pattern for invalid field-like items (any text with colon and space)
  const invalidFieldLikePattern = /^([^:]+):\s+(.*)$/;
  // Valid key pattern
  const validKeyPattern = /^[a-zA-Z_][a-zA-Z0-9_]*$/;

  let lastValidField = '__self';

  while (i < tokens.length) {
    const token = tokens[i];
    if (!token) break;

    if (token.type === 'bullet_list_open') {
      i++;
      continue;
    }

    if (token.type === 'bullet_list_close') {
      i++;
      break;
    }

    if (token.type === 'list_item_open') {
      i++;
      continue;
    }

    if (token.type === 'list_item_close') {
      i++;
      continue;
    }

    if (token.type === 'paragraph_open') {
      i++;
      continue;
    }

    if (token.type === 'paragraph_close') {
      i++;
      continue;
    }

    if (token.type === 'inline') {
      // Parse field from inline content
      const content = token.content?.trim() ?? '';

      // Handle multiline content: markdown-it may include \n in inline tokens
      const firstLine = content.split('\n')[0] ?? '';
      const match = firstLine.match(fieldPattern);

      if (match) {
        const key = match[1];
        let valueStr = match[2] ?? '';
        if (key) {
          // Check for multiline YAML array: value starts with [ but doesn't close on same line
          let isMultilineArray = false;
          if (
            valueStr.trimStart().startsWith('[') &&
            !valueStr.trimEnd().endsWith(']') &&
            content.includes('\n')
          ) {
            isMultilineArray = true;
            // Join all lines, strip leading whitespace from continuation lines
            const joined = content
              .split('\n')
              .map((l) => l.trim())
              .join(' ');
            const fullMatch = joined.match(fieldPattern);
            if (fullMatch && fullMatch[2]) {
              valueStr = fullMatch[2].trim();
            }
          }

          // Check for YAML multiline: `field: |`
          // markdown-it may merge `field: |` with next line into one inline token,
          // so valueStr can be `"|"` or `"|\n  next line..."`.
          if (valueStr.trim() === '|' || valueStr.trim().startsWith('|\n')) {
            // Use raw-slice from BlockTree when available (preserves numbering, formatting)
            if (blockTree && token.map) {
              // Find the OUTER list_item_close (track nesting to skip inner ones)
              let scan = i + 1;
              let nesting = 0;
              while (scan < tokens.length) {
                const st = tokens[scan]?.type;
                if (st === 'ordered_list_open' || st === 'bullet_list_open') {
                  nesting++;
                } else if (st === 'ordered_list_close' || st === 'bullet_list_close') {
                  nesting--;
                } else if (st === 'list_item_close' && nesting === 0) {
                  break;
                }
                scan++;
              }
              const pipeLine = token.map[0]; // line of "- field: |"
              // End line: next list_item_open or bullet_list_close
              let endLine: number | undefined;
              for (let j = scan; j < Math.min(scan + 3, tokens.length); j++) {
                if (tokens[j]?.type === 'list_item_open') {
                  if (tokens[j]?.map) {
                    endLine = tokens[j]!.map![0];
                  }
                  break;
                }
                if (tokens[j]?.type === 'bullet_list_close') {
                  break;
                }
              }
              if (endLine === undefined) {
                // Last item — find bullet_list_close
                for (let j = scan; j < Math.min(scan + 3, tokens.length); j++) {
                  if (tokens[j]?.type === 'bullet_list_close') {
                    if (tokens[j]?.map) {
                      endLine = tokens[j]!.map![0];
                    }
                    break;
                  }
                }
              }
              if (endLine === undefined) {
                endLine = blockTree.lineCount;
              }

              // Extract raw content after the pipe line
              const raw = blockTree.getLinesRaw(pipeLine + 1, endLine);
              const rawStripped = raw.replace(/\n+$/, '');
              const rawLines = rawStripped.split('\n');
              if (rawLines.length > 0) {
                const indents = rawLines
                  .filter((ln) => ln.trim().length > 0)
                  .map((ln) => ln.length - ln.trimStart().length);
                const minIndent = indents.length > 0 ? Math.min(...indents) : 0;
                const dedented = rawLines.map((ln) =>
                  ln.length >= minIndent ? ln.slice(minIndent) : ln
                );
                fields[key] = dedented.join('\n');
              } else {
                fields[key] = '';
              }
              types[key] = 'string';
              syntax[key] = 'yaml_multiline';
              lastValidField = key;
              // Skip to list_item_close
              i = scan;
              continue;
            }

            // Fallback: collect from tokens
            const multilineParts: string[] = [];
            if (valueStr.trim().startsWith('|\n')) {
              const afterPipe = valueStr.trim().slice(2);
              const lines = afterPipe.split('\n');
              if (lines.length > 0) {
                const indents = lines
                  .filter((ln) => ln.trim().length > 0)
                  .map((ln) => ln.length - ln.trimStart().length);
                const minIndent = indents.length > 0 ? Math.min(...indents) : 0;
                const dedented = lines.map((ln) =>
                  ln.length >= minIndent ? ln.slice(minIndent) : ln
                );
                multilineParts.push(dedented.join('\n'));
              }
            }
            i++;
            let _nesting = 0;
            while (i < tokens.length) {
              const t = tokens[i];
              if (t?.type === 'list_item_close' && _nesting === 0) {
                break;
              }
              if (t?.type === 'fence') {
                // Code block - include with fences
                const lang = t.info || '';
                const fenceContent = (t.content || '').replace(/\n$/, '');
                if (lang) {
                  multilineParts.push(`\`\`\`${lang}\n${fenceContent}\n\`\`\``);
                } else {
                  multilineParts.push(`\`\`\`\n${fenceContent}\n\`\`\``);
                }
              } else if (t?.type === 'code_block') {
                // Indented code block
                multilineParts.push((t.content || '').replace(/\n$/, ''));
              } else if (t?.type === 'inline') {
                multilineParts.push(t.content || '');
              } else if (t?.type === 'ordered_list_open' || t?.type === 'bullet_list_open') {
                _nesting++;
              } else if (t?.type === 'ordered_list_close' || t?.type === 'bullet_list_close') {
                _nesting--;
              } else if (t?.type === 'list_item_open' || t?.type === 'list_item_close') {
                // skip nested list item boundaries
              }
              // Skip paragraph_open/close, list_item_open/close
              i++;
            }

            fields[key] = multilineParts.join('\n') || '';
            types[key] = 'string';
            syntax[key] = 'yaml_multiline';
            lastValidField = key;
            continue;
          }

          if (options?.rawStrings) {
            fields[key] = valueStr;
            types[key] = 'string';
            lastValidField = key;
          } else {
            const [value, typeName, rawStr, arrayRawTokens] = parseFieldValue(valueStr);
            fields[key] = value;
            types[key] = typeName === 'ref_array' ? 'array' : typeName;
            if (rawStr !== undefined) {
              rawValues[key] = rawStr;
            }
            // For arrays, record per-element raw float tokens under compound keys
            // (`key[idx]`). These never collide with real field keys (which match
            // /^[a-zA-Z_][a-zA-Z0-9_]*$/) and are read only by the JSON data-column
            // serializer to restore canonical `X.0` form; rebuild ignores them.
            if (arrayRawTokens) {
              for (let idx = 0; idx < arrayRawTokens.length; idx++) {
                const tok = arrayRawTokens[idx];
                if (tok !== undefined) {
                  rawValues[`${key}[${idx}]`] = tok;
                }
              }
            }
            lastValidField = key;

            // Track syntax for arrays
            if (typeName === 'ref_array') {
              syntax[key] = 'comma_refs';
            } else if (typeName === 'array') {
              // Detect multiline array: value spans multiple lines
              if (isMultilineArray) {
                syntax[key] = 'yaml_multiline_array';
              } else {
                syntax[key] = 'yaml_array';
              }
            }
          }

          // Detect nested sub-items: field with empty value followed by nested list
          // e.g. `- affected_files:\n  - item1\n  - item2`
          // This is a syntax error (nested_subitems) per spec rule_no_nested_subitems
          if (valueStr === '' && i + 1 < tokens.length) {
            // Look ahead for nested bullet_list_open or ordered_list_open before list_item_close
            let lookahead = i + 1;
            while (lookahead < tokens.length && tokens[lookahead]?.type === 'paragraph_close') {
              lookahead++;
            }
            const nestedType = tokens[lookahead]?.type;
            if (
              lookahead < tokens.length &&
              (nestedType === 'bullet_list_open' || nestedType === 'ordered_list_open')
            ) {
              const nestedClose = nestedType.replace('_open', '_close');
              // Skip past the nested list
              lookahead++; // skip list_open
              while (lookahead < tokens.length && tokens[lookahead]?.type !== nestedClose) {
                lookahead++;
              }
              if (lookahead < tokens.length) {
                lookahead++; // skip bullet_list_close
              }
              // Record as nested_subitems error
              nestedSubitemsErrors.push({
                key,
                line: token.map ? token.map[0] + 1 : 0,
              });
              // Remove the field we just added (it has empty value)
              delete fields[key];
              delete types[key];
              i = lookahead;
              continue;
            }
          }
        }
      } else {
        // Not a valid field - check if it looks like a field with invalid key
        const invalidMatch = firstLine.match(invalidFieldLikePattern);
        if (invalidMatch) {
          const potentialKey = (invalidMatch[1] ?? '').trim();
          if (potentialKey && !validKeyPattern.test(potentialKey)) {
            // Invalid field key (e.g. Cyrillic)
            const lineNum = token.map ? token.map[0] + 1 : 0;
            invalidItems.push({
              key: potentialKey,
              content: `- ${content}`,
              line: lineNum,
              after: lastValidField,
            });
          }
        } else if (lastValidField !== '__self' && content) {
          // Plain list item after a valid field — not a field at all.
          // Save as invalid item so it can be preserved in __comments.
          const lineNum = token.map ? token.map[0] + 1 : 0;
          invalidItems.push({
            key: '',
            content: `- ${content}`,
            line: lineNum,
            after: lastValidField,
          });
        }
      }

      i++;
      continue;
    }

    // Unknown token, skip
    i++;
  }

  return [fields, types, syntax, invalidItems, i, rawValues, nestedSubitemsErrors];
}

/**
 * Parse list items as array elements (no key: prefix)
 *
 * Used for [[field: array]] sections where list items are plain values.
 *
 * Returns: [items_list, next_index]
 */
export function parseArrayItemsFromList(tokens: Token[], startIdx: number): [unknown[], number] {
  const items: unknown[] = [];
  let i = startIdx;
  let nesting = 0;

  // Only bullet lists reach this function — ordered lists are intercepted
  // by the parser and emitted as ordered_list_in_array errors.
  while (i < tokens.length) {
    const token = tokens[i];
    if (!token) break;

    if (token.type === 'bullet_list_open' || token.type === 'ordered_list_open') {
      nesting++;
      i++;
      continue;
    }

    if (token.type === 'bullet_list_close' || token.type === 'ordered_list_close') {
      nesting--;
      if (nesting <= 0) {
        i++;
        break;
      }
      i++;
      continue;
    }

    if (token.type === 'list_item_open') {
      i++;
      continue;
    }

    if (token.type === 'list_item_close') {
      i++;
      continue;
    }

    if (token.type === 'paragraph_open') {
      i++;
      continue;
    }

    if (token.type === 'paragraph_close') {
      i++;
      continue;
    }

    if (token.type === 'inline') {
      const content = token.content?.trim() ?? '';
      const [value] = parseFieldValue(content);
      items.push(value);
      i++;
      continue;
    }

    // Unknown token, skip
    i++;
  }

  return [items, i];
}
