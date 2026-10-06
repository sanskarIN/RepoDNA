#!/usr/bin/env bash
# Checks the packages that build.mjs staged: npm can pack each of them, and the launcher
# finds and runs the Linux x64 binary when both are installed side by side, as npm installs
# them. Runs on Linux x64.
#
# Usage: packaging/npm/check.sh DIR
set -euo pipefail

dir=${1:?Usage: packaging/npm/check.sh DIR}

for package in "$dir"/*/; do
  (cd "$package" && npm pack --dry-run)
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
modules="$work/node_modules/@sanskarin"
mkdir -p "$modules"
cp -r "$dir/repodna" "$dir/repodna-linux-x64" "$modules/"
node "$modules/repodna/bin/repodna.js" --version
