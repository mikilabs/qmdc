# Ignore: a negated line re-includes a path [[qmd73_negation: __Workspace]]

QMD-73 regression. `keep/*` hides both files in `keep/`, and the negated line
that follows brings `keep/important.qmd.md` back. The directory `keep/` itself
is not excluded, so git allows the re-include. `outside.qmd.md` is named by
neither line and stays.
