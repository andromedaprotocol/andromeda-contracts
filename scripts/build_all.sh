#!/bin/bash
mkdir -p ./artifacts

# Detect architecture and use appropriate optimizer image
if [[ $(uname -m) == "arm64" ]]; then
  OPTIMIZER_IMAGE="cosmwasm/optimizer-arm64:0.16.1"
else
  OPTIMIZER_IMAGE="cosmwasm/optimizer:0.16.1"
fi

echo "Building contracts for $(uname -m) architecture"

docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  $OPTIMIZER_IMAGE
