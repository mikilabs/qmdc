# QMD-97: Findings

## The card reads the raw bullet text [[qmd97_finding_cause: Finding]]

`_render_object_props_card()` in `qmdc-mkdocs/qmdc_mkdocs/converter.py` re-reads the page object's
`- key: value` bullets with `_PROPS_BULLET_RE` and passes the source text to `_props_card_html()`.
The parser's scalar rule in `qmdc-py/qmdc/parsers/field.py` drops one pair of matching `"` or `'`
quotes that wraps the whole value (`docs/format/types.qmd.md`: quoted values are always strings),
and the card never applied it. The fix applies the same rule to each card value in
`_unquote_scalar`. Taking the value from the parsed object instead, the issue's other option, would
lose the resolved reference links the card renders from the text, so the text path stays.

- category: bug
- related_to: [[#qmd97_props_card_quotes]]
- affected_files: [qmdc-mkdocs/qmdc_mkdocs/converter.py]
- solution: `_unquote_scalar` applied to each props-card value.
