#!/bin/bash

set -euo pipefail

# Make the script run in its own directory instead of the caller's directory.
cd "$(cd -P -- "$(dirname -- "$0")" && pwd -P)" || exit 1

pushd ..

cargo clippy -- -D warnings
cargo fmt --all -- --check

popd
