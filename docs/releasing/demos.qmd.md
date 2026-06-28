# Recording Demos [[release_demos: NarrativeDoc]]

- about: [[#ssg]], [[#quickstart]], [[#mcp]]

How the terminal demos shown across these docs (e.g. the [[#quickstart]] on the home page)
are produced: a small, repeatable pipeline that turns a content script into a `.cast`, then
a `.gif` and a high-quality `.mp4`. Built on [asciinema](https://asciinema.org/) + `agg` +
`ffmpeg`.

## Pipeline [[demos_pipeline: text]]

One engine script drives every demo:

```text
docs/.assets/<name>.demo.sh  ──record──▶  <name>.cast  ──render──▶  <name>.gif + <name>.mp4
```

All artifacts share a stem and live next to each other in `docs/.assets/`. Recording (build
the `.cast`) is separate from rendering (produce `.gif` / `.mp4`), so you can iterate on the
script and on the output formats independently.

```bash
scripts/demos/demo.sh record [name]   # run <name>.demo.sh under asciinema → .cast
scripts/demos/demo.sh gif    [name]   # .cast → .gif
scripts/demos/demo.sh mp4    [name]   # .cast → .mp4 (H.264, high quality)
scripts/demos/demo.sh render [name]   # gif + mp4
scripts/demos/demo.sh all    [name]   # record + render   (default; name defaults to quickstart)
```

Install the tools once: `brew install asciinema agg ffmpeg` (and `jq` for MCP demos).

## Add a New Demo [[demos_add: text]]

Drop a single content script `docs/.assets/<name>.demo.sh` — no boilerplate. The runner
sources `scripts/demos/demo.sh` into it first, so the helpers below are already available.
Then run `scripts/demos/demo.sh all <name>`.

A minimal example:

```bash
cd "$(mktemp -d)"                       # work in a throwaway dir
echo '# Demo [[demo: __Workspace]]' > readme.qmd.md

note "A heading is an object, list items are fields:"
run_live "cat readme.qmd.md"

note "Query it like a database:"
run_live 'qmdc query . "SELECT __id, __kind FROM objects"'
```

## Helpers [[demos_helpers: text]]

Provided by `scripts/demos/demo.sh` when sourced:

- `note "text"` — a cyan narration line (the "why" before a command).
- `run_live "cmd"` — type the command, run it live, print output, then pause.
- `run_shown "cmd" "$OUTPUT"` — type the command but print pre-captured output instantly
  (used for MCP, whose stdio server needs stdin held open briefly to flush — a piping
  quirk, not real latency).
- `qmdc …` — the repo CLI, so typed commands read as plain `qmdc`.
- `mcp_request <tool> <args-json> <file>` / `mcp_capture <file> <jq-filter>` — build and run
  a one-shot [[#mcp]] JSON-RPC `tools/call` and extract the payload with `jq`.

Tunables (env): `TYPE_SPEED`, `NOTE_PAUSE`, and the auto-pause `PAUSE_BASE_CS` /
`PAUSE_PER_LINE_CS` / `PAUSE_MAX_CS` (centiseconds). The reading pause after each command
scales with how many lines it printed, so dense output gets more time on screen.

## Embedding on the Site [[demos_embed: text]]

Reference the rendered gif from any page with plain Markdown — no attributes:

```markdown
![QMDC quickstart](.assets/quickstart.gif)
```

The SSG ([[#ssg_cmd_build]]) copies every referenced image into the built site and rewrites
the link (so a source in the hidden `.assets/` dir still ships), and the glightbox plugin
makes it click-to-zoom. Keep the `.demo.sh` / `.cast` / `.mp4` in `.assets/` too; they are
never published (the dir is hidden, and only referenced media is copied). For an in-editor
equivalent, see the extension's preview command [[#ext_cmd_preview]].

## Tips [[demos_tips: text]]

- **Keep output on screen** — pipe long output through `head` so the command and a sample
  of its result both stay visible; the demo's auto-pause already scales with line count.
- **Instant feel** — every `qmdc` call is ~10–50ms; pre-capture MCP output with
  `mcp_capture` so the recording shows the true speed instead of the stdio keep-alive.
- **Re-render only** — after tweaking themes or sizes, run `demo.sh render <name>` without
  re-recording; the `.cast` is the stable intermediate.
