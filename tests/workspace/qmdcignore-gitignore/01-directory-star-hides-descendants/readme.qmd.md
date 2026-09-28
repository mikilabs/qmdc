# Ignore: a star that matches a directory hides its descendants [[qmd73_dir_star: __Workspace]]

QMD-73 regression. The repository root ignores its fixtures with the line
`tests/*`; this fixture uses the same shape as `examples/*`. In git the star
matches the directory `examples/sub` itself, and a matched directory hides
everything below it, so `examples/sub/mid.qmd.md` is ignored too.
