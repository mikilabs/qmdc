# QMD-104: Findings

## zig links against a newer glibc than the wheel tag [[qmd104_finding_cause: Finding]]

`scripts/release-build.sh` cross-compiles the Linux targets with `cargo zigbuild --target
x86_64-unknown-linux-gnu` (and `aarch64-unknown-linux-gnu`). With no glibc version in the target,
zig links against a recent glibc, and Rust's standard library then uses `gettid` (glibc 2.30),
`statx` and `fcntl64` (2.28), and `pow`/`log` (2.29). The PyPI tag is set afterwards with
`wheel tags --platform-tag manylinux_2_17_x86_64`, which only renames the wheel; nothing checked
that the binary matched it. The musl binary is static and is not affected.

- category: bug
- related_to: [[#qmd104_glibc_floor]]
- affected_files: [scripts/release-build.sh]
- solution: Build each glibc target against the glibc its tag names, and check the result.

## Take the glibc version from the wheel tag and check the binary [[qmd104_finding_fix: Finding]]

cargo-zigbuild accepts a glibc version as a target suffix (`x86_64-unknown-linux-gnu.2.17`); the
output still goes to `target/<triple>/`. The build loop derives the suffix from the target's
`manylinux_X_Y` tag, so the tag stays the single place the floor is written. When zig links
against an older glibc, the standard library falls back to the older symbols (or a raw syscall)
for the calls above. The new `scripts/check-glibc.sh` reads the binary's dynamic symbols with
`objdump -T` and fails the build when any `GLIBC_` version is newer than the tag allows; tags
that are not manylinux are skipped. The npm `@qmdc/cli-linux-*` packages ship the same binaries
and get the same fix. The VS Code VSIX builds its Linux binaries separately in
`qmdc-vscode/package.json` with the plain triples, so they still need glibc 2.30 while VS Code
runs on 2.28; that is outside #58 and is noted on the PR.

- category: design
- related_to: [[#qmd104_glibc_floor]]
- solution: `<triple>.X.Y` targets and a `check-glibc.sh` call in `release-build.sh`.
