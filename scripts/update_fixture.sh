#!/usr/bin/env bash

# Update test/fixtures/dist_server.
#
# This script downloads files from https://moonup.csu.moe/v3 into target/update_fixture and
# processes and shrink them into files for fixture. For each of the `latest` and `nightly`
# channels, only the newest `KEEP` releases are kept (by the server's array order, which is
# ascending, so `releases[-KEEP:]`); the `bleeding` channel is preserved as-is.
#
# For the newest kept release of the `latest` channel, the `toolchain` component archives are
# additionally shrunk to a few-KB stub archive containing only the per-target native `moon`
# stub, and the component index is rewritten to point at that archive (dropping `libcore`).
# The stub binaries are cross-compiled on the fly with `zig cc` via tests/fixtures/stub/gen.sh
# (see tests/fixtures/stub/README.md); the binaries themselves are gitignored.
#
# When installed (`-i`), the `E2E_TEST_MOCK_INSTALL_VERSION` const in tests/constant.rs is
# rewritten to the derived stub version so the atomic e2e tests always target a release that
# exists in the committed fixtures.
#
# ## How to use
#
# Run this script; it derives everything from the live dist server.
#
# ## Arguments
#
# -i: Install the updates to the directory test/fixtures/dist_server.
#     If not specified, place processed files into target/update_fixture.

set -x

# Number of newest releases to keep per channel (latest, nightly).
# The `bleeding` channel is preserved as-is.
KEEP=3

STUB_DIR=tests/fixtures/stub/out
# target triple : stub binary (relative to STUB_DIR) : output binary name in archive : archive format
# The stubs are named `<triplet>-stub` (a `-stub` suffix so the same
# binary can be reused as, e.g., a `moonc` stub later). The archive format
# mirrors the real prod distribution: `.zip` on Windows, `.tar.gz` elsewhere.
STUB_TARGETS=(
    "aarch64-apple-darwin:aarch64-apple-darwin-stub:moon:tar.gz"
    "x86_64-unknown-linux:x86_64-unknown-linux-stub:moon:tar.gz"
    "aarch64-unknown-linux:aarch64-unknown-linux-stub:moon:tar.gz"
    "x86_64-pc-windows:x86_64-pc-windows-stub:moon.exe:zip"
)

DIR=tests/fixtures/dist_server
WORKDIR=target/update_fixture
URL=https://moonup.csu.moe/v3

function sha256_of() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        sha256sum "$1" | awk '{print $1}'
    fi
}

function download_and_process() {
    # rm -rf $DIR
    rm -rf $WORKDIR
    mkdir -p $WORKDIR
    for path in \
        index.json \
        channel-latest.json \
        channel-nightly.json \
        ; do \
        curl -fsSL $URL/$path -o $WORKDIR/$path
    done

    # channel-*.json — keep the newest KEEP releases per channel. The server
    # array order is ascending (oldest first), so the newest are the tail.
    cat $WORKDIR/channel-latest.json \
        | jq --argjson keep "$KEEP" '.releases |= .[-$keep:]' \
        > $WORKDIR/channel-latest.json.tmp
    mv $WORKDIR/channel-latest.json.tmp $WORKDIR/channel-latest.json
    cat $WORKDIR/channel-nightly.json \
        | jq --argjson keep "$KEEP" '.releases |= .[-$keep:]' \
        > $WORKDIR/channel-nightly.json.tmp
    mv $WORKDIR/channel-nightly.json.tmp $WORKDIR/channel-nightly.json
    cp -a $DIR/channel-bleeding.json $WORKDIR/

    # index.json — keep the downloaded index as-is and just append the
    # `bleeding` channel, which prod's index.json does not list but the
    # committed fixture serves. The `latest`/`nightly` channel entries already
    # point at the newest prod release, which the retention window keeps.
    jq --slurpfile b $DIR/channel-bleeding.json \
        '.channels += [{name: "bleeding", version: $b[0].releases[0].version}]' \
        $WORKDIR/index.json > $WORKDIR/index.json.tmp
    mv $WORKDIR/index.json.tmp $WORKDIR/index.json

    # latest/<version>/<target>.json
    cat $WORKDIR/channel-latest.json | jq -r '.releases[] | .version as $v | .targets[] as $t | [$v, $t] | @tsv' \
        | while IFS=$'\t' read -r version target; do
              mkdir -p "$WORKDIR/latest/$version"
              curl -fsSL "$URL/latest/$version/$target.json" -o "$WORKDIR/latest/$version/$target.json"
          done
    # nightly/<date>/<target>.json
    cat $WORKDIR/channel-nightly.json | jq -r '.releases[] | .date as $d | .targets[] as $t | [$d, $t] | @tsv' \
        | while IFS=$'\t' read -r date target; do
              mkdir -p "$WORKDIR/nightly/$date"
              curl -fsSL "$URL/nightly/$date/$target.json" -o "$WORKDIR/nightly/$date/$target.json"
          done
    # bleeding/*/*.json
    cp -a $DIR/bleeding $WORKDIR/
}

