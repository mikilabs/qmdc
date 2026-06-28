## Finding [[finding2: Finding]]

- affected_files: [package.json]
- solution: |
    Add the following to `contributes.menus`:

    ```json
    "editor/title": [
      {
        "command": "qmd.openPreview",
        "group": "navigation"
      }
    ]
    ```

    This enables the preview button.
- severity: medium

