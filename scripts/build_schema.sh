#!/bin/bash

set -e
# set -o pipefail

get_version_filename (){
    local CONTRACT=$1
    # Get the version of the contract processed
    local BUILD_VERSION=$(cargo pkgid $CONTRACT | cut -d# -f2 | cut -d: -f2)
    local BUILD_TARGET=${CONTRACT//-/_}

    echo "$BUILD_TARGET@$BUILD_VERSION";
}

copy_schema () {
    local CONTRACT_PATH=$1;
    local CONTRACT=$(basename $CONTRACT_PATH);
    echo "$CONTRACT"
    local VERSION_FILENAME=$(get_version_filename $CONTRACT);
    rm -rf ./schemas/$VERSION_FILENAME
    mkdir ./schemas/$VERSION_FILENAME
    # Loop through all the schema for this contract
    for schema in $CONTRACT_PATH/schema/*.json; do
        local SCHEMA_NAME=$(basename $schema);
        cp "$schema" "./schemas/$VERSION_FILENAME/$SCHEMA_NAME"   
    done
}

if [ ! -d "./schemas" ]; then
    mkdir schemas;
fi;

# Check if any arguments were provided
if [ $# -eq 0 ]; then
    echo "No contracts specified. Processing all contracts..."
    # Original behavior: process all contracts
    for directory in contracts/*/; do
        for contract in $directory/*/; do
            ( cd $contract && cargo schema )
            copy_schema $contract
        done
    done
else
    # Process specified contracts or categories
    for input in "$@"; do
        # First check if it's a category (directory under contracts/)
        if [ -d "contracts/$input" ]; then
            echo "Processing category: $input"
            for contract in contracts/$input/*/; do
                ( cd "$contract" && cargo schema )
                copy_schema "$contract"
            done
        else
            # Try to find as individual contract
            contract_path=$(find contracts -type d -name "$input")
            if [ -z "$contract_path" ]; then
                echo "Warning: Neither contract nor category '$input' found"
                continue
            fi
            ( cd "$contract_path" && cargo schema )
            copy_schema "$contract_path"
        fi
    done
fi