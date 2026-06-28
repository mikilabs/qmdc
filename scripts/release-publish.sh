#!/bin/bash
# Idempotently publish EVERYTHING that release-build.sh produced in ./dist-release.
#
#   ./scripts/release-publish.sh             # dry-run (default): show what WOULD publish, skip nothing real
#   ./scripts/release-publish.sh --publish   # real publish (needs the tokens below)
#
# Idempotent by design — safe to re-run on the same version. Each artifact is
# skipped if that exact version already exists in its registry, so a partially
# failed release can simply be re-run (already-published packages are no-ops).
#
# Registries + required env (in CI these come from GitHub Secrets):
#   PyPI                 UV_PUBLISH_TOKEN        (qmdc wheels + qmdc-semantic + qmdc-mkdocs)
#   npm                  NODE_AUTH_TOKEN         (@qmdc/qmdc + @qmdc/cli-<platform> x7)
#   crates.io            CARGO_REGISTRY_TOKEN    (qmdc crate)
#   VS Code Marketplace  VSCE_PAT                (qmdc-vscode, 6 platform vsix)
#   Open VSX             OVSX_TOKEN              (qmdc-vscode, 6 platform vsix)
#
# This script does NOT build. Run scripts/release-build.sh first (or `make publish`).
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="dist-release"

# Local tokens: source .env.publish if present (gitignored — never committed).
# In CI these come from GitHub Secrets instead, so the file simply won't exist.
if [[ -f .env.publish ]]; then
    set -a; . ./.env.publish; set +a
fi

DRY_RUN=1
REGISTRIES=()
for arg in "$@"; do
    case "$arg" in
        --publish)            DRY_RUN=0 ;;
        pypi|npm|crate|vscode) REGISTRIES+=("$arg") ;;
        *) echo "❌ unknown arg: $arg (use --publish and/or: pypi npm crate vscode)" >&2; exit 2 ;;
    esac