# Replace the `toolchain` archives of the given version (the newest kept release
# of the `latest` channel) with stub-only fixtures: each per-target archive
# contains only `bin/moon[.exe]`, the built fixture stub. The component index
# for that version is rewritten to list only the `toolchain` component pointing
# at the shrunk archive.
#
# The stubs are cross-compiled on the fly by gen.sh; assembly happens under a
# layout dir kept outside $WORKDIR so the installed dist-server fixtures only
# contain the served index/channel/component JSONs and the download archives.
function shrink_stub_archives() {
    # regenerate the stubs
    bash "$(dirname "$0")/../tests/fixtures/stub/gen.sh"

    local ver="$1"
    for entry in "${STUB_TARGETS[@]}"; do
        local target="${entry%%:*}"
        local fmt="${entry##*:}"
        local rest="${entry#*:}"
        local stub="$STUB_DIR/${rest%%:*}"
        local bin_name="${rest#*:}"
        bin_name="${bin_name%:*}"
        local archive_file="moonbit-v${ver}-${target}.${fmt}"

        if [ ! -f "$stub" ]; then
            echo "ERROR: stub missing for $target: $stub" >&2
            echo "Run tests/fixtures/stub/gen.sh (needs 'zig cc')" >&2
            exit 1
        fi

        local layout="$WORKDIR/../stub-layout/$target"
        rm -rf "$layout"
        mkdir -p "$layout/bin" "$WORKDIR/download/v${ver}"
        cp "$stub" "$layout/bin/$bin_name"

        # absolute path: the zip branch runs in a subshell that has cd'd into
        # $layout, so a relative path would point at the wrong place.
        local archive="$(cd "$WORKDIR" && pwd)/download/v${ver}/$archive_file"
        if [ "$fmt" = "zip" ]; then
            # zip stores paths relative to cwd, so run inside the layout dir
            # while writing the archive to the absolute output path.
            (cd "$layout" && zip -qr "$archive" bin)
        else
            tar -C "$layout" -czf "$archive" bin
        fi

        local sha256
        sha256=$(sha256_of "$WORKDIR/download/v${ver}/$archive_file")

        jq -n \
            --arg file "$archive_file" \
            --arg sha256 "$sha256" \
            '{version: 2, components: [{name: "toolchain", file: $file, sha256: $sha256}]}' \
            > "$WORKDIR/latest/$ver/$target.json"
    done
}

# The version whose toolchain archives are replaced by stub-only fixtures:
# the newest release kept in the filtered `latest` channel (the same value the
# `latest` entry of index.json points at).
function stub_version() {
    jq -r '.releases[-1].version' "$WORKDIR/channel-latest.json"
}

# Under `-i`, rewrite the E2E_TEST_MOCK_INSTALL_VERSION const in tests/constant.rs
# to the derived stub version so the atomic e2e tests target a release that
# exists in the committed fixtures. Only done on install; a dry run must not
# mutate a repo source file.
function sync_test_constant() {
    local ver="$1"
    local const_file="$(dirname "$0")/../tests/constant.rs"
    if ! grep -q 'E2E_TEST_MOCK_INSTALL_VERSION' "$const_file"; then
        echo "WARNING: E2E_TEST_MOCK_INSTALL_VERSION not found in $const_file; leaving it unchanged" >&2
        return
    fi
    local tmp="$const_file.tmp"
    sed "s#\(E2E_TEST_MOCK_INSTALL_VERSION: &str = \"\)[^\"]*\(\"\)#\1$ver\2#" "$const_file" > "$tmp"
    mv "$tmp" "$const_file"
}

function main() {
    if [ "$#" -gt 1 ]; then
        echo 'Invalid arguments'
        exit 1
    fi

    local should_install="false"
    case "$1" in
    "")
        ;;
    "-i")
        should_install="true"
        ;;
    *)
        echo 'Invalid arguments'
        exit 1
        ;;
    esac

    download_and_process

    local ver
    ver=$(stub_version)
    shrink_stub_archives "$ver"

    if [ "$should_install" = "true" ]; then
        rm -rf $DIR
        mv $WORKDIR $DIR
        sync_test_constant "$ver"
    fi
}

main "$@"
