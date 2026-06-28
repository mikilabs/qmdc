#!/usr/bin/env bash
# Repeatable terminal-demo pipeline + shared helpers in one file.
#
#   <name>.demo.sh → <name>.cast → <name>.gif → <name>.mp4   (all in docs/.assets/)
#
# Two roles:
#   1. SOURCED by a content script (docs/.assets/<name>.demo.sh) to get the demo helpers
#      (note / run_live / run_shown / mcp_request / mcp_capture, and `qmdc`).
#   2. EXECUTED as the pipeline runner. Recording (build the .cast) is separate from
#      processing (render .gif / .mp4), so the script and the rendering iterate independently.
#      Add a new demo by dropping a docs/.assets/<name>.demo.sh — no changes here.
#
# Usage:
#   scripts/demos/demo.sh record [name]   # run <name>.demo.sh under asciinema → .cast
#   scripts/demos/demo.sh gif    [name]   # .cast → .gif
#   scripts/demos/demo.sh mp4    [name]   # .cast → .mp4 (high quality, H.264)
#   scripts/demos/demo.sh render [name]   # gif + mp4
#   scripts/demos/demo.sh all    [name]   # record + render   (default action)
#
#   name defaults to "quickstart".  e.g.:  scripts/demos/demo.sh all example
set -e

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$DEMO_DIR/../.." && pwd)"

# ───────────────────────── shared demo helpers (also used when sourced) ──────────────────
# The repo CLI, exposed as `qmdc` so typed commands read naturally.
qmdc() { "$REPO/qmdc" "$@"; }

# Tunables (override via env before sourcing/running). Pause values are in centiseconds
# so the math stays pure-bash integer (no awk/float-quoting fragility).
TYPE_SPEED="${TYPE_SPEED:-0.02}"             # seconds per typed character (sleep arg)
NOTE_PAUSE="${NOTE_PAUSE:-1.1}"              # pause after a narration line (sleep arg)
PAUSE_BASE_CS="${PAUSE_BASE_CS:-170}"        # reading pause = BASE + PER_LINE*lines, capped
PAUSE_PER_LINE_CS="${PAUSE_PER_LINE_CS:-28}"
PAUSE_MAX_CS="${PAUSE_MAX_CS:-600}"          # 6.00s — keep in sync with agg's IDLE_CAP

# Cyan narration line explaining the "why" before a command.
note() { printf '\033[1;36m# %s\033[0m\n' "$1"; sleep "$NOTE_PAUSE"; }

_type() {  # type $1 one character at a time, then newline
  local s="$1" i
  for ((i = 0; i < ${#s}; i++)); do printf '%s' "${s:$i:1}"; sleep "$TYPE_SPEED"; done
  printf '\n'
}

# Reading pause scaled to how much output just appeared (one newline ≈ one line).
auto_pause() {
  local n cs
  n=$(printf '%s\n' "$1" | wc -l | tr -d ' ')
  cs=$(( PAUSE_BASE_CS + PAUSE_PER_LINE_CS * n ))
  (( cs > PAUSE_MAX_CS )) && cs=$PAUSE_MAX_CS
  sleep "$(( cs / 100 )).$(printf '%02d' "$(( cs % 100 ))")"
}

# Type a command, run it live (output is instant), print it, pause proportionally.
run_live() {
  printf '\033[32m$\033[0m '; _type "$1"
  local out; out="$(eval "$1" 2>&1)"
  printf '%s\n\n' "$out"
  auto_pause "$out"
}

# Type a command, then print pre-captured output instantly, pause proportionally.
run_shown() {
  printf '\033[32m$\033[0m '; _type "$1"
  printf '%s\n\n' "$2"
  auto_pause "$2"
}

# Build a one-shot MCP JSON-RPC request file (initialize + initialized + one tools/call).
mcp_request() {  # $1=tool  $2=args-json  $3=outfile
  {
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"agent","version":"1"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}"
  } > "$3"
}

# Run an MCP request file through `qmdc mcp` and extract the tool payload with jq.
# stdin is held open briefly so the stdio server flushes before EOF (a piping quirk,
# not real latency) — pre-capture so the recording stays instant.
mcp_capture() {  # $1=reqfile  $2=jq-filter applied to the decoded tool payload
  { cat "$1"; sleep 0.6; } | qmdc mcp 2>/dev/null \
    | jq "select(.id==2).result.content[0].text | fromjson | $2"
}

# If sourced (by a content script), stop here — expose helpers only.
[[ "${BASH_SOURCE[0]}" != "${0}" ]] && return 0

# ───────────────────────────────── pipeline runner ──────────────────────────────────────
set -uo pipefail

ASSETS="$REPO/docs/.assets"
SELF="$DEMO_DIR/$(basename "${BASH_SOURCE[0]}")"
ACTION="${1:-all}"
NAME="${2:-quickstart}"
COLS="${COLS:-92}"
ROWS="${ROWS:-32}"
IDLE_CAP="${IDLE_CAP:-6}"

SCRIPT="$ASSETS/$NAME.demo.sh"
CAST="$ASSETS/$NAME.cast"
GIF="$ASSETS/$NAME.gif"
MP4="$ASSETS/$NAME.mp4"

require() { command -v "$1" >/dev/null 2>&1 || { echo "error: '$1' not found (brew install $1)" >&2; exit 1; }; }

cmd_record() {
  require asciinema
  [ -f "$SCRIPT" ] || { echo "error: demo script not found: $SCRIPT" >&2; exit 1; }
  mkdir -p "$ASSETS"
  # Source this file first so the content script gets the helpers (note/run_live/…/qmdc)
  # without any boilerplate of its own, then run the content script.
  asciinema rec --headless --overwrite --cols "$COLS" --rows "$ROWS" \
    -c "bash -c 'source \"$SELF\"; source \"$SCRIPT\"'" "$CAST"
  echo "recorded → $CAST"
}

cmd_gif() {
  require agg
  [ -f "$CAST" ] || { echo "error: no cast yet — run: $0 record $NAME" >&2; exit 1; }
  agg --theme asciinema --font-size 18 --idle-time-limit "$IDLE_CAP" "$CAST" "$GIF"
  echo "gif → $GIF ($(du -h "$GIF" | cut -f1))"
}

cmd_mp4() {
  # High quality: render a large, crisp gif first, then encode to H.264.
  require agg; require ffmpeg
  [ -f "$CAST" ] || { echo "error: no cast yet — run: $0 record $NAME" >&2; exit 1; }
  local tmp; tmp="$(mktemp -t qmdc-demo-XXXX).gif"
  agg --theme asciinema --font-size 28 --idle-time-limit "$IDLE_CAP" "$CAST" "$tmp"
  ffmpeg -y -loglevel error -i "$tmp" \
    -movflags +faststart -pix_fmt yuv420p \
    -vf "scale=trunc(iw/2)*2:trunc(ih/2)*2,fps=30" \
    -c:v libx264 -crf 18 -preset slow "$MP4"
  rm -f "$tmp"
  echo "mp4 → $MP4 ($(du -h "$MP4" | cut -f1))"
}

cmd_render() { cmd_gif; cmd_mp4; }
cmd_all() { cmd_record; cmd_render; }

case "$ACTION" in
  record) cmd_record ;;
  gif)    cmd_gif ;;
  mp4)    cmd_mp4 ;;
  render) cmd_render ;;
  all)    cmd_all ;;
  *) echo "usage: $0 {record|gif|mp4|render|all} [name]" >&2; exit 1 ;;
esac
