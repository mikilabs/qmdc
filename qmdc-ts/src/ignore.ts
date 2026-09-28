/**
 * `.qmdcignore` matching with git's `.gitignore` semantics (QMD-73).
 *
 * This is a port of git's own matcher, not an approximation of it: `dowild` from git's
 * `wildmatch.c`, and `match_basename`, `match_pathname`, the line parsing and the
 * "an excluded directory hides everything below it" walk from git's `dir.c`.
 *
 * The Python parser (`qmdc-py/qmdc/ignore.py`) and the Rust parser
 * (`qmdc-rs/src/ignore.rs`) carry the same port, function for function. Keep the three in
 * step: a change here needs the same change there. The shared acceptance data is
 * `tests/ignore/gitignore-matrix.json`, whose expected answers were produced by git itself.
 *
 * Matching works on UTF-8 bytes, as git does, and is case-sensitive, as git is with
 * `core.ignorecase=false`, on every OS.
 */

import { readFileSync } from 'fs';
import { isAbsolute, join, relative, sep } from 'path';

// dowild() results, as in git's wildmatch.h.
const WM_MATCH = 0;
const WM_NOMATCH = 1;
const WM_ABORT_ALL = -1;
const WM_ABORT_TO_STARSTAR = -2;

// dowild() flag: `*`, `?` and a bracket expression never match `/`.
const WM_PATHNAME = 1;

const SLASH = 0x2f;
const STAR = 0x2a;
const QUESTION = 0x3f;
const OPEN = 0x5b;
const CLOSE = 0x5d;
const BACKSLASH = 0x5c;
const BANG = 0x21;
const CARET = 0x5e;
const DASH = 0x2d;
const COLON = 0x3a;
const HASH = 0x23;
const SPACE = 0x20;
const CR = 0x0d;
const LF = 0x0a;

const encoder = new TextEncoder();

/** git's `is_glob_special()`: the characters that end a pattern's literal prefix. */
function isGlobSpecial(c: number): boolean {
  return c === STAR || c === QUESTION || c === OPEN || c === BACKSLASH;
}

// The character classes of git's own `sane_ctype` table (ASCII only). Note that git's
// isspace() is tab, newline, carriage return and space: vertical tab and form feed are not
// spaces there.
const isDigit = (c: number): boolean => c >= 0x30 && c <= 0x39;
const isUpper = (c: number): boolean => c >= 0x41 && c <= 0x5a;
const isLower = (c: number): boolean => c >= 0x61 && c <= 0x7a;
const isAlpha = (c: number): boolean => isUpper(c) || isLower(c);
const isAlnum = (c: number): boolean => isAlpha(c) || isDigit(c);
const isGraph = (c: number): boolean => c >= 0x21 && c <= 0x7e;

const CHAR_CLASSES: Record<string, (c: number) => boolean> = {
  alnum: isAlnum,
  alpha: isAlpha,
  blank: (c) => c === 0x09 || c === 0x20,
  cntrl: (c) => c < 0x20 || c === 0x7f,
  digit: isDigit,
  graph: isGraph,
  lower: isLower,
  print: (c) => c >= 0x20 && c <= 0x7e,
  punct: (c) => isGraph(c) && !isAlnum(c),
  space: (c) => c === 0x09 || c === 0x0a || c === 0x0d || c === 0x20,
  upper: isUpper,
  xdigit: (c) => isDigit(c) || (c >= 0x41 && c <= 0x46) || (c >= 0x61 && c <= 0x66),
};

