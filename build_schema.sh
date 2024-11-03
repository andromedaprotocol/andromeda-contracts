#!/bin/bash

set -e
set -o pipefail

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
    rm -rf ./artifacts/$VERSION_FILENAME
    mkdir ./artifacts/$VERSION_FILENAME
    # Loop through all the schema for this contract
    for schema in $CONTRACT_PATH/schema/*.json; do
        local SCHEMA_NAME=$(basename $schema);
        cp "$schema" "./artifacts/$VERSION_FILENAME/$SCHEMA_NAME"   

    done

}

if [ ! -d "./artifacts" ]; then
    mkdir artifacts;
fi;

SKIP_GIT_DIFF=${SKIP_GIT_DIFF:-true}

GIT_BASE_BRANCH=${GIT_BASE_BRANCH:-development}

for directory in contracts/*/; do
    for contract in $directory/*/; do
        if ! $SKIP_GIT_DIFF; then
            if [[ -n $(git diff --merge-base $GIT_BASE_BRANCH --stat "$contract") ]]; then
                echo "Changes detected in $(basename $contract). Building..."
            else
                echo "No changes in $(basename $contract). Skip schema build."
                continue
            fi
        fi
        ( cd $contract && cargo schema )
        copy_schema $contract
    done
done