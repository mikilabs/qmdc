#!/usr/bin/env bash
# Run the shared conformance harnesses against each PACKED library, installed
# the way a user installs it, instead of the in-tree sources.
#
#   bash scripts/package-e2e.sh [ts] [py] [rs]      (default: all three)
#
# The harnesses are the same ones `make test` runs; only where the library comes
# from changes. Each run first proves the library really came from the install
# directory, so a silent fallback to the sources cannot pass. The code blocks of
# each package README are run as well, against a small `my-project` workspace,
# so a README cannot promise an import or a function the package does not have.
# Release preparation only: needs network for dependencies. Not part of `make test`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/qmdc-package-e2e.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
if [ "$#" -eq 0 ]; then set -- ts py rs; fi

# The workspace the README examples talk about.
make_readme_workspace() {
    mkdir -p "$1/my-project"
    printf '# My project [[my_project: __Workspace]]\n' >"$1/my-project/readme.qmd.md"
    printf '## Users [[users: Table]]\n\n- engine: postgres\n' >"$1/my-project/tables.qmd.md"
    printf '## Get users [[get_users: Endpoint]]\n\n- returns: [[#users]]\n- errors: [[#no_such_object]]\n' \
        >"$1/my-project/api.qmd.md"
}

# Write every ```<lang> block of README $1 to $2/readme-<n>.<ext>; print the count.
extract_blocks() {
    awk -v lang="$3" -v dir="$2" -v ext="$4" '
        $0 == "```" lang { n++; f = sprintf("%s/readme-%d.%s", dir, n, ext); inblk = 1; next }
        inblk && /^```/ { inblk = 0; close(f); next }
        inblk { print > f }
        END { print n + 0 }
    ' "$1"
}

# A harness that found no cases, or failed one, is not a pass.
check_reports() {
    local f tests failures
    for f in "$1"/*.xml; do
        [ -e "$f" ] || { echo "no JUnit reports in $1" >&2; exit 1; }
        tests="$(grep -o '<testsuite [^>]*tests="[0-9]*"' "$f" | head -1 | sed 's/.*tests="\([0-9]*\)"/\1/')"
        failures="$(grep -o '<testsuite [^>]*failures="[0-9]*"' "$f" | head -1 | sed 's/.*failures="\([0-9]*\)"/\1/')"
        echo "$(basename "$f" .xml): $tests cases, $failures failures"
        if [ "${tests:-0}" -eq 0 ] || [ "${failures:-1}" -ne 0 ]; then exit 1; fi
    done
}

run_ts() {
    echo "=== package-e2e: ts ==="
    local pack="$WORK/ts-pack" inst="$WORK/ts-install" reports="$WORK/ts-reports"
    mkdir -p "$pack" "$inst" "$reports"
    (cd "$ROOT/qmdc-ts" && npm pack --silent --pack-destination "$pack" >/dev/null)
    local tgz
    tgz="$(ls "$pack"/*.tgz)"
    echo "packed: $(basename "$tgz")"
    (
        cd "$inst"
        printf '{ "name": "qmdc-package-e2e", "private": true, "type": "module" }\n' >package.json
        # The CLI binaries are optional platform packages; the library does not need them.
        npm install --silent --omit=optional --no-audit --no-fund "$tgz"
    )
    # Resolve the package from INSIDE the install dir, as a user's project would.
    cat >"$inst/entry.mjs" <<'EOF'
export * from '@qmdc/qmdc';
export const resolvedFrom = import.meta.resolve('@qmdc/qmdc');
EOF
    # README blocks under plain Node: no tsx loader in the chain, which could
    # otherwise make a package that ships only .ts sources look importable.
    make_readme_workspace "$inst"
    local n i
    n="$(extract_blocks "$ROOT/qmdc-ts/README.md" "$inst" typescript mjs)"
    [ "$n" -gt 0 ] || { echo "no typescript blocks in qmdc-ts/README.md" >&2; exit 1; }
    for i in $(seq 1 "$n"); do (cd "$inst" && node "readme-$i.mjs" >/dev/null); done
    echo "README: $n blocks ran"
    (
        cd "$ROOT/qmdc-ts"
        export QMDC_TEST_TARGET=installed QMDC_TEST_INSTALL_DIR="$inst" QMDC_TEST_REPORT_DIR="$reports"
        npx tsx tests/test-parser.ts
        npx tsx tests/test-workspace.ts
        npx tsx tests/test-sql.ts
    )
    check_reports "$reports"
}

run_py() {
    echo "=== package-e2e: py ==="
    local pack="$WORK/py-pack" inst="$WORK/py-install" reports="$WORK/py-reports"
    mkdir -p "$pack" "$reports"
    # Build from a clean copy: setuptools reuses a stale qmdc-py/build/lib, so a
    # module dropped from the package would still ship (release-build.sh deletes
    # build/ for the same reason).
    rsync -a --exclude build --exclude dist --exclude .venv --exclude '*.egg-info' \
        --exclude __pycache__ --exclude .pytest_cache --exclude .ruff_cache \
        "$ROOT/qmdc-py/" "$WORK/py-src/"
    (cd "$WORK/py-src" && uv build --wheel --quiet --out-dir "$pack")
    local whl
    whl="$(ls "$pack"/*.whl)"
    echo "packed: $(basename "$whl")"
    uv venv --quiet --python 3.12 "$inst"
    VIRTUAL_ENV="$inst" uv pip install --quiet "$whl" pytest
    make_readme_workspace "$inst"
    local n i
    n="$(extract_blocks "$ROOT/qmdc-py/README.md" "$inst" python py)"
    [ "$n" -gt 0 ] || { echo "no python blocks in qmdc-py/README.md" >&2; exit 1; }
    # -I: isolated mode, so neither the cwd nor PYTHONPATH can put sources first.
    for i in $(seq 1 "$n"); do (cd "$inst" && bin/python -I "readme-$i.py" >/dev/null); done
    echo "README: $n blocks ran"
    # The library harnesses only; the CLI cross-parser tests in test_workspace.py
    # run the in-tree binaries and say nothing about the wheel. importlib mode
    # keeps pytest from inserting qmdc-py/ (which holds the qmdc/ sources) into
    # sys.path; tests/conftest.py refuses to start if qmdc came from anywhere else.
    echo "[qmdc under test] installed package: $(cd "$inst" && bin/python -I -c 'import qmdc; print(qmdc.__file__)')"
    (
        cd "$inst"
        export QMDC_TEST_TARGET=installed QMDC_TEST_INSTALL_DIR="$inst"
        bin/python -I -m pytest -q -p no:cacheprovider --import-mode=importlib \
            --rootdir "$ROOT/qmdc-py" -c "$ROOT/qmdc-py/pyproject.toml" \
            "$ROOT/qmdc-py/tests/test_parser.py" \
            "$ROOT/qmdc-py/tests/test_workspace.py::TestWorkspace" \
            "$ROOT/qmdc-py/tests/test_sql.py" \
            --junit-xml="$reports/py.xml" 2>&1 | tail -n 3
    )
    check_reports "$reports"
}

run_rs() {
    echo "=== package-e2e: rs ==="
    # The consumer sits in $base next to a `tests` link to the fixtures, because
    # the harnesses find them at CARGO_MANIFEST_DIR/../tests and write reports to
    # CARGO_MANIFEST_DIR/../test-reports.
    local base="$WORK/rs" pack="$WORK/rs-pack"
    local consumer="$base/consumer"
    mkdir -p "$pack" "$consumer/tests" "$consumer/src" "$base/test-reports"
    ln -s "$ROOT/tests" "$base/tests"
    (cd "$ROOT/qmdc-rs" && cargo package --quiet --allow-dirty --no-verify --target-dir "$pack/target")
    local crate
    crate="$(ls "$pack"/target/package/qmdc-*.crate)"
    echo "packed: $(basename "$crate")"
    tar xzf "$crate" -C "$pack"
    local unpacked
    unpacked="$(cd "$pack" && ls -d "$pack"/qmdc-*/)"
    unpacked="${unpacked%/}"
    cat >"$consumer/Cargo.toml" <<EOF
[package]
name = "qmdc-package-e2e"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
qmdc = { path = "$unpacked" }

[dev-dependencies]
regex = "1.10"
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
EOF
    cp "$ROOT/qmdc-rs/Cargo.lock" "$consumer/Cargo.lock"
    cp "$ROOT/qmdc-rs/tests/parser.rs" "$ROOT/qmdc-rs/tests/sql.rs" "$consumer/tests/"
    cp -R "$ROOT/qmdc-rs/tests/common" "$consumer/tests/"
    # The README block becomes the consumer's main.
    local n
    n="$(extract_blocks "$ROOT/qmdc-rs/README.md" "$WORK" rust rs)"
    [ "$n" -gt 0 ] || { echo "no rust blocks in qmdc-rs/README.md" >&2; exit 1; }
    {
        echo "#![allow(unused_variables)]"
        sed -n '/^use /p' "$WORK"/readme-*.rs
        echo "fn main() {"
        sed '/^use /d' "$WORK"/readme-*.rs
        echo "}"
    } >"$consumer/src/main.rs"
    # Prove the qmdc being compiled is the unpacked crate, not the tree.
    local resolved
    resolved="$(cd "$consumer" && cargo metadata --format-version 1 --quiet |
        python3 -c 'import json,sys; m=json.load(sys.stdin); print(next(p["manifest_path"] for p in m["packages"] if p["name"]=="qmdc"))')"
    case "$resolved" in
        "$unpacked"/*) echo "[qmdc under test] packaged crate: $resolved" ;;
        *) echo "qmdc resolved to $resolved, not the package under $unpacked" >&2; exit 1 ;;
    esac
    (cd "$consumer" && cargo run --quiet >/dev/null)
    echo "README: $n blocks ran"
    (cd "$consumer" && cargo test --quiet --test parser --test sql 2>&1 | tail -n 4)
    check_reports "$base/test-reports"
}

for t in "$@"; do
    case "$t" in
        ts) run_ts ;;
        py) run_py ;;
        rs) run_rs ;;
        *) echo "unknown target: $t (known: ts py rs)" >&2; exit 2 ;;
    esac
done
echo "✅ package-e2e passed: $*"