/** Match pattern `p` against `text`: git's `dowild()`, without case folding. */
function dowild(p: Uint8Array, text: Uint8Array, flags: number): number {
  const plen = p.length;
  const tlen = text.length;
  let pi = 0;
  let ti = 0;
  while (pi < plen) {
    let pCh = p[pi] as number;
    const tCh = ti < tlen ? (text[ti] as number) : 0;
    if (ti >= tlen && pCh !== STAR) {
      return WM_ABORT_ALL;
    }
    if (pCh === BACKSLASH) {
      // Literal match with the following character; a trailing backslash compares against
      // the end of the pattern and fails.
      pi += 1;
      pCh = pi < plen ? (p[pi] as number) : 0;
      if (tCh !== pCh) {
        return WM_NOMATCH;
      }
    } else if (pCh === QUESTION) {
      if (flags & WM_PATHNAME && tCh === SLASH) {
        return WM_NOMATCH;
      }
    } else if (pCh === STAR) {
      pi += 1;
      let matchSlash: boolean;
      if (pi < plen && p[pi] === STAR) {
        const prev = pi - 2;
        pi += 1;
        while (pi < plen && p[pi] === STAR) {
          pi += 1;
        }
        const nxt = pi < plen ? (p[pi] as number) : 0;
        if (
          (prev < 0 || p[prev] === SLASH) &&
          (pi >= plen ||
            nxt === SLASH ||
            (nxt === BACKSLASH && pi + 1 < plen && p[pi + 1] === SLASH))
        ) {
          // `**/` may match no directory at all: try the rest right here.
          if (nxt === SLASH && dowild(p.subarray(pi + 1), text.subarray(ti), flags) === WM_MATCH) {
            return WM_MATCH;
          }
          matchSlash = true;
        } else {
          matchSlash = false;
        }
      } else {
        matchSlash = (flags & WM_PATHNAME) === 0;
      }
      if (pi >= plen) {
        // A trailing `**` matches everything; a trailing `*` only when no slash is left.
        if (!matchSlash && text.subarray(ti).includes(SLASH)) {
          return WM_NOMATCH;
        }
        return WM_MATCH;
      }
      if (!matchSlash && p[pi] === SLASH) {
        // One star followed by a slash matches the rest of this directory name.
        const slash = text.indexOf(SLASH, ti);
        if (slash < 0) {
          return WM_NOMATCH;
        }
        // Both slashes are consumed by the shared step at the bottom of the loop.
        ti = slash;
      } else {
        for (;;) {
          if (ti >= tlen) {
            break;
          }
          if (!isGlobSpecial(p[pi] as number)) {
            // Skip ahead to the next occurrence of the literal after the star.
            const literal = p[pi] as number;
            while (ti < tlen && (matchSlash || text[ti] !== SLASH)) {
              if (text[ti] === literal) {
                break;
              }
              ti += 1;
            }
            if (ti >= tlen || text[ti] !== literal) {
              return matchSlash ? WM_ABORT_ALL : WM_ABORT_TO_STARSTAR;
            }
          }
          const matched = dowild(p.subarray(pi), text.subarray(ti), flags);
          if (matched !== WM_NOMATCH) {
            if (!matchSlash || matched !== WM_ABORT_TO_STARSTAR) {
              return matched;
            }
          } else if (!matchSlash && text[ti] === SLASH) {
            return WM_ABORT_TO_STARSTAR;
          }
          ti += 1;
        }
        return WM_ABORT_ALL;
      }
    } else if (pCh === OPEN) {
      pi += 1;
      pCh = pi < plen ? (p[pi] as number) : 0;
      if (pCh === CARET) {
        pCh = BANG;
      }
      const negated = pCh === BANG;
      if (negated) {
        pi += 1;
        pCh = pi < plen ? (p[pi] as number) : 0;
      }
      let prevCh = 0;
      let matchedClass = false;
      for (;;) {
        if (pCh === 0) {
          return WM_ABORT_ALL;
        }
        if (pCh === BACKSLASH) {
          pi += 1;
          pCh = pi < plen ? (p[pi] as number) : 0;
          if (pCh === 0) {
            return WM_ABORT_ALL;
          }
          if (tCh === pCh) {
            matchedClass = true;
          }
        } else if (pCh === DASH && prevCh !== 0 && pi + 1 < plen && p[pi + 1] !== CLOSE) {
          pi += 1;
          pCh = p[pi] as number;
          if (pCh === BACKSLASH) {
            pi += 1;
            pCh = pi < plen ? (p[pi] as number) : 0;
            if (pCh === 0) {
              return WM_ABORT_ALL;
            }
          }
          if (prevCh <= tCh && tCh <= pCh) {
            matchedClass = true;
          }
          pCh = 0; // a range cannot start another range
        } else if (pCh === OPEN && pi + 1 < plen && p[pi + 1] === COLON) {
          const start = pi + 2;
          pi = start;
          while (pi < plen && p[pi] !== CLOSE) {
            pi += 1;
          }
          if (pi >= plen) {
            return WM_ABORT_ALL;
          }
          const nameLen = pi - start - 1;
          if (nameLen < 0 || p[pi - 1] !== COLON) {
            // No `:]`, so the `[` is an ordinary member and scanning resumes right after it.
            pi = start - 2;
            pCh = OPEN;
            if (tCh === pCh) {
              matchedClass = true;
            }
          } else {
            const name = String.fromCharCode(...p.subarray(start, start + nameLen));
            const test = Object.prototype.hasOwnProperty.call(CHAR_CLASSES, name)
              ? CHAR_CLASSES[name]
              : undefined;
            if (test === undefined) {
              return WM_ABORT_ALL;
            }
            if (test(tCh)) {
              matchedClass = true;
            }
            pCh = 0;
          }
        } else if (tCh === pCh) {
          matchedClass = true;
        }
        prevCh = pCh;
        pi += 1;
        pCh = pi < plen ? (p[pi] as number) : 0;
        if (pCh === CLOSE) {
          break;
        }
      }
      if (matchedClass === negated || (flags & WM_PATHNAME && tCh === SLASH)) {
        return WM_NOMATCH;
      }
    } else if (tCh !== pCh) {
      return WM_NOMATCH;
    }
    pi += 1;
    ti += 1;
  }
  return ti < tlen ? WM_NOMATCH : WM_MATCH;
}

