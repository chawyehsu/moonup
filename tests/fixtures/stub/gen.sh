#!/usr/bin/env bash
# Generate the fixture stub binaries with `zig cc`.
#
# The binaries are build output and gitignored (see .gitignore);
# `scripts/update_fixture.sh` runs this script before assembling the shrunken
# toolchain archives. Run it standalone to regenerate just the stubs.

set -euo pipefail

cd "$(dirname "$0")"

if ! command -v zig >/dev/null 2>&1; then
    echo "error: 'zig' not found" >&2
    exit 1
fi

mkdir -p out

zig cc -target aarch64-macos      -Os -s stub.c -o out/aarch64-apple-darwin-stub
zig cc -target x86_64-linux-musl  -Os -s stub.c -o out/x86_64-unknown-linux-stub
zig cc -target aarch64-linux-musl -Os -s stub.c -o out/aarch64-unknown-linux-stub
zig cc -target x86_64-windows-gnu -Os -s stub.c -o out/x86_64-pc-windows-stub

echo "Generated stubs under out/"
