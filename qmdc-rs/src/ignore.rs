//! `.qmdcignore` matching with git's `.gitignore` semantics (QMD-73).
//!
//! This is a port of git's own matcher, not an approximation of it: `dowild` from git's
//! `wildmatch.c`, and `match_basename`, `match_pathname`, the line parsing and the
//! "an excluded directory hides everything below it" walk from git's `dir.c`.
//!
//! The Python parser (`qmdc-py/qmdc/ignore.py`) and the TypeScript parser
//! (`qmdc-ts/src/ignore.ts`) carry the same port, function for function. Keep the three
//! in step: a change here needs the same change there. The shared acceptance data is
//! `tests/ignore/gitignore-matrix.json`, whose expected answers were produced by git
//! itself.
//!
//! Matching works on UTF-8 bytes, as git does, and is case-sensitive, as git is with
//! `core.ignorecase=false`, on every OS.

use std::fs;
use std::path::Path;

// dowild() results, as in git's wildmatch.h.
const WM_MATCH: i32 = 0;
const WM_NOMATCH: i32 = 1;
const WM_ABORT_ALL: i32 = -1;
const WM_ABORT_TO_STARSTAR: i32 = -2;

// dowild() flag: `*`, `?` and a bracket expression never match `/`.
const WM_PATHNAME: u32 = 1;

/// git's `is_glob_special()`: the characters that end a pattern's literal prefix.
fn is_glob_special(c: u8) -> bool {
    matches!(c, b'*' | b'?' | b'[' | b'\\')
}

/// The character classes of git's own `sane_ctype` table (ASCII only). Note that git's
/// `isspace()` is tab, newline, carriage return and space: vertical tab and form feed are
/// not spaces there. `None` for a class name git does not know.
fn char_class(name: &[u8], c: u8) -> Option<bool> {
    let digit = c.is_ascii_digit();
    let upper = c.is_ascii_uppercase();
    let lower = c.is_ascii_lowercase();
    let alpha = upper || lower;
    let alnum = alpha || digit;
    let graph = (0x21..=0x7e).contains(&c);
    Some(match name {
        b"alnum" => alnum,
        b"alpha" => alpha,
        b"blank" => c == 0x09 || c == 0x20,
        b"cntrl" => c < 0x20 || c == 0x7f,
        b"digit" => digit,
        b"graph" => graph,
        b"lower" => lower,
        b"print" => (0x20..=0x7e).contains(&c),
        b"punct" => graph && !alnum,
        b"space" => matches!(c, 0x09 | 0x0a | 0x0d | 0x20),
        b"upper" => upper,
        b"xdigit" => digit || (0x41..=0x46).contains(&c) || (0x61..=0x66).contains(&c),
        _ => return None,
    })
}

