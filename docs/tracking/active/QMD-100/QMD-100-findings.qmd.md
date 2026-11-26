# QMD-100: Findings

## A bare 1fr track cannot shrink below its content [[qmd100_finding_cause: Finding]]

`qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-extra.css` lays a card row out as
`grid-template-columns: minmax(5rem, max-content) 1fr`, and on narrow screens `1fr`. A bare `1fr`
is `minmax(auto, 1fr)`, so its minimum is the value's min-content width, and `.qmdc-prop__val`
had no `min-width` or wrapping rule. An unbroken URL therefore set the track to its own width
(610 px in the issue's page) and widened the page. The fix is the issue's CSS: the value track is
`minmax(0, 1fr)` in both layouts, and the value gets `min-width: 0; overflow-wrap: anywhere`.
On a 1280 px desktop the value also overflowed the card before; it now wraps there too.

- category: bug
- related_to: [[#qmd100_props_overflow]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/templates/css/qmdc-extra.css]
- solution: `minmax(0, 1fr)` value track and `overflow-wrap: anywhere` on the value.
