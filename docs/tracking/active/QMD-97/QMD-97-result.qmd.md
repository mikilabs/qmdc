# QMD-97: Result

## The props card shows quoted values without their quotes [[qmd97_result: Result]]

A props-card value wholly wrapped in one matching pair of quotes is shown without them, matching
what the parser returns. Other values are unchanged.

- feature: [[#qmd97_props_card_quotes]]
- completed: 2026-11-26
- files_changed: [qmdc-mkdocs/qmdc_mkdocs/converter.py]
- tests_changed: [qmdc-mkdocs/tests/test_converter.py]

### Verification [[qmd97_result_verification: text]]

- about: [[#qmd97_result]]

- `TestPropsCardValues::test_quoted_values_lose_their_quotes` (double, single and empty quoted
  values) fails on the old code and passes after the fix;
  `test_partly_quoted_values_are_left_alone` passes before and after.
- The issue's reproduction with `qmdc-mkdocs init` and `build`: the card shows `2026-09-08`, the
  same value `qmdc query` returns; the URL and the `[[#repro]]` link render as before.
- `make mkdocs-test`, `make test-fast`, `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
