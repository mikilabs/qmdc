# Ignore: where a line is anchored [[qmd73_anchoring: __Workspace]]

QMD-73 regression. A line with no slash except a trailing one matches at any
depth: `build/` hides `src/build`, `notes.qmd.md` hides `src/notes.qmd.md`.
A leading slash anchors the line to the directory of the ignore file:
`/top.qmd.md` hides the root `top.qmd.md` and keeps `src/top.qmd.md`.
