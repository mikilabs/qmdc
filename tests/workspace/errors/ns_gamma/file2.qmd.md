# Gamma File 2 [[gamma_file2]]

## Config [[config:Config]]

- name: config
- timeout: 60
- description: Duplicate Config in file2 — ERROR!

This object has the same Kind:Id (`Config:config`) as in file1.qmd.md.

## Expected Errors

```json
[
  {
    "type": "duplicate_id",
    "global_id": "gamma:Config:config",
    "files": ["ns_gamma/file1.qmd.md", "ns_gamma/file2.qmd.md"],
    "message": "Duplicate Kind:Id 'Config:config' in namespace 'gamma'",
    "severity": "error"
  }
]
```

