# Walk-up Workspace [[walkup_ws: __Workspace]]

- description: A real workspace. Pointing the CLI at a SUBDIRECTORY of this dir must walk UP and resolve this workspace, not synthesize a virtual workspace named after the subdir.

## Overview [[walkup_overview: Section]]

This fixture covers QMD-59 walk-up parity: `query`/`validate` on `sub/` must
resolve `walkup_ws` in all three parsers.
