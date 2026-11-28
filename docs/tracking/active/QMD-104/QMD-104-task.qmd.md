# QMD-104: Linux wheels need the glibc their tag promises

## The manylinux wheels bundle a binary that needs glibc 2.30 [[qmd104_glibc_floor: Bug]]

GitHub [#58](https://github.com/mikilabs/qmdc/issues/58). The `qmdc` x86_64 Linux wheel is
tagged `manylinux_2_17_x86_64` and the aarch64 wheel `manylinux_2_28_aarch64`, so pip installs
them on glibc 2.17 and 2.28. The bundled binaries import `GLIBC_2.30` symbols, so on an older
system (Amazon Linux 2, glibc 2.26) the install succeeds and every `qmdc` command fails with
`version 'GLIBC_2.30' not found`.

- status: done_review
- priority: high
- category: release
- findings: [[#qmd104_finding_cause]], [[#qmd104_finding_fix]]
- result: [[#qmd104_result]]

### Goals [[goals: [Goal]]]

#### A1: The glibc need of each Linux binary is measured and checked [[qmd104_goal_a1]]

`scripts/check-glibc.sh BINARY TAG` fails when a binary imports a `GLIBC_` symbol newer than its
`manylinux_X_Y` tag allows. On binaries built the current way it fails for both glibc targets
(`GLIBC_2.30` against 2.17 and 2.28).

- group: A_tests
- done: true

#### B1: Each glibc binary links against the glibc of its wheel tag [[qmd104_goal_b1]]

`scripts/release-build.sh` builds a glibc target as `<triple>.X.Y`, taking `X.Y` from the
target's wheel tag, and runs the check on every binary it builds. The x86_64 binary needs
nothing newer than `GLIBC_2.17` and the aarch64 binary nothing newer than `GLIBC_2.28`.

- group: B_fix
- done: true
