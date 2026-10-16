#!/usr/bin/env bash
# Create one upstream PR per package, with explicit commit author/committer email.
set -euo pipefail
root="$1"
tag="$2"
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "::error::Expected a stable vX.Y.Z tag" >&2; exit 1
fi
if [[ -z "$GH_TOKEN" ]]; then
  echo "::error::No authorized GitHub token available" >&2; exit 1
fi
version="$(printf '%s' "$tag" | cut -c2-)"
login="$(gh api user --jq .login)"
if [[ "$login" != sanskarIN ]]; then
  echo "::error::Use a GitHub token for sanskarIN" >&2; exit 1
fi
fork="$login/winget-pkgs"
if ! gh api "repos/$fork" > /dev/null 2>&1; then
  gh api -X POST repos/microsoft/winget-pkgs/forks -f name=winget-pkgs --silent
fi
ready=false
for _ in $(seq 1 30); do
  if gh api "repos/$fork" > /dev/null 2>&1; then
    parent="$(gh api "repos/$fork" --jq '.parent.full_name // ""')"
    if [[ "$parent" != microsoft/winget-pkgs ]]; then
      echo "::error::The existing fork name belongs to a different upstream" >&2; exit 1
    fi
    if gh api "repos/$fork/git/ref/heads/master" > /dev/null 2>&1; then
      ready=true; break
    fi
  fi
  sleep 2
done
if [[ "$ready" != true ]]; then
  echo "::error::Fork initialization timed out" >&2; exit 1
fi
gh api -X POST "repos/$fork/merge-upstream" -f branch=master --silent
base="$(gh api "repos/$fork/git/ref/heads/master" --jq .object.sha)"
base_tree="$(gh api "repos/$fork/git/commits/$base" --jq .tree.sha)"
for entry in "SanskarIN.RepoDNA:desktop" "SanskarIN.RepoDNA.CLI:cli"; do
  id="$(printf '%s' "$entry" | cut -d: -f1)"
  label="$(printf '%s' "$entry" | cut -d: -f2)"
  folder="$root/s/$(printf '%s' "$id" | tr . /)/$version"
  test -d "$folder"
  test "$(find "$folder" -maxdepth 1 -type f -name '*.yaml' | wc -l)" -eq 3
  branch="repodna-$tag-$label"
  if ! gh api "repos/$fork/git/ref/heads/$branch" > /dev/null 2>&1; then
    temp_tree="$(mktemp)"
    node - "$root" "$folder" "$base_tree" > "$temp_tree" <<'NODE'
const fs = require('node:fs');
const path = require('node:path');
const [root, folder, base_tree] = process.argv.slice(2);
const names = fs.readdirSync(folder).filter(n => n.endsWith('.yaml')).sort();
if (names.length !== 3) process.exit(1);
const tree = names.map(name => ({
  path: 'manifests/' + path.relative(root, path.join(folder, name)).split(path.sep).join('/'),
  mode: '100644',
  type: 'blob',
  content: fs.readFileSync(path.join(folder, name), 'utf8')
}));
process.stdout.write(JSON.stringify({base_tree, tree}));
NODE
    tree_sha="$(gh api -X POST "repos/$fork/git/trees" --input "$temp_tree" --jq .sha)"
    rm "$temp_tree"
    temp_commit="$(mktemp)"
    jq -n --arg message "New package: $id version $version" --arg tree "$tree_sha" --arg parent "$base" \
      '{message:$message,tree:$tree,parents:[$parent],author:{name:"sanskarIN",email:"sanskarin@outlook.in"},committer:{name:"sanskarIN",email:"sanskarin@outlook.in"}}' > "$temp_commit"
    commit_sha="$(gh api -X POST "repos/$fork/git/commits" --input "$temp_commit" --jq .sha)"
    rm "$temp_commit"
    temp_ref="$(mktemp)"
    jq -n --arg ref "refs/heads/$branch" --arg sha "$commit_sha" '{ref:$ref,sha:$sha}' > "$temp_ref"
    gh api -X POST "repos/$fork/git/refs" --input "$temp_ref" --silent
    rm "$temp_ref"
  fi
  existing="$(gh api -X GET "repos/microsoft/winget-pkgs/pulls?state=open&head=$login:$branch" --jq '.[0].html_url // ""')"
  if [[ -n "$existing" ]]; then
    echo "::notice::Upstream pull request already exists: $existing"
    continue
  fi
  temp_pr="$(mktemp)"
  jq -n --arg title "New package: $id version $version" --arg head "$login:$branch" \
    --arg body "RepoDNA $version manifests from official, SHA-256-verified Windows release artifacts (x64 and ARM64). This PR adds exactly one package version, with all three manifest files. Release: https://github.com/sanskarIN/RepoDNA/releases/tag/$tag. Microsoft validation and review are required." \
    '{title:$title,head:$head,base:"master",body:$body}' > "$temp_pr"
  pr_url="$(gh api -X POST repos/microsoft/winget-pkgs/pulls --input "$temp_pr" --jq .html_url)"
  rm "$temp_pr"
  echo "::notice::$id PR created: $pr_url"
done
