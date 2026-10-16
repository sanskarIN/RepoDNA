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

A maintainer must submit **both** complete version directories from the
release ZIP to the [WinGet package repository](https://github.com/microsoft/winget-pkgs).
Use the official [WinGet Manifest Creator](https://github.com/microsoft/winget-create)
or open pull requests with the YAML under the appropriate
`manifests/s/SanskarIN/RepoDNA/...` paths. On the first
submission, confirm the identifiers are available; keep them unchanged for
future releases. The optional [WinGet submission workflow](../.github/workflows/winget-submit.yml)
can create those pull requests after a release is published if a maintainer
has configured the GitHub secret `WINGET_CREATE_GITHUB_TOKEN`
with appropriate permissions; it is never required for ordinary builds.

Do not store tokens in source files, workflow inputs, release ZIPs or logs.
An external GitHub token is not provided by the standard `GITHUB_TOKEN`
for creating community-repository pull requests. Microsoft controls review,
acceptance and indexing, so submission does not guarantee immediate availability.

### Future releases

Keep the existing archive and MSI file naming stable. Each subsequent tag
(v1.4.2, v1.4.3, etc.) regenerates new manifests and checksums. After each
public release, submit that **new** manifest version through the community
repository. Users then get normal version comparisons from WinGet without
RepoDNA needing a background updater or network calls of its own.
