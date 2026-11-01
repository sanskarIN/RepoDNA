# GitHub Pages deployment and verification

RepoDNA publishes a static web application at
[RepoDNA online](https://sanskarin.github.io/RepoDNA/) and the
[documentation portal](https://sanskarin.github.io/RepoDNA/docs/).
The site is generated from the repository's `main` branch by GitHub Actions.
The browser analyzer uses a WASI WebAssembly engine built from this repository,
not a hosted AI API or a service that receives your repository.

## How publication works

The [Web version workflow](../.github/workflows/pages.yml) triggers after
selected application, Rust, documentation, dependency, and packaging changes on
`main`. It can also be started manually with **Actions → Web version →
Run workflow** by someone with appropriate repository permissions.

The workflow:

1. Checks out `main`, installs locked npm dependencies, and compiles
   the local analysis engine to `wasm32-wasip1`.
2. Builds `@repodna/web` with that compiled engine, rather than only
   shipping a mock interface.
3. Builds every documentation guide using
   `node packaging/docs/build.mjs --out apps/web/dist/docs --check`.
   Broken internal links or missing pages cause the build to fail.
4. Uploads the site artifact and deploys through
   `actions/deploy-pages` to the `github-pages` environment.
5. Reports the deployment URL and warns if the repository's Pages source
   is still configured to deploy from a branch.

GitHub Actions deploys the **site and docs together**. A successful CI build
on a feature branch is **not** a public Pages deployment.

## Verify a deployed version

1. Open [Actions: Web version](https://github.com/sanskarIN/RepoDNA/actions/workflows/pages.yml)
   and select the most recent run on `main` with a successful **Deploy** job.
2. Verify that the run's **Build** job succeeded, including the WebAssembly
   build, web UI build, documentation link validation, and artifact upload.
3. Open [the application](https://sanskarin.github.io/RepoDNA/) and
   [the documentation](https://sanskarin.github.io/RepoDNA/docs/).
4. Check the published docs search, navigation and at least one new guide.
   When verifying an exact change, compare the run's commit SHA with the
   corresponding commit on `main`. A green CI check by itself is insufficient.
5. If the previous build is still visible, check whether a newer Pages
   deployment is queued or running before interpreting the website as stale.

The deployment action's logs confirm the published commit and resulting URL.
The site does not automatically reflect unmerged feature-branch changes.

## Pages source and deployment pitfalls

Set **Settings → Pages → Build and deployment → Source: GitHub Actions**
for this repository. When the source is **Deploy from a branch**, GitHub may
temporarily publish the raw repository contents after a push, replacing the web
interface until the Actions workflow deploys the built artifact again.

The [Pages workflow](../.github/workflows/pages.yml) detects that mismatch
and emits a warning. It cannot change the repository's Pages settings by
itself. The workflow serializes deployments, so a newer run might be waiting
behind an earlier one. Avoid creating artificial empty commits or releases
just to increment the deployment count.

If a deployment fails, inspect the first failing step in the matching run:

- WebAssembly build: validate Rust target, Cargo.lock and WASI entry points.
- Web build: validate npm lockfile, package scripts and compiled engine path.
- Documentation: run the docs builder with `--check` and fix missing
  files/heading anchors.
- Deployment: inspect `github-pages` environment permissions and Actions
  Pages source; do not publish an untested `dist` directory as a workaround.

## Reproducible local documentation build

After installing project npm dependencies, run:

~~~sh
npm ci
node packaging/docs/build.mjs --out apps/web/dist/docs --check
~~~

The generated `dist` is a build output. Commit the original Markdown and
web source files, not compiled artifacts. Always wait for CI checks before
merging site changes that might break the public interface.
