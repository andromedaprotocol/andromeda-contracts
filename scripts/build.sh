#!/bin/bash

# EXAMPLE USAGE:
# build.sh andromeda-contract some-category
# Builds "andromeda-contract" contract and "some-category" category

# LOG all the contracts compiled with there compressed file size
FILE_LOG=""

get_version_filename (){
    local CONTRACT=$1
    # Get the version of the contract processed
    local BUILD_VERSION=$(cargo pkgid $CONTRACT | cut -d# -f2 | cut -d: -f2)
    local BUILD_TARGET=${CONTRACT//-/_}

    echo "$BUILD_TARGET@$BUILD_VERSION";
}

build_contract () {
    local CONTRACT_PATH=$1;

    local CONTRACT=`basename $CONTRACT_PATH`;
    echo "Building contract $CONTRACT..."
    if ! cargo wasm -p $CONTRACT -q; then
        exit 1
    fi

    local BUILD_TARGET=${CONTRACT//-/_}
    # local VERSION_FILENAME=$(get_version_filename $CONTRACT);
    
    local IN_FILE="./target/wasm32-unknown-unknown/release/$BUILD_TARGET.wasm"
    local OUT_FILE="./artifacts/$BUILD_TARGET.wasm"
    local OUT_FILE_IBC_TEST="./ibc-tests/artifacts/$BUILD_TARGET.wasm"
    local OUT_FILE_PACKAGE="./packages/andromeda-testing-e2e/artifacts/$BUILD_TARGET.wasm"

    wasm-opt -Os $IN_FILE -o $OUT_FILE
    cp $IN_FILE $OUT_FILE_IBC_TEST
    cp $IN_FILE $OUT_FILE_PACKAGE
    
    # NOT SO IMPORTANT STEPS
    # Log wasm file sizes at the end of build process
    local IN_FILESIZE=$(($(wc -c <"$IN_FILE") +0))
    local OUT_FILESIZE=$(($(wc -c <"$OUT_FILE") +0))
    local LOG="$BUILD_TARGET \t\t: $IN_FILESIZE \t- $OUT_FILESIZE bytes"
    FILE_LOG="$FILE_LOG\n$LOG"
}

build_category () {
     for directory in contracts/*/; do
        if [ "$(basename $directory)" = "$1" ]; then
            echo "Building all contracts in category $(basename $directory)..."
            for contract in $directory/*/; do
                build_contract $contract;
            done
            break
        fi
    done
}

# Helper function to build all contracts with build all command
build_all() {
    for directory in contracts/*/; do
        build_category $(basename $directory)
    done
}

is_contract() {
    for directory in contracts/*/; do
        for contract in $directory/*/; do
            if [ "$(basename $contract)" = "$1" ]; then
                return 0
            fi
        done
    done
    return 1
}

is_category() {
    for directory in contracts/*/; do
        if [ "$(basename $directory)" = "$1" ]; then
            return 0
        fi
    done
    return 1
}

export RUSTFLAGS="-C link-arg=-s"

#Clear current builds
rm -rf ./target
rm -rf ./artifacts
rm -rf ./packages/andromeda-testing-e2e/artifacts
rm -rf ./ibc-tests/artifacts
mkdir artifacts
mkdir packages/andromeda-testing-e2e/artifacts
mkdir ibc-tests/artifacts

set -e
for target in "$@"; do
    if [ "$target" = "all" ]; then
        build_all
    elif is_contract $target; then
        build_contract $target
    elif is_category $target; then
        build_category $target
    else
        echo "$target is not a valid target"
        exit 1
    fi
    echo -e "$FILE_LOG"
done