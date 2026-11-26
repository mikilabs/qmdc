# QMD-96: Findings

## Hover-only visibility and an overlay position [[qmd96_finding_cause: Finding]]

`qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-mermaid.css` sets `.mermaid-toolbar` to
`position: absolute; top: 8px; right: 8px; opacity: 0.35` and raises the opacity only on
`.mermaid:hover` or `:focus-within`; `.mermaid-btn` is 26x26 px. A touch screen gives neither
state before the first tap, so the toolbar stays faint, and on a narrow screen it covers nodes.

- category: bug
- related_to: [[#qmd96_touch_toolbar]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-mermaid.css]
- solution: Override the toolbar under `@media (hover: none)`.

## Touch-only rules, toolbar moved above the diagram [[qmd96_finding_fix: Finding]]

The issue's suggested `position: static` alone would put the toolbar below the diagram, because
`qmdc-mermaid-core.js` appends it after the viewport. Under `@media (hover: none)` the diagram
container becomes a flex column and the toolbar gets `order: -1` and `align-self: flex-end`, so
it sits in its own row above the diagram on the right, without a script change. Buttons there are
44x44 px (WCAG 2.5.5). Devices with hover keep the floating faint toolbar unchanged.

- category: design
- related_to: [[#qmd96_touch_toolbar]]
- solution: One `@media (hover: none)` block in `qmdc-mermaid.css`; stylesheet tests in `tests/test_mermaid.py`.
