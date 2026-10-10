#!/usr/bin/env bash
# Installs the repodna command line for the GitHub Action: the release archive for this
# runner's system and processor, checked against the release's SHA256SUMS.txt.
#
# Usage: install.sh VERSION DIR
#   VERSION  a release such as 1.3.1 (or v1.3.1), or "latest"
#   DIR      where to put the repodna binary
set -euo pipefail

usage="Usage: install.sh VERSION DIR"
version=${1:?$usage}
dir=${2:?$usage}
repository=${REPODNA_REPOSITORY:-sanskarIN/RepoDNA}
server=${GITHUB_SERVER_URL:-https://github.com}

if [ "$version" = latest ]; then
  # The address of the latest release names its tag. Unlike the API, it has no rate limit
  # shared with every other job on the runner's network.
  url=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$server/$repository/releases/latest")
  version=${url##*/}
fi
version=${version#v}
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "::error title=RepoDNA::$version is not a RepoDNA version such as 1.3.1." >&2
  exit 1
fi

case "${RUNNER_OS:-$(uname -s)}:${RUNNER_ARCH:-$(uname -m)}" in
  Linux:X64 | Linux:x86_64) target=x86_64-unknown-linux-musl ;;
  Linux:ARM64 | Linux:aarch64) target=aarch64-unknown-linux-musl ;;
  macOS:X64 | Darwin:x86_64) target=x86_64-apple-darwin ;;
  macOS:ARM64 | Darwin:arm64) target=aarch64-apple-darwin ;;
  Windows:X64) target=x86_64-pc-windows-msvc ;;
  Windows:ARM64) target=aarch64-pc-windows-msvc ;;
  *)
    echo "::error title=RepoDNA::RepoDNA has no build for ${RUNNER_OS:-?} on ${RUNNER_ARCH:-?}." >&2
    exit 1
    ;;
esac

name="repodna-$version-$target"
case "$target" in
  *windows*) archive="$name.zip" binary=repodna.exe ;;
  *) archive="$name.tar.gz" binary=repodna ;;
esac

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
base="$server/$repository/releases/download/v$version"
for file in "$archive" SHA256SUMS.txt; do
  if ! curl -fsSL --retry 3 -o "$work/$file" "$base/$file"; then
    echo "::error title=RepoDNA::RepoDNA $version has no $file; see $server/$repository/releases" >&2
    exit 1
  fi
done

expected=$(awk -v file="$archive" '$2 == file || $2 == "*" file { print $1 }' "$work/SHA256SUMS.txt")
if command -v sha256sum > /dev/null; then
  actual=$(sha256sum "$work/$archive" | cut -d ' ' -f 1)
else
  actual=$(shasum -a 256 "$work/$archive" | cut -d ' ' -f 1)
fi
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  echo "::error title=RepoDNA::$archive does not match the checksum in SHA256SUMS.txt." >&2
  exit 1
fi

case "$archive" in
  *.zip) unzip -q "$work/$archive" -d "$work" ;;
  *) tar -xzf "$work/$archive" -C "$work" ;;
esac
mkdir -p "$dir"
cp "$work/$name/$binary" "$dir/$binary"
chmod +x "$dir/$binary"
"$dir/$binary" --version
