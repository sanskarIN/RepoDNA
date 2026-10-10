#!/usr/bin/env bash
# Checks the packages that build.mjs staged: npm can pack each of them, the launcher
# finds and runs the Linux x64 binary when both are installed side by side, as npm installs
# them, the web interface's server serves its page, and the WebAssembly package analyzes
# a sample repository. Runs on Linux x64.
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

if [ -d "$dir/repodna-web" ]; then
  cp -r "$dir/repodna-web" "$modules/"
  node "$modules/repodna-web/bin/repodna-web.js" --version
  node --input-type=module -e '
    const { createWebServer, root } = await import(process.argv[1]);
    const server = createWebServer(root).listen(0, "127.0.0.1", async () => {
      const response = await fetch(`http://127.0.0.1:${server.address().port}/`);
      const page = await response.text();
      server.close();
      if (response.status !== 200 || !page.includes("<div id=\"root\">")) {
        console.error(`The web interface was not served: ${response.status}`);
        process.exit(1);
      }
      console.log("The web interface is served.");
    });
  ' "$modules/repodna-web/lib/index.js"
fi

if [ -d "$dir/repodna-wasm" ]; then
  # Its one dependency, as npm would install it next to it.
  repository=$(cd "$(dirname "$0")/../.." && pwd)
  mkdir -p "$work/node_modules/@bjorn3"
  cp -r "$repository/node_modules/@bjorn3/browser_wasi_shim" "$work/node_modules/@bjorn3/"
  cp -r "$dir/repodna-wasm" "$modules/"
  node "$modules/repodna-wasm/bin/repodna-wasm.js" --version
  sample="$work/sample"
  mkdir -p "$sample/src"
  printf 'pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n' > "$sample/src/lib.rs"
  printf '[package]\nname = "sample"\nversion = "0.1.0"\n' > "$sample/Cargo.toml"
  node "$modules/repodna-wasm/bin/repodna-wasm.js" analyze "$sample" --profile quick \
    --output "$work/sample.repodna"
  node -e '
    const dna = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8"));
    if (dna.identity.name !== "sample" || dna.languages.primary[0] !== "rust") {
      console.error("The WebAssembly analysis did not find the sample repository.");
      process.exit(1);
    }
    console.log("The WebAssembly analysis works.");
  ' "$work/sample.repodna"
fi