/// Match pattern `p` against `text`: git's `dowild()`, without case folding.
fn dowild(p: &[u8], text: &[u8], flags: u32) -> i32 {
    let plen = p.len();
    let tlen = text.len();
    let mut pi = 0usize;
    let mut ti = 0usize;
    while pi < plen {
        let mut p_ch = p[pi];
        let t_ch = if ti < tlen { text[ti] } else { 0 };
        if ti >= tlen && p_ch != b'*' {
            return WM_ABORT_ALL;
        }
        if p_ch == b'\\' {
            // Literal match with the following character; a trailing backslash compares
            // against the end of the pattern and fails.
            pi += 1;
            p_ch = if pi < plen { p[pi] } else { 0 };
            if t_ch != p_ch {
                return WM_NOMATCH;
            }
        } else if p_ch == b'?' {
            if flags & WM_PATHNAME != 0 && t_ch == b'/' {
                return WM_NOMATCH;
            }
        } else if p_ch == b'*' {
            pi += 1;
            let match_slash;
            if pi < plen && p[pi] == b'*' {
                let prev = pi as isize - 2;
                pi += 1;
                while pi < plen && p[pi] == b'*' {
                    pi += 1;
                }
                let nxt = if pi < plen { p[pi] } else { 0 };
                if (prev < 0 || p[prev as usize] == b'/')
                    && (pi >= plen
                        || nxt == b'/'
                        || (nxt == b'\\' && pi + 1 < plen && p[pi + 1] == b'/'))
                {
                    // `**/` may match no directory at all: try the rest right here.
                    if nxt == b'/' && dowild(&p[pi + 1..], &text[ti..], flags) == WM_MATCH {
                        return WM_MATCH;
                    }
                    match_slash = true;
                } else {
                    match_slash = false;
                }
            } else {
                match_slash = flags & WM_PATHNAME == 0;
            }
            if pi >= plen {
                // A trailing `**` matches everything; a trailing `*` only when no slash is left.
                if !match_slash && text[ti..].contains(&b'/') {
                    return WM_NOMATCH;
                }
                return WM_MATCH;
            }
            if !match_slash && p[pi] == b'/' {
                // One star followed by a slash matches the rest of this directory name.
                match text[ti..].iter().position(|&c| c == b'/') {
                    None => return WM_NOMATCH,
                    // Both slashes are consumed by the shared step at the bottom of the loop.
                    Some(offset) => ti += offset,
                }
            } else {
                loop {
                    if ti >= tlen {
                        break;
                    }
                    if !is_glob_special(p[pi]) {
                        // Skip ahead to the next occurrence of the literal after the star.
                        let literal = p[pi];
                        while ti < tlen && (match_slash || text[ti] != b'/') {
                            if text[ti] == literal {
                                break;
                            }
                            ti += 1;
                        }
                        if ti >= tlen || text[ti] != literal {
                            return if match_slash {
                                WM_ABORT_ALL
                            } else {
                                WM_ABORT_TO_STARSTAR
                            };
                        }
                    }
                    let matched = dowild(&p[pi..], &text[ti..], flags);
                    if matched != WM_NOMATCH {
                        if !match_slash || matched != WM_ABORT_TO_STARSTAR {
                            return matched;
                        }
                    } else if !match_slash && text[ti] == b'/' {
                        return WM_ABORT_TO_STARSTAR;
                    }
                    ti += 1;
                }
                return WM_ABORT_ALL;
            }
        } else if p_ch == b'[' {
            pi += 1;
            p_ch = if pi < plen { p[pi] } else { 0 };
            if p_ch == b'^' {
                p_ch = b'!';
            }
            let negated = p_ch == b'!';
            if negated {
                pi += 1;
                p_ch = if pi < plen { p[pi] } else { 0 };
            }
            let mut prev_ch: u8 = 0;
            let mut matched_class = false;
            loop {
                if p_ch == 0 {
                    return WM_ABORT_ALL;
                }
                if p_ch == b'\\' {
                    pi += 1;
                    p_ch = if pi < plen { p[pi] } else { 0 };
                    if p_ch == 0 {
                        return WM_ABORT_ALL;
                    }
                    if t_ch == p_ch {
                        matched_class = true;
                    }
                } else if p_ch == b'-' && prev_ch != 0 && pi + 1 < plen && p[pi + 1] != b']' {
                    pi += 1;
                    p_ch = p[pi];
                    if p_ch == b'\\' {
                        pi += 1;
                        p_ch = if pi < plen { p[pi] } else { 0 };
                        if p_ch == 0 {
                            return WM_ABORT_ALL;
                        }
                    }
                    if prev_ch <= t_ch && t_ch <= p_ch {
                        matched_class = true;
                    }
                    p_ch = 0; // a range cannot start another range
                } else if p_ch == b'[' && pi + 1 < plen && p[pi + 1] == b':' {
                    let start = pi + 2;
                    pi = start;
                    while pi < plen && p[pi] != b']' {
                        pi += 1;
                    }
                    if pi >= plen {
                        return WM_ABORT_ALL;
                    }
                    let name_len = pi as isize - start as isize - 1;
                    if name_len < 0 || p[pi - 1] != b':' {
                        // No `:]`, so the `[` is an ordinary member and scanning resumes
                        // right after it.
                        pi = start - 2;
                        p_ch = b'[';
                        if t_ch == p_ch {
                            matched_class = true;
                        }
                    } else {
                        match char_class(&p[start..start + name_len as usize], t_ch) {
                            None => return WM_ABORT_ALL,
                            Some(true) => matched_class = true,
                            Some(false) => {}
                        }
                        p_ch = 0;
                    }
                } else if t_ch == p_ch {
                    matched_class = true;
                }
                prev_ch = p_ch;
                pi += 1;
                p_ch = if pi < plen { p[pi] } else { 0 };
                if p_ch == b']' {
                    break;
                }
            }
            if matched_class == negated || (flags & WM_PATHNAME != 0 && t_ch == b'/') {
                return WM_NOMATCH;
            }
        } else if t_ch != p_ch {
            return WM_NOMATCH;
        }
        pi += 1;
        ti += 1;
    }
    if ti < tlen {
        WM_NOMATCH
    } else {
        WM_MATCH
    }
}

/// One `.qmdcignore` line, parsed the way git's `parse_path_pattern()` parses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreRule {
    /// The pattern without its leading `!` and trailing `/`.
    pattern: Vec<u8>,
    /// A leading `!`: a match re-includes the path.
    negated: bool,
    /// A trailing `/`: the line matches directories only.
    dir_only: bool,
    /// No slash left in the pattern: it is matched against the last path component, at
    /// any depth (git's `PATTERN_FLAG_NODIR`).
    basename_only: bool,
    /// Length of the pattern before its first glob character (git's `nowildcardlen`).
    literal_prefix: usize,
}

/// The parsed lines of one `.qmdcignore`, in file order.
pub type IgnoreRules = Vec<IgnoreRule>;

