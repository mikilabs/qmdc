# Workspace Location Validation Test

This microtest verifies that `__Workspace` can only be defined in `readme.qmd.md` files.

## Test structure

- `ws-root/readme.qmd.md` — root workspace (valid)
- `ws-subdir/readme.qmd.md` — workspace in a subfolder (valid)
- `wrong.qmd.md` — workspace NOT in readme.qmd.md (error)
