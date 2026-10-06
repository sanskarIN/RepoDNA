#!/usr/bin/env bash
# Unpacks the command line archives of one version into DIR/<target>/repodna (repodna.exe
# on Windows), the layout that build.mjs --binaries reads.
#
# Usage: packaging/npm/unpack.sh VERSION ARCHIVES DIR
set -euo pipefail

usage="Usage: packaging/npm/unpack.sh VERSION ARCHIVES DIR"
version=${1:?$usage}
archives=${2:?$usage}
out=${3:?$usage}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

found=0
for archive in "$archives"/repodna-"$version"-*; do
  name=$(basename "$archive")
  case "$name" in
    *.tar.gz)
      name=${name%.tar.gz}
      tar -xzf "$archive" -C "$work"
      ;;
    *.zip)
      name=${name%.zip}
      unzip -q "$archive" -d "$work"
      ;;
    *) continue ;;
  esac
  target=${name#"repodna-$version-"}
  mkdir -p "$out/$target"
  cp "$work/$name"/repodna* "$out/$target/"
  found=$((found + 1))
done

if [ "$found" -eq 0 ]; then
  echo "There are no archives of repodna $version in $archives." >&2
  exit 1
fi
ls -l "$out"/*
