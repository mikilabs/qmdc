# Ignore: a negated line cannot re-include below an excluded directory [[qmd73_no_reinclude: __Workspace]]

QMD-73 guard. `keep/` excludes the directory itself, and git does not look
inside an excluded directory, so the negated line that follows cannot bring
`keep/important.qmd.md` back: both files in `keep/` stay hidden. A matcher
that decides each file by the last matching line alone gets this wrong.
