# QMD-81: parse exited 0 on a document it could not fully parse

## parse exit code ignored __ParsingError [[qmd81_parse_exit: Bug]]

GitHub [#11](https://github.com/mikilabs/qmdc/issues/11). `qmdc parse` returned 0 for a document whose
result held `__ParsingError` objects, in all three parsers. The agent guide teaches
`qmdc parse -i f.qmd.md > /dev/null || exit 1` as the syntax check, so a document that had lost a
value (an `invalid-key` map entry, a nested sub-item, two definitions in one heading) passed it.

- status: done
- priority: high
- category: cli
