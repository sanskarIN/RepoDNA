# Dependency analysis

RepoDNA reads dependency manifests and lockfiles to describe what a repository depends on,
how those dependencies are pinned, and which tools it needs. It only reads files: it never
runs a package manager and never contacts a registry, so results work offline and are
reproducible.

```sh
repodna dependencies
repodna report --format csv --table dependencies -o dependencies.csv
```

## Supported ecosystems

| Ecosystem | Manifests | Lockfiles |
|---|---|---|
| Rust (`cargo`) | `Cargo.toml` (including workspaces) | `Cargo.lock` |
| JavaScript and TypeScript (`npm`) | `package.json`, `pnpm-workspace.yaml` | `package-lock.json`, `npm-shrinkwrap.json`, `yarn.lock`, `pnpm-lock.yaml` |
| Python (`pypi`) | `pyproject.toml`, `setup.py`, `Pipfile`, `requirements*.txt`, `requirements*.in` | `poetry.lock`, `Pipfile.lock`, `uv.lock`, `pdm.lock` |
| Go (`go`) | `go.mod`, `go.work` | `go.sum` |
| Java, Kotlin, Scala (`maven`, `gradle`) | `pom.xml`, `build.gradle`, `build.gradle.kts`, `settings.gradle(.kts)`, `libs.versions.toml` | `gradle.lockfile` |
| .NET (`nuget`) | `*.csproj`, `*.fsproj`, `*.vbproj`, `packages.config`, `Directory.Packages.props` | `packages.lock.json` |
| Ruby (`rubygems`) | `Gemfile`, `*.gemspec` | `Gemfile.lock` |
| PHP (`composer`) | `composer.json` | `composer.lock` |
| Dart and Flutter (`pub`) | `pubspec.yaml` | `pubspec.lock` |
| Swift (`swiftpm`) | `Package.swift` | `Package.resolved` |

## What it reports

- **Dependencies** with their ecosystem, declared requirement, locked version, scope
  (runtime, development, build, peer, or optional), the manifests that declare them, and
  whether they are internal (another package of the same repository).
- **Usage**: how many files import each external package, from the import analysis. A
  declared package that no file imports may still be used by tooling or at run time.
- **Workspaces** and the packages they contain, which also define
  [modules](architecture-analysis.md#modules).
- **Requirements**: toolchains, runtimes, SDKs, and package managers the project declares,
  such as a Rust or Go version, Node.js `engines`, `requires-python`, or the package manager
  named in `package.json`.
- **Duplicates**: packages locked at more than one version.

## Findings

| Rule | Reported when |
|---|---|
| `dependencies.lock-mismatch` | A lockfile does not lock every dependency its manifest declares, so installs may resolve differently on different machines. |
| `dependencies.no-lockfile` | Manifests declare dependencies, but no lockfile is committed (for ecosystems that use one). |
| `dependencies.duplicate-versions` | A package is locked at more than one version. |
| `dependencies.stale-manifest` | A manifest has not changed for `stale_manifest_days` (730) while the repository did. |
| `dependencies.concentration` | Many files import one external package. |

## Limits

- **No vulnerability data.** RepoDNA does not know which versions have known
  vulnerabilities. Use your ecosystem's audit tool (`cargo audit`, `npm audit`,
  `pip-audit`, and so on) for that.
- **No resolution.** Version ranges are reported as declared; RepoDNA does not compute what
  a package manager would install.
- **No transitive analysis** beyond what lockfiles record.
- Build files that are programs (`setup.py`, `build.gradle`, `Package.swift`) are read as
  text, so dependencies computed at build time can be missed.
