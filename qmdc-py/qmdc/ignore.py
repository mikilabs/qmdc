"""`.qmdcignore` matching with git's `.gitignore` semantics (QMD-73).

This is a port of git's own matcher, not an approximation of it: `dowild` from git's
`wildmatch.c`, and `match_basename`, `match_pathname`, the line parsing and the
"an excluded directory hides everything below it" walk from git's `dir.c`.

The Rust parser (`qmdc-rs/src/ignore.rs`) and the TypeScript parser
(`qmdc-ts/src/ignore.ts`) carry the same port, function for function. Keep the three
in step: a change here needs the same change there. The shared acceptance data is
`tests/ignore/gitignore-matrix.json`, whose expected answers were produced by git
itself.

Matching works on UTF-8 bytes, as git does, and is case-sensitive, as git is with
`core.ignorecase=false`, on every OS.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

# dowild() results, as in git's wildmatch.h.
_WM_MATCH = 0
_WM_NOMATCH = 1
_WM_ABORT_ALL = -1
_WM_ABORT_TO_STARSTAR = -2

# dowild() flag: `*`, `?` and a bracket expression never match `/`.
_WM_PATHNAME = 1

_SLASH = ord("/")
_STAR = ord("*")
_QUESTION = ord("?")
_OPEN = ord("[")
_CLOSE = ord("]")
_BACKSLASH = ord("\\")
_BANG = ord("!")
_CARET = ord("^")
_DASH = ord("-")
_COLON = ord(":")
_HASH = ord("#")
_SPACE = ord(" ")

# git's is_glob_special(): the characters that end a pattern's literal prefix.
_GLOB_SPECIAL = frozenset(b"*?[\\")


# The character classes of git's own `sane_ctype` table (ASCII only). Note that git's
# isspace() is tab, newline, carriage return and space: vertical tab and form feed are
# not spaces there.
def _is_digit(c: int) -> bool:
    return 0x30 <= c <= 0x39


def _is_upper(c: int) -> bool:
    return 0x41 <= c <= 0x5A


def _is_lower(c: int) -> bool:
    return 0x61 <= c <= 0x7A


def _is_alpha(c: int) -> bool:
    return _is_upper(c) or _is_lower(c)


def _is_alnum(c: int) -> bool:
    return _is_alpha(c) or _is_digit(c)


def _is_graph(c: int) -> bool:
    return 0x21 <= c <= 0x7E


_CHAR_CLASSES: dict[bytes, Callable[[int], bool]] = {
    b"alnum": _is_alnum,
    b"alpha": _is_alpha,
    b"blank": lambda c: c in (0x09, 0x20),
    b"cntrl": lambda c: c < 0x20 or c == 0x7F,
    b"digit": _is_digit,
    b"graph": _is_graph,
    b"lower": _is_lower,
    b"print": lambda c: 0x20 <= c <= 0x7E,
    b"punct": lambda c: _is_graph(c) and not _is_alnum(c),
    b"space": lambda c: c in (0x09, 0x0A, 0x0D, 0x20),
    b"upper": _is_upper,
    b"xdigit": lambda c: _is_digit(c) or 0x41 <= c <= 0x46 or 0x61 <= c <= 0x66,
}


def _dowild(p: bytes, text: bytes, flags: int) -> int:
    """Match pattern `p` against `text`: git's `dowild()`, without case folding."""
    pi = 0
    ti = 0
    plen = len(p)
    tlen = len(text)
    while pi < plen:
        p_ch = p[pi]
        t_ch = text[ti] if ti < tlen else 0
        if ti >= tlen and p_ch != _STAR:
            return _WM_ABORT_ALL
        if p_ch == _BACKSLASH:
            # Literal match with the following character; a trailing backslash compares
            # against the end of the pattern and fails.
            pi += 1
            p_ch = p[pi] if pi < plen else 0
            if t_ch != p_ch:
                return _WM_NOMATCH
        elif p_ch == _QUESTION:
            if (flags & _WM_PATHNAME) and t_ch == _SLASH:
                return _WM_NOMATCH
        elif p_ch == _STAR:
            pi += 1
            if pi < plen and p[pi] == _STAR:
                prev = pi - 2
                pi += 1
                while pi < plen and p[pi] == _STAR:
                    pi += 1
                nxt = p[pi] if pi < plen else 0
                if (prev < 0 or p[prev] == _SLASH) and (
                    pi >= plen
                    or nxt == _SLASH
                    or (nxt == _BACKSLASH and pi + 1 < plen and p[pi + 1] == _SLASH)
                ):
                    if nxt == _SLASH:
                        # `**/` may match no directory at all: try the rest right here.
                        matched = _dowild(p[pi + 1 :], text[ti:], flags)
                        if matched == _WM_MATCH:
                            return _WM_MATCH
                        # The one deliberate difference from git, which discards this result.
                        # The loop below tries the same rest only against shorter tails of
                        # this text, and a star loop stops at the first WM_ABORT_ALL because
                        # no shorter tail can match, so the loop could only end in
                        # WM_ABORT_ALL too. Redoing that failed work once per `**/` costs 2^k
                        # steps for k of them: git 2.44 takes 15 s on a line of 22 against a
                        # path 40 directories deep. Returning here gives git's answer in
                        # linear time.
                        if matched == _WM_ABORT_ALL:
                            return _WM_ABORT_ALL
                    match_slash = True
                else:
                    match_slash = False
            else:
                match_slash = not (flags & _WM_PATHNAME)
            if pi >= plen:
                # A trailing `**` matches everything; a trailing `*` only when no slash is left.
                if not match_slash and _SLASH in text[ti:]:
                    return _WM_NOMATCH
                return _WM_MATCH
            if not match_slash and p[pi] == _SLASH:
                # One star followed by a slash matches the rest of this directory name.
                slash = text.find(b"/", ti)
                if slash < 0:
                    return _WM_NOMATCH
                # Both slashes are consumed by the shared step at the bottom of the loop.
                ti = slash
            else:
                while True:
                    if ti >= tlen:
                        break
                    if p[pi] not in _GLOB_SPECIAL:
                        # Skip ahead to the next occurrence of the literal after the star.
                        p_ch = p[pi]
                        while ti < tlen and (match_slash or text[ti] != _SLASH):
                            if text[ti] == p_ch:
                                break
                            ti += 1
                        if ti >= tlen or text[ti] != p_ch:
                            return _WM_ABORT_ALL if match_slash else _WM_ABORT_TO_STARSTAR
                    matched = _dowild(p[pi:], text[ti:], flags)
                    if matched != _WM_NOMATCH:
                        if not match_slash or matched != _WM_ABORT_TO_STARSTAR:
                            return matched
                    elif not match_slash and text[ti] == _SLASH:
                        return _WM_ABORT_TO_STARSTAR
                    ti += 1
                return _WM_ABORT_ALL
        elif p_ch == _OPEN:
            pi += 1
            p_ch = p[pi] if pi < plen else 0
            if p_ch == _CARET:
                p_ch = _BANG
            negated = p_ch == _BANG
            if negated:
                pi += 1
                p_ch = p[pi] if pi < plen else 0
            prev_ch = 0
            matched_class = False
            while True:
                if p_ch == 0:
                    return _WM_ABORT_ALL
                if p_ch == _BACKSLASH:
                    pi += 1
                    p_ch = p[pi] if pi < plen else 0
                    if p_ch == 0:
                        return _WM_ABORT_ALL
                    if t_ch == p_ch:
                        matched_class = True
                elif p_ch == _DASH and prev_ch and pi + 1 < plen and p[pi + 1] != _CLOSE:
                    pi += 1
                    p_ch = p[pi]
                    if p_ch == _BACKSLASH:
                        pi += 1
                        p_ch = p[pi] if pi < plen else 0
                        if p_ch == 0:
                            return _WM_ABORT_ALL
                    if prev_ch <= t_ch <= p_ch:
                        matched_class = True
                    p_ch = 0  # a range cannot start another range
                elif p_ch == _OPEN and pi + 1 < plen and p[pi + 1] == _COLON:
                    start = pi + 2
                    pi = start
                    while pi < plen and p[pi] != _CLOSE:
                        pi += 1
                    if pi >= plen:
                        return _WM_ABORT_ALL
                    name_len = pi - start - 1
                    if name_len < 0 or p[pi - 1] != _COLON:
                        # No `:]`, so the `[` is an ordinary member and scanning resumes
                        # right after it.
                        pi = start - 2
                        p_ch = _OPEN
                        if t_ch == p_ch:
                            matched_class = True
                    else:
                        test = _CHAR_CLASSES.get(p[start : start + name_len])
                        if test is None:
                            return _WM_ABORT_ALL
                        if test(t_ch):
                            matched_class = True
                        p_ch = 0
                elif t_ch == p_ch:
                    matched_class = True
                prev_ch = p_ch
                pi += 1
                p_ch = p[pi] if pi < plen else 0
                if p_ch == _CLOSE:
                    break
            if matched_class == negated or ((flags & _WM_PATHNAME) and t_ch == _SLASH):
                return _WM_NOMATCH
        elif t_ch != p_ch:
            return _WM_NOMATCH
        pi += 1
        ti += 1
    return _WM_NOMATCH if ti < tlen else _WM_MATCH


