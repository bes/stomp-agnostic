#!/bin/bash

set -euo pipefail

# Make the script run in its own directory instead of the caller's directory.
cd "$(cd -P -- "$(dirname -- "$0")" && pwd -P)" || exit 1

pushd ..

cargo fmt --all

popd
