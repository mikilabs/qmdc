# Ignore: a line that names a directory hides its contents [[qmd73_dir_match: __Workspace]]

QMD-73 regression. Neither line has a trailing slash or a glob, and both name
a directory: `generated` (a bare name) and `examples/sub` (a path). In git a
line that matches a directory excludes everything inside it, while a sibling
of the named directory stays.
