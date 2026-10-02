# Ignore: a star never crosses a slash [[qmd73_star_one_level: __Workspace]]

QMD-73 regression. `docs/*.qmd.md` names the files directly inside `docs/`.
In git a star never matches a slash and `docs/sub` is not matched by the
line, so `docs/sub/nested.qmd.md` stays.
