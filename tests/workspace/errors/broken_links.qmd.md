# Broken Links Test [[broken_links_test]]

Test for detecting broken links.

## Service A [[service_a:Service]]

- name: Service A
- database: [[#nonexistent_db]]
- config: [[#missing_config]]

The references `[[#nonexistent_db]]` and `[[#missing_config]]` point to nonexistent objects.

## Service B [[service_b:Service]]

- name: Service B
- dependency: [[#other_namespace:missing]]

A reference to a nonexistent object in a nonexistent namespace.

## Expected Errors [[errors:json]]

```json example
[
  {
    "type": "broken_link",
    "file": "broken_links.qmd.md",
    "object": "service_a",
    "field": "database",
    "reference": "[[#nonexistent_db]]",
    "message": "Object 'nonexistent_db' not found"
  },
  {
    "type": "broken_link",
    "file": "broken_links.qmd.md",
    "object": "service_a",
    "field": "config",
    "reference": "[[#missing_config]]",
    "message": "Object 'missing_config' not found"
  },
  {
    "type": "broken_link",
    "file": "broken_links.qmd.md",
    "object": "service_b",
    "field": "dependency",
    "reference": "[[#other_namespace:missing]]",
    "message": "Namespace 'other_namespace' not found"
  }
]
```