fn simple_length(pattern: &[u8]) -> usize {
    pattern
        .iter()
        .position(|&c| is_glob_special(c))
        .unwrap_or(pattern.len())
}

/// git's `trim_trailing_spaces()`: drop trailing spaces unless escaped with `\`.
fn trim_trailing_spaces(line: &[u8]) -> &[u8] {
    let mut last_space: Option<usize> = None;
    let mut i = 0;
    while i < line.len() {
        match line[i] {
            b' ' => {
                if last_space.is_none() {
                    last_space = Some(i);
                }
            }
            b'\\' => {
                i += 1;
                if i >= line.len() {
                    return line;
                }
                last_space = None;
            }
            _ => last_space = None,
        }
        i += 1;
    }
    match last_space {
        Some(end) => &line[..end],
        None => line,
    }
}

fn parse_rule(line: &[u8]) -> Option<IgnoreRule> {
    let (negated, line) = match line.strip_prefix(b"!") {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let (dir_only, line) = match line.strip_suffix(b"/") {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    if line.is_empty() {
        // git keeps an empty pattern, and an empty pattern never matches a path.
        return None;
    }
    Some(IgnoreRule {
        pattern: line.to_vec(),
        negated,
        dir_only,
        basename_only: !line.contains(&b'/'),
        literal_prefix: simple_length(line),
    })
}

/// Parse `.qmdcignore` content as git parses a `.gitignore` file.
pub fn parse_qmdcignore(data: &[u8]) -> IgnoreRules {
    let data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    let mut rules = Vec::new();
    for line in data.split(|&c| c == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        if let Some(rule) = parse_rule(trim_trailing_spaces(line)) {
            rules.push(rule);
        }
    }
    rules
}

/// The rules of the `.qmdcignore` in `root_path`; `None` when it is absent or unreadable.
pub fn load_qmdcignore(root_path: &Path) -> Option<IgnoreRules> {
    let data = fs::read(root_path.join(".qmdcignore")).ok()?;
    Some(parse_qmdcignore(&data))
}

/// git's `match_pathname()` for an ignore file at the root: the whole relative path.
fn match_pathname(path: &[u8], pattern: &[u8], prefix: usize) -> bool {
    let (mut pattern, prefix) = match pattern.strip_prefix(b"/") {
        // A leading `/` is literal, so it is part of the prefix and `prefix >= 1` here.
        Some(rest) => (rest, prefix - 1),
        None => (pattern, prefix),
    };
    if path.is_empty() {
        return false;
    }
    let mut path = path;
    if prefix > 0 {
        if prefix > path.len() {
            return false;
        }
        if pattern[..prefix] != path[..prefix] {
            return false;
        }
        pattern = &pattern[prefix..];
        path = &path[prefix..];
        if pattern.is_empty() && path.is_empty() {
            return true;
        }
    }
    dowild(pattern, path, WM_PATHNAME) == WM_MATCH
}

/// git's `last_matching_pattern_from_list()`: the last line that matches wins.
fn last_match<'a>(rules: &'a [IgnoreRule], path: &[u8], is_dir: bool) -> Option<&'a IgnoreRule> {
    let basename = match path.iter().rposition(|&c| c == b'/') {
        Some(slash) => &path[slash + 1..],
        None => path,
    };
    for rule in rules.iter().rev() {
        if rule.dir_only && !is_dir {
            continue;
        }
        if rule.basename_only {
            if dowild(&rule.pattern, basename, 0) == WM_MATCH {
                return Some(rule);
            }
            continue;
        }
        if match_pathname(path, &rule.pattern, rule.literal_prefix) {
            return Some(rule);
        }
    }
    None
}

fn excluded(rules: &[IgnoreRule], path: &[u8], is_dir: bool) -> bool {
    matches!(last_match(rules, path, is_dir), Some(rule) if !rule.negated)
}

/// Whether `rel_path` (relative to the ignore file, `/`-separated) is ignored.
///
/// As in git, a path below an excluded directory is ignored whatever later lines say, so
/// a negated line cannot bring back a file whose parent directory is excluded.
pub fn is_ignored_relative(rules: &[IgnoreRule], rel_path: &str, is_dir: bool) -> bool {
    if rules.is_empty() || rel_path.is_empty() {
        return false;
    }
    let path = rel_path.as_bytes();
    for (i, &c) in path.iter().enumerate() {
        if c == b'/' && excluded(rules, &path[..i], true) {
            return true;
        }
    }
    excluded(rules, path, is_dir)
}

/// Whether `path`, a file unless `is_dir`, is ignored by `rules` loaded from `root_path`.
pub fn is_ignored(
    path: &Path,
    root_path: &Path,
    rules: &Option<IgnoreRules>,
    is_dir: bool,
) -> bool {
    let Some(rules) = rules else {
        return false;
    };
    let Ok(rel) = path.strip_prefix(root_path) else {
        return false;
    };
    let rel = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    is_ignored_relative(rules, &rel, is_dir)
}
