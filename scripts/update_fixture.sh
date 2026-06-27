#!/usr/bin/env bash

# Update test/fixtures/dist_server.
#
# This script downloads files from https://moonup.csu.moe/v3 into target/update_fixture and
# processes and shrink them into files for fixture with the given `VERSIONS`. Files for bleeding
# channel is preserved.
#
# ## How to use
#
# Update `VERSIONS` below and run this script.
#
# ## Arguments
#
# -i: Install the updates to the directory test/fixtures/dist_server.
#     If not specified, place processed files into target/update_fixture.

set -x

VERSIONS='0.10.0+84519ca0a,0.10.0+e66899a54,0.10.1+a46be2066,0.10.1+1f52b86e1,0.10.2+b1ac037eb,0.10.2+f06d4fbd5'

DIR=tests/fixtures/dist_server
WORKDIR=target/update_fixture
URL=https://moonup.csu.moe/v3

function download_and_process() {
    # rm -rf $DIR
    rm -rf $WORKDIR
    mkdir -p $WORKDIR
    for path in \
        index.json \
        channel-latest.json \
        channel-nightly.json \
        ; do \
        curl -fL $URL/$path -o $WORKDIR/$path
    done

    # index.json
    jq --slurpfile a $DIR/index.json '.channels += [$a[0].channels[] | select(.name == "bleeding")]' $WORKDIR/index.json > $WORKDIR/index.json.tmp
    mv $WORKDIR/index.json.tmp $WORKDIR/index.json

    # channel-*.json
    cat $WORKDIR/channel-latest.json \
        | jq --arg versions "$VERSIONS" '($versions | split(",")) as $vs | .releases |= map(select(.version | IN($vs[])))' \
        > $WORKDIR/channel-latest.json.tmp
    mv $WORKDIR/channel-latest.json.tmp $WORKDIR/channel-latest.json
    cat $WORKDIR/channel-nightly.json \
        | jq --arg versions "$VERSIONS" '($versions | split(",")) as $vs | .releases |= map(select(.version | IN($vs[])))' \
        > $WORKDIR/channel-nightly.json.tmp
    mv $WORKDIR/channel-nightly.json.tmp $WORKDIR/channel-nightly.json
    cp -a $DIR/channel-bleeding.json $WORKDIR/

    # latest/<version>/<target>.json
    cat $WORKDIR/channel-latest.json | jq -r '.releases[] | .version as $v | .targets[] as $t | [$v, $t] | @tsv' \
        | while IFS=$'\t' read -r version target; do
              mkdir -p "$WORKDIR/latest/$version"
              curl -fL "$URL/latest/$version/$target.json" -o "$WORKDIR/latest/$version/$target.json"
          done
    # nightly/<date>/<target>.json
    cat $WORKDIR/channel-nightly.json | jq -r '.releases[] | .date as $d | .targets[] as $t | [$d, $t] | @tsv' \
        | while IFS=$'\t' read -r date target; do
              mkdir -p "$WORKDIR/nightly/$date"
              curl -fL "$URL/nightly/$date/$target.json" -o "$WORKDIR/nightly/$date/$target.json"
          done
    # bleeding/*/*.json
    cp -a $DIR/bleeding $WORKDIR/
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

    if [ "$should_install" = "true" ]; then
        rm -rf $DIR
        mv $WORKDIR $DIR
    fi
}

main "$@"
