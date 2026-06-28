# Docs Workspace [[docs_ws: __Workspace]]

- description: The only real workspace, living inside a non-workspace container dir.

## Overview [[overview: Section]]

This workspace sits inside a parent directory that has NO `[[__Workspace]]` marker.
Parsing/validating the parent (container) directory must NOT report `nested_workspace`
for this workspace — the container is not a workspace, so this is not nesting.