/** One `.qmdcignore` line, parsed the way git's `parse_path_pattern()` parses it. */
export interface IgnoreRule {
  /** The pattern without its leading `!` and trailing `/`. */
  readonly pattern: Uint8Array;
  /** A leading `!`: a match re-includes the path. */
  readonly negated: boolean;
  /** A trailing `/`: the line matches directories only. */
  readonly dirOnly: boolean;
  /**
   * No slash left in the pattern: it is matched against the last path component, at any
   * depth (git's PATTERN_FLAG_NODIR).
   */
  readonly basenameOnly: boolean;
  /** Length of the pattern before its first glob character (git's nowildcardlen). */
  readonly literalPrefix: number;
}

function simpleLength(pattern: Uint8Array): number {
  for (let i = 0; i < pattern.length; i++) {
    if (isGlobSpecial(pattern[i] as number)) {
      return i;
    }
  }
  return pattern.length;
}

/** git's `trim_trailing_spaces()`: drop trailing spaces unless escaped with `\`. */
function trimTrailingSpaces(line: Uint8Array): Uint8Array {
  let lastSpace = -1;
  let i = 0;
  const n = line.length;
  while (i < n) {
    const c = line[i];
    if (c === SPACE) {
      if (lastSpace < 0) {
        lastSpace = i;
      }
    } else if (c === BACKSLASH) {
      i += 1;
      if (i >= n) {
        return line;
      }
      lastSpace = -1;
    } else {
      lastSpace = -1;
    }
    i += 1;
  }
  return lastSpace >= 0 ? line.subarray(0, lastSpace) : line;
}

function parseRule(line: Uint8Array): IgnoreRule | null {
  const negated = line.length > 0 && line[0] === BANG;
  if (negated) {
    line = line.subarray(1);
  }
  const dirOnly = line.length > 0 && line[line.length - 1] === SLASH;
  if (dirOnly) {
    line = line.subarray(0, line.length - 1);
  }
  if (line.length === 0) {
    // git keeps an empty pattern, and an empty pattern never matches a path.
    return null;
  }
  return {
    // A copy: `line` may be a view into the file's Buffer, whose `slice` is a view too.
    pattern: new Uint8Array(line),
    negated,
    dirOnly,
    basenameOnly: !line.includes(SLASH),
    literalPrefix: simpleLength(line),
  };
}

