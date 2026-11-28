# QMD-104: Result

## Linux binaries match their manylinux wheel tags [[qmd104_result: Result]]

The x86_64 glibc binary now needs nothing newer than glibc 2.17 and the aarch64 one nothing newer
than 2.28, the floors their wheel tags promise. The release build fails if a glibc binary ever
needs more than its tag allows. The npm `@qmdc/cli-linux-x64` and `@qmdc/cli-linux-arm64`
packages carry the same binaries.

- feature: [[#qmd104_glibc_floor]]
- completed: 2026-11-28
- files_changed: [scripts/release-build.sh, scripts/check-glibc.sh, docs/releasing/readme.qmd.md]
- tests_changed: []

### Verification [[qmd104_result_verification: text]]

- about: [[#qmd104_result]]

- Built the way `release-build.sh` builds on `main` (`3c4c053`), with the release CI pins
  (Rust 1.99.0, zig 0.15.2, cargo-zigbuild 0.23.0): both glibc binaries import `GLIBC_2.30`
  (`gettid`), plus `GLIBC_2.29` (`pow`, `log`) and on x86_64 `GLIBC_2.28` (`statx`,
  `fcntl64`). `check-glibc.sh` fails on the aarch64 one against `manylinux_2_28_aarch64` and
  lists those symbols.
- `make dist` on the branch exits 0. The build log shows `x86_64-unknown-linux-gnu.2.17` and
  `aarch64-unknown-linux-gnu.2.28`, and the check reports `GLIBC_2.17` and `GLIBC_2.28`. The
  binaries unpacked from `qmdc-2.0.2-py3-none-manylinux_2_17_x86_64.whl`,
  `qmdc-2.0.2-py3-none-manylinux_2_28_aarch64.whl` and the two npm tarballs give the same
  versions. The other five targets build as before.
- `check-glibc.sh` exits 0 for a non-manylinux tag and for a binary at its tag's floor.
- `make validate-docs` and `make md-lint` exit 0.
- Code review: self-review only.
