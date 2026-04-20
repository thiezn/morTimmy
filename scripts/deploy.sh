#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(dirname "$(realpath "$0")")/.."
cd "$ROOT_DIR" || { echo "❌ Failed to cd into ${ROOT_DIR}"; exit 1; }


# First test and build the production apps
# cargo test --release
# echo "Tests passed"

echo "Building the production binaries..."
cargo build --release

sudo cp ./target/release/mortimmy /usr/local/bin

# Now change the permissions of the binaries so that they can be executed by anyone.
sudo chown root:admin /usr/local/bin/mortimmy

# Verify the permissions
# ls -ltrah /usr/local/bin/* | grep "\-rwx"
