# Installing and upgrading RepoDNA with WinGet

RepoDNA has two distinct Windows packages in the Microsoft Windows Package Manager
(WinGet) manifest format:

| Package ID | Installs | Release artifact |
|---|---|---|
| `SanskarIN.RepoDNA` | Desktop application | x64 / ARM64 MSI |
| `SanskarIN.RepoDNA.CLI` | `repodna` CLI (with the local web UI built in) | x64 / ARM64 portable ZIP |

**Important:** The manifests generated with RepoDNA v1.4.1 must first be accepted
into [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) and
propagated to the WinGet community source. A GitHub release, manifest ZIP or
the existence of this document alone **does not make the package available**
through `winget`. Until the listing is live, use the
[GitHub release downloads](https://github.com/sanskarIN/RepoDNA/releases).

## When the community source lists the packages

On Windows 10 or Windows 11 with a WinGet client that supports ZIP/portable
installations (WinGet 1.5 or newer), open PowerShell:

```powershell
winget source update
winget search --id SanskarIN.RepoDNA --exact
winget search --id SanskarIN.RepoDNA.CLI --exact

# Choose the desktop app, CLI, or both:
winget install --id SanskarIN.RepoDNA --exact --source winget
winget install --id SanskarIN.RepoDNA.CLI --exact --source winget
repodna --version

# Show available upgrades, including RepoDNA when a newer
# manifest is published and installed version detection matches:
winget upgrade

# Upgrade either package, when an upgrade is available:
winget upgrade --id SanskarIN.RepoDNA --exact --source winget
winget upgrade --id SanskarIN.RepoDNA.CLI --exact --source winget

# Or upgrade all supported packages:
winget upgrade --all
```

For example, if you install 1.4.1 through WinGet and the community source
later offers 1.4.2, `winget upgrade` should list 1.4.2
and `winget upgrade --id ...` can install it.
Merely publishing 1.4.2 on GitHub is **not enough**: the corresponding
1.4.2 manifests must be submitted, validated, merged and indexed.
WinGet does not necessarily recognize an existing manually extracted CLI ZIP.
The desktop MSI may be recognized through its Windows installed-app record,
but using WinGet for the initial installation is the most reliable path.

## How release maintainers publish WinGet manifests

The [Release workflow](../.github/workflows/release.yml) builds actual x64 and
ARM64 Windows installers and archives. Its `winget` job hashes exactly
those artifacts, checks that the CLI ZIPs contain the nested executable under
the expected path, and writes **six YAML manifests**: version, default locale
and installer manifests for each of the two package IDs.

The job uploads `repodna-<version>-winget-manifests.zip` to its artifacts.
On tagged releases, the existing `publish` job also attaches this
ZIP to the GitHub release and includes it in `SHA256SUMS.txt`.
For a dry-run workflow dispatch without a tag, this stays a build artifact
and does not publish a GitHub release.

### Verify or regenerate

Run `node --test packaging/winget/test/*.test.mjs` on a development
machine with Node.js 22, `zip`, and `unzip`.
To regenerate the YAML from **built** release files:

```sh
node packaging/winget/generate.mjs \
  --version 1.4.1 \
  --archives ./downloaded-release-artifacts \
  --out ./winget-out
```

No made-up hashes or placeholder URLs are accepted by this generator: all
four platform artifacts must be present locally. Before submission, check
the public URLs, inspect the YAML, verify the ZIP file contents, and on Windows
run `winget validate --manifest <manifest-version-directory>` for each
package. Test installation and upgrading on clean Windows x64/ARM64 machines,
including uninstall/reinstall and CLI PATH recognition.

### Submit to Microsoft

RepoDNA has a dedicated [WinGet submission workflow](../.github/workflows/winget-submit.yml).
It verifies all four Windows binaries against the published release SHA-256 list,
regenerates the manifests with the schema required by the community repository,
and publishes the validated YAML as a GitHub Actions artifact. For v1.4.1,
the exact six proposed files are also checked in under
[`packaging/winget/submissions/manifests`](../packaging/winget/submissions/manifests/).

For the upstream pull requests to be created automatically, the repository owner must
perform this **one-time authorization** (never share the token in an issue or chat):

1. On GitHub, create a **classic personal access token**, owned by `sanskarIN`,
   with only the `public_repo` scope required for public-repository submissions.
2. In **RepoDNA → Settings → Secrets and variables → Actions**, add a
   repository secret named `WINGET_CREATE_GITHUB_TOKEN` with that token as its value.
3. Go to **RepoDNA → Actions → Submit RepoDNA to WinGet → Run workflow** and select
   `v1.4.1` to submit the already-published version. New successful tagged releases
   automatically dispatch this workflow after they finish publishing.

When the secret is available, [`packaging/winget/submit.sh`](../packaging/winget/submit.sh)
creates or synchronizes the personal fork of
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), commits each package
version to a **separate branch**, and opens **two independent pull requests**.
The commits have author and committer email `sanskarin@outlook.in`.
It checks for already-open PRs to avoid duplicates. Without the secret, the
workflow still verifies and archives the six manifests, reports the missing
credential as an explicit warning, and marks the external submission job
**skipped**, not successful. It cannot submit the files to Microsoft.

The community repository requires **one package version per PR** and schema-header
comments in all three YAML files. Its PR template currently recommends schema
`1.12.0`; the older v1.4.1 GitHub release manifest ZIP was generated with
schema `1.10.0`, so the submission workflow regenerates the corrected
`1.12.0` files from the exact release binaries rather than submitting that ZIP.
Any later release uses the updated generator directly.

Microsoft's own validation checks and any required contributor-license agreement
remain mandatory. Opening a PR does **not** mean the package has been approved,
indexed, or made available through `winget install` or `winget upgrade`.
After Microsoft merges and indexes the manifests, run `winget source update`,
then `winget search --id SanskarIN.RepoDNA --exact` and check the CLI identifier
separately to verify public availability.

Do not put tokens in source files, release ZIPs, logs, or workflow inputs.
The standard repository `GITHUB_TOKEN` cannot create commits or pull requests
on behalf of the user in the external community repository.

### Future releases

Keep the existing archive and MSI file naming stable. Each subsequent tag
(v1.4.2, v1.4.3, etc.) regenerates new manifests and checksums.
The release pipeline automatically queues a fresh community submission
after successfully publishing a tagged GitHub release. Once the owner
configures the authorization secret, that workflow can open a separate
upstream PR for each package for the new version. Microsoft must then
accept and index it before `winget upgrade` will list the newer version.
Without the authorization secret, only manifest verification runs, and
the external submission is explicitly skipped.
