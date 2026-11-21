# Installing and upgrading RepoDNA with WinGet

RepoDNA uses the `Sanskar` publisher prefix in its two Microsoft Windows Package Manager
(WinGet) manifest format:

| Package ID | Installs | Release artifact |
|---|---|---|
| `Sanskar.RepoDNA` | Desktop application | x64 / ARM64 MSI |
| `Sanskar.RepoDNA.CLI` | `repodna` CLI (with the local web UI built in) | x64 / ARM64 portable ZIP |

**Important:** RepoDNA [v1.5.2](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2)
is published as stable on GitHub, but **WinGet indexing is separate**.
Its first automated [submission attempt](https://github.com/sanskarIN/RepoDNA/actions/runs/44757474107)
validated the Windows installers but failed during full-fork synchronization:
GitHub refused an unrelated workflow-file update without the PAT's `workflow`
scope (HTTP 422). [RepoDNA PR #289](https://github.com/sanskarIN/RepoDNA/pull/289)
removed that unnecessary synchronization and added a fail-closed fork-history
check. The [v1.5.2 retry](https://github.com/sanskarIN/RepoDNA/actions/runs/44763093413)
succeeded and opened two upstream Microsoft PRs:

- [Desktop `Sanskar.RepoDNA` — PR #485086](https://github.com/microsoft/winget-pkgs/pull/485086)
- [CLI `Sanskar.RepoDNA.CLI` — PR #485087](https://github.com/microsoft/winget-pkgs/pull/485087)

Both PRs contained exactly one commit and three package manifest YAML files
when opened. Microsoft still needs to review, merge and index the submissions;
an open PR is **not proof** that either package is installable from the
[public WinGet catalog](https://github.com/microsoft/winget-pkgs).
Until the exact version is independently verified in that catalog, use
[the official v1.5.2 GitHub downloads](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2).

## When the community source lists the packages

On Windows 10 or Windows 11 with a WinGet client that supports ZIP/portable
installations (WinGet 1.5 or newer), open PowerShell:

```powershell
winget source update
winget search --id Sanskar.RepoDNA --exact
winget search --id Sanskar.RepoDNA.CLI --exact

# Choose the desktop app, CLI, or both:
winget install --id Sanskar.RepoDNA --exact --source winget
winget install --id Sanskar.RepoDNA.CLI --exact --source winget
repodna --version

# Show available upgrades, including RepoDNA when a newer
# manifest is published and installed version detection matches:
winget upgrade

# Upgrade either package, when an upgrade is available:
winget upgrade --id Sanskar.RepoDNA --exact --source winget
winget upgrade --id Sanskar.RepoDNA.CLI --exact --source winget

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

For future upstream submissions, the repository owner must keep a
**user-authorized GitHub credential** configured (never put tokens into chat,
issues, source files, artifacts or logs):

1. Use an account-authorized token belonging to `sanskarIN` with the least
   permissions required to create manifest-only PRs in the public fork.
   The corrected submission script no longer synchronizes upstream
   `.github/workflows` files, so it does not require additional privileges
   to update those workflow files. If a fork has unrelated commits, the
   script fails rather than pushing them upstream.
2. In **RepoDNA → Settings → Secrets and variables → Actions**, configure
   the repository secret `WINGET_CREATE_GITHUB_TOKEN` securely.
3. Use **RepoDNA → Actions → Submit RepoDNA to WinGet → Run workflow** for a
   released tag when resubmission is needed. The script checks for existing
   package PRs before creating new ones; v1.5.2 already has the two upstream
   PRs linked above, so avoid duplicate manual submissions.

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
`1.12.0`; the original v1.4.1 GitHub release manifest ZIP was generated with
schema `1.10.0` and the former `SanskarIN` package identifiers. Do **not**
submit that historical ZIP to Microsoft. The submission workflow regenerates
schema `1.12.0` files using the selected `Sanskar.RepoDNA` and
`Sanskar.RepoDNA.CLI` IDs directly from the original verified release binaries.
Any later release uses the updated generator directly.

Microsoft's own validation checks and any required contributor-license agreement
remain mandatory. Opening a PR does **not** mean the package has been approved,
indexed, or made available through `winget install` or `winget upgrade`.
After Microsoft merges and indexes the manifests, run `winget source update`,
then `winget search --id Sanskar.RepoDNA --exact` and check the CLI identifier
separately to verify public availability.

Do not put tokens in source files, release ZIPs, logs, or workflow inputs.
The standard repository `GITHUB_TOKEN` cannot create commits or pull requests
on behalf of the user in the external community repository.

### Community indexing is distinct from GitHub publication

**Historical v1.4.1 and v1.4.2 baseline.** Do not treat an open Microsoft PR, a passing upstream
validation pipeline, or the presence of a GitHub release as proof that WinGet
has activated a package. For the v1.4.1 baseline, confirm that **both**
[Desktop PR #453917](https://github.com/microsoft/winget-pkgs/pull/453917) and
[CLI PR #453918](https://github.com/microsoft/winget-pkgs/pull/453918)
are merged, and verify both package IDs appear in the public `winget` source:

```powershell
winget source update
winget show --id Sanskar.RepoDNA --exact --source winget
winget show --id Sanskar.RepoDNA.CLI --exact --source winget
```

This section records the historical baseline; it is not a current block on
GitHub releases. v1.5.2 has already been published as stable, but Microsoft
must separately accept and index **v1.5.2** manifests for any WinGet upgrade.
Never describe it as installable solely because the GitHub release
or valid manifest ZIP exists.

During v1.4.2 development, use the included PowerShell verifier to smoke-test
Windows CLI ZIP packages **without installing them** (no WinGet source, admin
privileges, or account configuration are required):

```powershell
./packaging/winget/check-windows-cli.ps1 `
  -Archive "./repodna-1.4.2-x86_64-pc-windows-msvc.zip" `
  -Version "1.4.2" -Target "x86_64-pc-windows-msvc"
```

This checks archive paths and duplicates, the presence of the expected
`repodna.exe`, the PE architecture, and the CLI's version/help/no-argument
launch behavior and invalid-argument exit code in a disposable directory.
Use `-Target "aarch64-pc-windows-msvc" -SkipExecution` to statically
validate ARM64 packages on x64 Windows; executable validation on ARM64 still
requires a native Windows-on-Arm runner or device. CI creates an x64 debug ZIP
for the same checks, and release builds check their **actual packaged ZIPs**
before upload. These checks do not guarantee acceptance by Microsoft.

For v1.4.2 development, the CLI's no-argument launch has a regression test and
returns success after printing help. The v1.4.1 CLI community PR was labeled
`Validation-Executable-Error` even though its overall validation pipeline
passed; its actual cause has **not been conclusively established**. Before
submitting a future CLI version, run the new release smoke checks on native
Windows x64 and ARM64 devices, verify `repodna.exe`, `repodna.exe --help`, and
`repodna.exe --version`, and confirm that WinGet installs the portable alias.
Treat external package-manager validation as an additional requirement; never
mark a version installable before Microsoft has merged and indexed it.

Keep the existing archive and MSI file naming stable. Each subsequent tag
(v1.4.2, v1.4.3, etc.) regenerates new manifests and checksums.
The release pipeline automatically queues a fresh community submission
after successfully publishing a tagged GitHub release. Once the owner
configures the authorization secret, that workflow can open a separate
upstream PR for each package for the new version. Microsoft must then
accept and index it before `winget upgrade` will list the newer version.
Without the authorization secret, only manifest verification runs, and
the external submission is explicitly skipped.