done
# Default: all registries.
[[ ${#REGISTRIES[@]} -eq 0 ]] && REGISTRIES=(pypi npm crate vscode)

if [[ ! -d "$OUT" ]]; then
    echo "❌ $OUT/ not found — run scripts/release-build.sh first." >&2
    exit 1
fi

note() { printf '  %s\n' "$1"; }
hr()   { echo "=== $1 ==="; }

# --- PyPI ------------------------------------------------------------------
# twine --skip-existing is natively idempotent: it uploads only files the index
# does not already have, so re-running a release re-uploads nothing.
publish_pypi() {
    hr "PyPI (dist-release/pypi)"
    shopt -s nullglob
    local files=("$OUT"/pypi/*.whl "$OUT"/pypi/*.tar.gz)
    shopt -u nullglob
    if [[ ${#files[@]} -eq 0 ]]; then note "no PyPI artifacts, skip"; return; fi

    uv run --with twine twine check "${files[@]}"
    if [[ "$DRY_RUN" == "1" ]]; then
        note "dry-run: would 'twine upload --skip-existing' ${#files[@]} files"
        return
    fi
    : "${UV_PUBLISH_TOKEN:?Set UV_PUBLISH_TOKEN to publish to PyPI}"
    TWINE_USERNAME=__token__ TWINE_PASSWORD="$UV_PUBLISH_TOKEN" \
        uv run --with twine twine upload --skip-existing "${files[@]}"
}

# --- npm -------------------------------------------------------------------
# Per package@version pre-check via `npm view`; publish only when missing.
# Platform packages MUST go up before the main launcher so its
# optionalDependencies resolve for early installers.
npm_publish_tgz() {
    local tgz="$1"
    # Read name+version straight from the packed tarball (authoritative).
    local pkgjson name version
    pkgjson="$(tar -xzOf "$tgz" package/package.json)"
    name="$(node -e 'let d="";process.stdin.on("data",c=>d+=c).on("end",()=>console.log(JSON.parse(d).name))' <<<"$pkgjson")"
    version="$(node -e 'let d="";process.stdin.on("data",c=>d+=c).on("end",()=>console.log(JSON.parse(d).version))' <<<"$pkgjson")"

    if [[ -n "$(npm view "$name@$version" version 2>/dev/null)" ]]; then
        note "skip $name@$version (already on npm)"
        return
    fi
    if [[ "$DRY_RUN" == "1" ]]; then
        note "dry-run: would 'npm publish --access public' $name@$version"
        return
    fi
    : "${NODE_AUTH_TOKEN:?Set NODE_AUTH_TOKEN to publish to npm}"
    npm publish --access public "$tgz"
}

publish_npm() {
    hr "npm (dist-release/npm)"
    shopt -s nullglob
    local platform=("$OUT"/npm/qmdc-cli-*.tgz)   # @qmdc/cli-<suffix> packs to qmdc-cli-*.tgz
    local main=("$OUT"/npm/qmdc-qmdc-*.tgz)       # main launcher @qmdc/qmdc packs to qmdc-qmdc-<version>.tgz
    shopt -u nullglob
    if [[ ${#platform[@]} -eq 0 && ${#main[@]} -eq 0 ]]; then note "no npm tarballs, skip"; return; fi

    # npm reads the auth token from an .npmrc, not from $NODE_AUTH_TOKEN directly.
    # Point npm at a temp userconfig so we never touch the user's ~/.npmrc; the
    # RETURN trap removes it when this function exits.
    if [[ "$DRY_RUN" == "0" ]]; then
        : "${NODE_AUTH_TOKEN:?Set NODE_AUTH_TOKEN to publish to npm}"
        local npmrc; npmrc="$(mktemp)"
        printf '//registry.npmjs.org/:_authToken=%s\n' "$NODE_AUTH_TOKEN" > "$npmrc"
        export NPM_CONFIG_USERCONFIG="$npmrc"
        trap 'rm -f "$npmrc"' RETURN
    fi

    local tgz
    for tgz in ${platform[@]+"${platform[@]}"}; do npm_publish_tgz "$tgz"; done
    for tgz in ${main[@]+"${main[@]}"}; do npm_publish_tgz "$tgz"; done
}

# --- crates.io -------------------------------------------------------------
# crates.io takes a source upload (not a prebuilt artifact). Pre-check the
# version via the public API; publish only when absent.
publish_crate() {
    hr "crates.io (qmdc)"
    local version
    version="$(grep '^version = ' qmdc-rs/Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
    # crates.io's API REQUIRES a User-Agent — without it the request 403s, which
    # would make this pre-check silently fall through and try to re-publish.
    if curl -fsSL -A "qmdc-release (https://github.com/mikilabs/qmdc)" \
            "https://crates.io/api/v1/crates/qmdc/$version" >/dev/null 2>&1; then
        note "skip qmdc@$version (already on crates.io)"
        return
    fi
    if [[ "$DRY_RUN" == "1" ]]; then
        note "dry-run: would 'cargo publish' qmdc@$version"
        (cd qmdc-rs && cargo publish --dry-run --allow-dirty >/dev/null) && note "cargo publish --dry-run OK"
        return
    fi
    : "${CARGO_REGISTRY_TOKEN:?Set CARGO_REGISTRY_TOKEN to publish to crates.io}"
    # Belt-and-suspenders: if the pre-check missed it (stale index, race), don't
    # let cargo's "already exists" error abort the whole publish run.
    local out
    if out="$(cd qmdc-rs && cargo publish 2>&1)"; then
        printf '%s\n' "$out"
    elif printf '%s' "$out" | grep -q "already exists"; then
        note "skip qmdc@$version (already on crates.io)"
    else
        printf '%s\n' "$out" >&2
        return 1
    fi
}

# --- VS Code Marketplace + Open VSX ----------------------------------------
# Both take the per-platform .vsix files. Idempotency is best-effort: a version
# already published makes the tool error, which we treat as "skip".
publish_vscode() {
    hr "VS Code Marketplace + Open VSX (dist-release/vscode)"
    shopt -s nullglob
    local vsix=("$OUT"/vscode/*.vsix)
    shopt -u nullglob
    if [[ ${#vsix[@]} -eq 0 ]]; then note "no vsix, skip"; return; fi

    # The two registries are independent — publish to whichever has a token set.
    local do_vsce=0 do_ovsx=0
    [[ -n "${VSCE_PAT:-}" ]]   && do_vsce=1
    [[ -n "${OVSX_TOKEN:-}" ]] && do_ovsx=1
    if [[ "$DRY_RUN" == "0" ]]; then
        [[ $do_vsce -eq 0 ]] && note "VSCE_PAT not set — skipping VS Code Marketplace"
        [[ $do_ovsx -eq 0 ]] && note "OVSX_TOKEN not set — skipping Open VSX"
        if [[ $do_vsce -eq 0 && $do_ovsx -eq 0 ]]; then note "no vscode tokens, nothing to do"; return; fi
        # Open VSX requires the namespace to exist (idempotent — ignore if already owned).
        if [[ $do_ovsx -eq 1 ]]; then
            npx --yes ovsx create-namespace mikilabs -p "$OVSX_TOKEN" 2>/dev/null \
                || note "ovsx namespace 'mikilabs' already exists"
        fi
    fi

    local f
    for f in "${vsix[@]}"; do
        if [[ "$DRY_RUN" == "1" ]]; then
            note "dry-run: would publish $(basename "$f") (Marketplace if VSCE_PAT, Open VSX if OVSX_TOKEN)"
            continue
        fi
        if [[ $do_vsce -eq 1 ]]; then
            npx --yes @vscode/vsce publish --packagePath "$f" --pat "$VSCE_PAT" --skip-duplicate \
                || note "vsce: $(basename "$f") skipped (already published?)"
        fi
        if [[ $do_ovsx -eq 1 ]]; then
            npx --yes ovsx publish "$f" --pat "$OVSX_TOKEN" --skip-duplicate \
                || note "ovsx: $(basename "$f") skipped (already published?)"
        fi
    done
}

# --- run -------------------------------------------------------------------
if [[ "$DRY_RUN" == "1" ]]; then
    echo "▶ release-publish DRY-RUN (no uploads). Pass --publish to upload."
else
    echo "▶ release-publish: REAL publish (idempotent)."
fi
echo "  registries: ${REGISTRIES[*]}"
echo ""

for r in "${REGISTRIES[@]}"; do
    case "$r" in
        pypi)   publish_pypi ;;
        npm)    publish_npm ;;
        crate)  publish_crate ;;
        vscode) publish_vscode ;;
    esac
done

echo ""
echo "✅ release-publish done."
