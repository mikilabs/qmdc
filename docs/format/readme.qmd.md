# QMD.md Format Specification [[format:__Namespace]]

- concepts: [[#heading]], [[#object]], [[#field]], [[#data_type]], [[#reference]], [[#array]], [[#comment]], [[#workspace]], [[#dynamic_block]], [[#yaml_block]], [[#validation_errors]]

The QMD.md format specification: syntax for structured data in Markdown.

QMDC bridges the gap between structured and unstructured information. Markdown becomes a way to store and process structured data while remaining human-readable and editable. The format requires no special training — it uses familiar Markdown constructs with minimal additions (`[[id]]`, `[[id: Kind]]`, `[[:Kind]]`).

Core principles:

- Natural Markdown syntax using familiar constructs
- Minimal ceremony — only the essentials
- Readability over parse simplicity — humans should understand documents without a parser
- Expressive parity with YAML — anything expressible in YAML is expressible in QMD.md
