# YAML Block [[yaml_block: SyntaxConcept]]

- depends: [[#field]], [[#object]]

## Description [[description: text]]

QMD.md supports embedded YAML and JSON blocks for migrating existing configurations. A block is defined via a heading-syntax field with type `yaml` or `json` containing a fenced code block.

## YAML Syntax [[yaml_syntax: text]]

````markdown example
### Configuration [[config: yaml]]

```yaml
database:
  host: localhost
  port: 5432
  credentials:
    user: admin
    password: secret
replicas: 3
```
````

Result: field `config` contains parsed YAML as a nested object. Data is accessible directly: `object.config.database.host`. The `__syntax` field records `yaml_object`.

## JSON Syntax [[json_syntax: text]]

````markdown example
### Configuration [[config: json]]

```json
{
  "database": {
    "host": "localhost",
    "port": 5432
  },
  "replicas": 3
}
```
````

Result: field `config` contains parsed JSON as a nested object. The `__syntax` field records `json_object`.

## Error Handling [[error_handling: text]]

If the YAML or JSON is invalid, the field is saved as a raw string. The graph continues loading (does not crash).

## Rules [[rules: text]]

- YAML blocks always have `__syntax: "yaml_object"`
- JSON blocks always have `__syntax: "json_object"`
- Invalid YAML/JSON is saved as a raw string (warning, not fatal)
- References `[[#id]]` inside YAML/JSON blocks are not currently parsed (deferred)
- YAML key order is preserved on rebuild (`sort_keys: false`)
