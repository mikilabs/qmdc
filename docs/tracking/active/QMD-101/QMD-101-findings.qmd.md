# QMD-101: Findings

## The default fit has no lower bound [[qmd101_finding_cause: Finding]]

`fit()` in `qmdc-mkdocs/qmdc_mkdocs/templates/js/qmdc-mermaid-core.js` sets
`scale = Math.min(availWidth() / natW, 1)`, and the viewport calls it on load and on every
container resize. A diagram of any width is therefore shrunk to the column, however small its
labels become. The fit-to-width toolbar button calls the same function.

- category: bug
- related_to: [[#qmd101_mermaid_fit_floor]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/templates/js/qmdc-mermaid-core.js, qmdc-vscode/templates/qmdc-mermaid-core.js]
- solution: Clamp the default fit to a floor and keep the button's fit unclamped.

## A fixed 75% floor for the default view only [[qmd101_finding_fix: Finding]]

`fit(floor)` now clamps the scale to `Math.max(floor, ...)`. The view starts with
`fitFloor = MIN_FIT` (0.75), which keeps mermaid's 16 px labels at 12 px and lies in the
60-75% range the issue suggests; the resize re-fit uses the same floor. The fit-to-width
button and the `0` key set `fitFloor = 0`, so they still show the whole diagram. A diagram
narrower than the column stays at 100%. The wrapper already scrolls horizontally, so the page
does not widen. The floor is a constant: an override (a `data-` attribute or a plugin option)
would be new configuration and is left for a separate request. The core is shared with the
VS Code preview and was copied with `make mermaid-sync`; two preview e2e tests that pinned the
old fill-the-column default were updated to the new behaviour.

- category: design
- related_to: [[#qmd101_mermaid_fit_floor]]
- solution: `MIN_FIT` and `fitFloor` in the shared core; core tests in `tests/test_mermaid.py`; preview e2e tests, README and UI test plan updated.