/** Parse `.qmdcignore` content as git parses a `.gitignore` file. */
export function parseQmdcignore(data: Uint8Array): IgnoreRule[] {
  if (data.length >= 3 && data[0] === 0xef && data[1] === 0xbb && data[2] === 0xbf) {
    data = data.subarray(3);
  }
  const rules: IgnoreRule[] = [];
  let start = 0;
  while (start <= data.length) {
    let end = data.indexOf(LF, start);
    if (end < 0) {
      end = data.length;
    }
    let line = data.subarray(start, end);
    start = end + 1;
    if (line.length > 0 && line[line.length - 1] === CR) {
      line = line.subarray(0, line.length - 1);
    }
    if (line.length === 0 || line[0] === HASH) {
      continue;
    }
    const rule = parseRule(trimTrailingSpaces(line));
    if (rule !== null) {
      rules.push(rule);
    }
  }
  return rules;
}

/** The rules of the `.qmdcignore` in `rootPath`; none when it is absent or unreadable. */
export function loadQmdcignore(rootPath: string): IgnoreRule[] {
  let data: Uint8Array;
  try {
    data = readFileSync(join(rootPath, '.qmdcignore'));
  } catch {
    return [];
  }
  return parseQmdcignore(data);
}

function startsWith(haystack: Uint8Array, needle: Uint8Array, length: number): boolean {
  for (let i = 0; i < length; i++) {
    if (haystack[i] !== needle[i]) {
      return false;
    }
  }
  return true;
}

/** git's `match_pathname()` for an ignore file at the root: the whole relative path. */
function matchPathname(path: Uint8Array, pattern: Uint8Array, prefix: number): boolean {
  if (pattern.length > 0 && pattern[0] === SLASH) {
    pattern = pattern.subarray(1);
    prefix -= 1;
  }
  if (path.length === 0) {
    return false;
  }
  if (prefix > 0) {
    if (prefix > path.length) {
      return false;
    }
    if (!startsWith(path, pattern, prefix)) {
      return false;
    }
    pattern = pattern.subarray(prefix);
    path = path.subarray(prefix);
    if (pattern.length === 0 && path.length === 0) {
      return true;
    }
  }
  return dowild(pattern, path, WM_PATHNAME) === WM_MATCH;
}

/** git's `last_matching_pattern_from_list()`: the last line that matches wins. */
function lastMatch(rules: IgnoreRule[], path: Uint8Array, isDir: boolean): IgnoreRule | null {
  const slash = path.lastIndexOf(SLASH);
  const basename = slash >= 0 ? path.subarray(slash + 1) : path;
  for (let i = rules.length - 1; i >= 0; i--) {
    const rule = rules[i] as IgnoreRule;
    if (rule.dirOnly && !isDir) {
      continue;
    }
    if (rule.basenameOnly) {
      if (dowild(rule.pattern, basename, 0) === WM_MATCH) {
        return rule;
      }
      continue;
    }
    if (matchPathname(path, rule.pattern, rule.literalPrefix)) {
      return rule;
    }
  }
  return null;
}

function excluded(rules: IgnoreRule[], path: Uint8Array, isDir: boolean): boolean {
  const rule = lastMatch(rules, path, isDir);
  return rule !== null && !rule.negated;
}

/**
 * Whether `relPath` (relative to the ignore file, `/`-separated) is ignored.
 *
 * As in git, a path below an excluded directory is ignored whatever later lines say, so a
 * negated line cannot bring back a file whose parent directory is excluded.
 */
export function isIgnoredRelative(rules: IgnoreRule[], relPath: string, isDir: boolean): boolean {
  if (rules.length === 0 || relPath === '') {
    return false;
  }
  const path = encoder.encode(relPath);
  for (let i = 0; i < path.length; i++) {
    if (path[i] === SLASH && excluded(rules, path.subarray(0, i), true)) {
      return true;
    }
  }
  return excluded(rules, path, isDir);
}

/** Whether `path`, a file unless `isDir`, is ignored by `rules` loaded from `rootPath`. */
export function isIgnored(
  path: string,
  rootPath: string,
  rules: IgnoreRule[],
  isDir = false
): boolean {
  if (rules.length === 0) {
    return false;
  }
  const rel = relative(rootPath, path);
  if (rel === '' || isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`)) {
    return false;
  }
  return isIgnoredRelative(rules, rel.split(sep).join('/'), isDir);
}