@dataclass(frozen=True)
class IgnoreRule:
    """One `.qmdcignore` line, parsed the way git's `parse_path_pattern()` parses it."""

    # The pattern without its leading `!` and trailing `/`.
    pattern: bytes
    # A leading `!`: a match re-includes the path.
    negated: bool
    # A trailing `/`: the line matches directories only.
    dir_only: bool
    # No slash left in the pattern: it is matched against the last path component, at
    # any depth (git's PATTERN_FLAG_NODIR).
    basename_only: bool
    # Length of the pattern before its first glob character (git's nowildcardlen).
    literal_prefix: int


def _simple_length(pattern: bytes) -> int:
    for i, c in enumerate(pattern):
        if c in _GLOB_SPECIAL:
            return i
    return len(pattern)


def _trim_trailing_spaces(line: bytes) -> bytes:
    """git's `trim_trailing_spaces()`: drop trailing spaces unless escaped with `\\`."""
    last_space = -1
    i = 0
    n = len(line)
    while i < n:
        c = line[i]
        if c == _SPACE:
            if last_space < 0:
                last_space = i
        elif c == _BACKSLASH:
            i += 1
            if i >= n:
                return line
            last_space = -1
        else:
            last_space = -1
        i += 1
    return line[:last_space] if last_space >= 0 else line


