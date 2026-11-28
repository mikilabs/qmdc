#!/bin/bash
# Fail when a Linux binary needs a newer glibc than its manylinux wheel tag promises.
#
#   bash scripts/check-glibc.sh BINARY PLATFORM_TAG
#
# PLATFORM_TAG is the wheel tag, e.g. manylinux_2_17_x86_64. A manylinux_X_Y tag
# lets pip install the wheel on glibc X.Y and newer, so every GLIBC_ symbol
# version the binary imports must be X.Y or older. Any other tag (musllinux,
# macOS, Windows) has nothing to check. Reads the dynamic symbol table with
# objdump (the Xcode/LLVM one reads ELF too).
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "usage: $0 BINARY PLATFORM_TAG" >&2
    exit 2
fi
bin="$1"
tag="$2"

if [[ ! "$tag" =~ ^manylinux_([0-9]+)_([0-9]+)_ ]]; then
    exit 0
fi
allowed="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}"

command -v objdump >/dev/null || { echo "check-glibc: objdump not found" >&2; exit 2; }
syms="$(objdump -T "$bin")" || { echo "check-glibc: cannot read $bin" >&2; exit 2; }

needed="$(printf '%s\n' "$syms" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' \
    | sort -u -t. -k1,1n -k2,2n -k3,3n | tail -1)"
if [ -z "$needed" ]; then
    echo "check-glibc: $bin imports no GLIBC_ symbols; is it a glibc ELF binary?" >&2
    exit 2
fi

# The binary fits the tag when the allowed version is the newer of the two.
newest="$(printf '%s\n%s\n' "$needed" "$allowed" | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)"
if [ "$newest" != "$allowed" ]; then
    echo "check-glibc: $bin needs GLIBC_$needed but its tag $tag promises glibc $allowed:" >&2
    printf '%s\n' "$syms" | awk -v a="$allowed" '
        match($0, /GLIBC_[0-9][0-9.]*/) {
            v = substr($0, RSTART + 6, RLENGTH - 6)
            split(v, p, "."); split(a, q, ".")
            if (p[1] + 0 > q[1] + 0 || (p[1] + 0 == q[1] + 0 && p[2] + 0 > q[2] + 0))
                print "  " $NF " (GLIBC_" v ")"
        }' | sort -u >&2
    exit 1
fi
echo "check-glibc: $(basename "$bin") needs GLIBC_$needed, tag $tag allows $allowed"
