/**
 * Header parser - extracts __id, __label, __kind from headings
 */

import type Token from 'markdown-it/lib/token';

export interface HeaderData {
  id: string;
  label: string;
  kind?: string;
  fieldType?: string; // "array" for [[field: array]], "object_array" for [[field: [Kind]]]
  arrayKind?: string; // Kind for [[field: [Kind]]]
  hasExplicitId?: boolean; // true if [[...]] was present in heading
  multipleDefinitions?: string[]; // Raw [[...]] strings when 2+ definitions found
}

// Seeded random number generator for deterministic fallback IDs
class SeededRandom {
  private seed: number;

  constructor(seed: number) {
    this.seed = seed;
  }

  next(): number {
    // Linear congruential generator
    this.seed = (this.seed * 1664525 + 1013904223) % 4294967296;
    return this.seed / 4294967296;
  }
}

let randomGen = new SeededRandom(666);

export function setRandomSeed(seed: number): void {
  randomGen = new SeededRandom(seed);
}

export function generateFallbackId(): string {
  const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
  let suffix = '';
  for (let i = 0; i < 6; i++) {
    suffix += chars[Math.floor(randomGen.next() * chars.length)];
  }
  return `object_${suffix}`;
}

/**
 * Parse heading tokens to extract object metadata
 *
 * Patterns:
 * - [[id]]                    -> __id=id, __label from text
 * - [[id: Kind]]              -> __id=id, __kind=Kind, __label from text
 * - [[:Kind]]                 -> __kind=Kind, __id from snake_case(label)
 * - Label [[id]]              -> __id=id, __label=Label
 * - Label                     -> __id from snake_case(Label), __label=Label
 */
export function parseHeader(tokens: Token[], startIdx = 0): HeaderData | null {
  // Find heading_open and inline tokens
  let inlineToken: Token | undefined = undefined;

  for (let i = startIdx; i < Math.min(startIdx + 3, tokens.length); i++) {
    const token = tokens[i];
    if (token && token.type === 'inline') {
      inlineToken = token;
      break;
    }
  }

  if (!inlineToken || !inlineToken.content) {
    return null;
  }

  const content = inlineToken.content.trim();

  // Extract [[...]] patterns
  // Pattern: [[id]], [[id: Kind]], [[:Kind]], [[]], [[field: [Kind]]]
  // Use balanced matching for nested brackets
  const bracketPattern = /\[\[((?:[^\[\]]|\[[^\]]*\])*)\]\]/g;

  // Strip backtick-escaped content before matching [[...]] patterns
  // so that `[[id]]` inside backticks is not treated as a definition
  const searchContent = content.replace(/`[^`]+`/g, (m) => ' '.repeat(m.length));
  const matches = Array.from(searchContent.matchAll(bracketPattern));

  const result: HeaderData = { id: '', label: '' };

  if (matches.length > 0) {
    // Detect multiple definitions
    if (matches.length > 1) {
      result.multipleDefinitions = matches.map((m) =>
        content.substring(m.index!, m.index! + m[0].length)
      );
    }

    // Remove [[...]] from content to get label (use match positions from searchContent)
    let label = content;
    for (let mi = matches.length - 1; mi >= 0; mi--) {
      const match = matches[mi]!;
      const start = match.index!;
      const end = start + match[0].length;
      label = label.substring(0, start) + label.substring(end);
    }
    // Clean up multiple spaces and trim
    label = label
      .split(/\s+/)
      .filter((s) => s)
      .join(' ')
      .trim();

    // Parse first [[...]] (extract bracket content from original content using positions)
    const firstMatch = matches[0];
    if (!firstMatch || firstMatch.index === undefined) {
      return null;
    }
    const bracketContent = content
      .substring(firstMatch.index + 2, firstMatch.index + firstMatch[0].length - 2)
      .trim();

    if (bracketContent.includes(':')) {
      // [[id: Kind]] or [[:Kind]] or [[field: array]]
      const parts = bracketContent.split(':', 2);
      const left = parts[0]?.trim() ?? '';
      const right = parts[1]?.trim() ?? '';

      if (right.toLowerCase() === 'array') {
        // [[field: array]] - primitive array
        result.id = left || generateFallbackId();
        result.fieldType = 'array';
      } else if (right.toLowerCase() === 'yaml') {
        // [[field: yaml]] - YAML block
        result.id = left || generateFallbackId();
        result.fieldType = 'yaml';
      } else if (right.toLowerCase() === 'json') {
        // [[field: json]] - JSON block
        result.id = left || generateFallbackId();
        result.fieldType = 'json';
      } else if (right.toLowerCase() === 'text') {
        // [[field: text]] - multiline text field
        result.id = left || generateFallbackId();
        result.fieldType = 'text';
      } else if (right.toLowerCase() === 'map') {
        // [[field: map]] - key-value map field (str→str)
        result.id = left || generateFallbackId();
        result.fieldType = 'map';
      } else if (right.startsWith('[') && right.endsWith(']')) {
        // [[field: [Kind]]] - object array
        const kindName = right.slice(1, -1).trim();
        result.id = left || generateFallbackId();
        result.fieldType = 'object_array';
        result.arrayKind = kindName;
      } else if (left) {
        // [[id: Kind]]
        result.id = left;
        result.kind = right;
      } else {
        // [[:Kind]]
        result.kind = right;
        result.id = label ? snakeCase(label) : generateFallbackId();
      }
    } else {
      // [[id]]
      result.id = bracketContent || generateFallbackId();
    }

    // For [[...]] patterns, use label as-is (may be empty)
    result.label = label;
    result.hasExplicitId = true; // [[...]] was present
  } else {
    // No [[...]], just plain text - use content for both label and id
    result.label = content;
    result.id = snakeCase(content);
    result.hasExplicitId = false; // No [[...]] - ID was auto-generated
  }

  return result;
}

/**
 * Convert text to snake_case for auto-generated IDs
 * Supports Unicode letters (Cyrillic, etc.)
 */
function snakeCase(text: string): string {
  // Remove special chars except letters (including Unicode), digits, spaces, hyphens
  // \p{L} matches any Unicode letter
  let cleaned = text.replace(/[^\p{L}\p{N}\s-]/gu, '');
  cleaned = cleaned.replace(/[\s-]+/g, '_');
  const result = cleaned.toLowerCase().replace(/^_+|_+$/g, '');
  // Return fallback ID if result is empty (e.g., heading with only special chars)
  return result || generateFallbackId();
}