def _parse_rule(line: bytes) -> IgnoreRule | None:
    negated = line[:1] == b"!"
    if negated:
        line = line[1:]
    dir_only = line[-1:] == b"/"
    if dir_only:
        line = line[:-1]
    if not line:
        # git keeps an empty pattern, and an empty pattern never matches a path.
        return None
    return IgnoreRule(
        pattern=line,
        negated=negated,
        dir_only=dir_only,
        basename_only=b"/" not in line,
        literal_prefix=_simple_length(line),
    )


def parse_qmdcignore(data: bytes) -> list[IgnoreRule]:
    """Parse `.qmdcignore` content as git parses a `.gitignore` file."""
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    rules: list[IgnoreRule] = []
    for line in data.split(b"\n"):
        if line.endswith(b"\r"):
            line = line[:-1]
        if not line or line[0] == _HASH:
            continue
        rule = _parse_rule(_trim_trailing_spaces(line))
        if rule is not None:
            rules.append(rule)
    return rules


def load_qmdcignore(root_path: Path) -> list[IgnoreRule]:
    """The rules of the `.qmdcignore` in `root_path`; none when it is absent or unreadable."""
    try:
        data = (Path(root_path) / ".qmdcignore").read_bytes()
    except OSError:
        return []
    return parse_qmdcignore(data)


def _match_pathname(path: bytes, pattern: bytes, prefix: int) -> bool:
    """git's `match_pathname()` for an ignore file at the root: the whole relative path."""
    if pattern[:1] == b"/":
        pattern = pattern[1:]
        prefix -= 1
    if not path:
        return False
    if prefix:
        if prefix > len(path):
            return False
        if pattern[:prefix] != path[:prefix]:
            return False
        pattern = pattern[prefix:]
        path = path[prefix:]
        if not pattern and not path:
            return True
    return _dowild(pattern, path, _WM_PATHNAME) == _WM_MATCH


def _last_match(rules: list[IgnoreRule], path: bytes, is_dir: bool) -> IgnoreRule | None:
    """git's `last_matching_pattern_from_list()`: the last line that matches wins."""
    basename = path.rsplit(b"/", 1)[-1]
    for rule in reversed(rules):
        if rule.dir_only and not is_dir:
            continue
        if rule.basename_only:
            if _dowild(rule.pattern, basename, 0) == _WM_MATCH:
                return rule
            continue
        if _match_pathname(path, rule.pattern, rule.literal_prefix):
            return rule
    return None


def _excluded(rules: list[IgnoreRule], path: bytes, is_dir: bool) -> bool:
    rule = _last_match(rules, path, is_dir)
    return rule is not None and not rule.negated


def is_ignored_relative(rules: list[IgnoreRule], rel_path: str, is_dir: bool) -> bool:
    """Whether `rel_path` (relative to the ignore file, `/`-separated) is ignored.

    As in git, a path below an excluded directory is ignored whatever later lines say, so
    a negated line cannot bring back a file whose parent directory is excluded.
    """
    if not rules or not rel_path:
        return False
    path = rel_path.encode("utf-8", "surrogateescape")
    segments = path.split(b"/")
    for depth in range(1, len(segments)):
        if _excluded(rules, b"/".join(segments[:depth]), True):
            return True
    return _excluded(rules, path, is_dir)


def is_ignored(path: Path, root_path: Path, rules: list[IgnoreRule], is_dir: bool = False) -> bool:
    """Whether `path`, a file unless `is_dir`, is ignored by `rules` loaded from `root_path`."""
    if not rules:
        return False
    try:
        rel = Path(path).relative_to(root_path)
    except ValueError:
        return False
    return is_ignored_relative(rules, "/".join(rel.parts), is_dir)
