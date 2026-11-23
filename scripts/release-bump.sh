#!/usr/bin/env bash
# Bump every published package to the next version in lockstep and refresh the
# lock files, so a release commit is exactly what the Release button pushes.
#
# Usage: scripts/release-bump.sh patch|minor|major
#
# Packages: qmdc-rs (crate), qmdc-py (PyPI qmdc), qmdc-ts (npm), qmdc-vscode,
# qmdc-mkdocs (all via `make bump-<part>`) and qmdc-semantic (`make
# semantic-bump-<part>`). They have moved together since 1.0; the script refuses
# to run if they have drifted apart, because the tag vX.Y.Z would then be ambiguous.
# Needs uv, npm, and qmdc-vscode's dev deps (`npm ci` there) for its tsx bump script.
# Prints the new version as the last line.
set -euo pipefail

PART="${1:-}"
if [[ ! "$PART" =~ ^(major|minor|patch)$ ]]; then
  echo "usage: $0 patch|minor|major" >&2
  exit 2
fi
cd "$(dirname "$0")/.."

versions() {
  grep -m1 '^version = ' qmdc-rs/Cargo.toml | cut -d'"' -f2
  for p in qmdc-py qmdc-semantic qmdc-mkdocs; do
    grep -m1 '^version = ' "$p/pyproject.toml" | cut -d'"' -f2
  done
  for p in qmdc-ts qmdc-vscode; do
    node -p "require('./$p/package.json').version"
  done
}

current=$(versions | sort -u)
if [ "$(printf '%s\n' "$current" | wc -l)" -ne 1 ]; then
  echo "package versions differ, refusing a lockstep bump:" >&2
  printf '%s\n' "$current" >&2
  exit 1
fi

make --no-print-directory "bump-$PART" >&2
make --no-print-directory "semantic-bump-$PART" >&2

new=$(versions | sort -u)
if [ "$(printf '%s\n' "$new" | wc -l)" -ne 1 ] || [ "$new" = "$current" ]; then
  echo "bump did not move every package from $current to one new version:" >&2
  printf '%s\n' "$new" >&2
  exit 1
fi

# Lock files record each package's own version; refresh them the same way the
# release commits for 2.0.0 and 2.0.1 did.
for p in qmdc-py qmdc-semantic qmdc-mkdocs; do
  (cd "$p" && uv lock -q)
done
# npm locks: set only the root package's own version. `npm install
# --package-lock-only` would also pull qmdc-ts's placeholder optionalDependencies
# (the @qmdc/cli-* pins are written at pack time) into the lock.
for p in qmdc-ts qmdc-vscode; do
  node -e '
    const fs = require("fs"), f = process.argv[1], v = process.argv[2];
    const lock = JSON.parse(fs.readFileSync(f, "utf8"));
    lock.version = v;
    if (lock.packages && lock.packages[""]) lock.packages[""].version = v;
    fs.writeFileSync(f, JSON.stringify(lock, null, 2) + "\n");
  ' "$p/package-lock.json" "$new"
done

echo "$new"
