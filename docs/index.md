# RepoDNA documentation

RepoDNA is an open-source, local-first repository intelligence and code archaeology tool.
It turns a repository into an evidence-backed analysis of its structure, architecture,
history, dependencies, and health, which you can explore in the terminal, in a web
interface, or in a desktop app, and share as reports and Project DNA cards.

This documentation is published at
[sanskarin.github.io/RepoDNA/docs](https://sanskarin.github.io/RepoDNA/docs/), with search,
and is in the [`docs` folder](https://github.com/sanskarIN/RepoDNA/tree/main/docs) of the
repository.

**Latest stable release:** [RepoDNA v1.6.0](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.6.0). Download binaries and installers from the release page or follow the [installation guide](installation.md).

## Start here

- [Getting started](getting-started.md): analyze your first repository in five minutes
- [Practical workflows](workflows.md): browser exploration, CLI history, CI baselines, and safe sharing
- [Reviewing changes](reviewing-changes.md): baseline impact, module ownership, safe Findings exports, and version availability
- [Installation](installation.md): binaries, the desktop app, npm, WebAssembly, and building
  from source
- [WinGet on Windows](winget.md): desktop and CLI installation, upgrades, and publishing
- [The web version](web.md#the-web-version): analyze a folder in your browser, with nothing to
  install
- [GitHub Pages deployment](github-pages.md): how to verify and troubleshoot web and docs builds
- [FAQ](faq.md)

## Using RepoDNA

- [Command-line reference](cli.md)
- [The web interface](web.md) and its local API
- [The desktop app](desktop.md)
- [The GitHub Action](github-action.md)
- [v1.6.0 stable: reviewing changes and release compatibility](reviewing-changes.md#5-release-and-research-status)
- [Reports](reports.md) and [Project DNA cards](dna-cards.md)
- [Configuration](configuration.md)
- [Privacy](privacy.md)
- [AI explanations](ai.md) (optional)
- [Plugins](plugins.md)
- [Troubleshooting](troubleshooting.md)

## Native AI model and benchmark development

RepoDNA includes a built-in **offline extractive explanation** provider and an
experimental locally trained relevance ranker. These are not a trained
generative language model. The research guides explain how to prepare
independently reviewed labels and evaluate ranking quality without claiming
accuracy on synthetic fixtures.

- [Current native model capabilities and limitations](native-model.md)
- [Human relevance annotation rubric](ai-annotation-rubric.md)
- [Curated question-task coverage audit](ai-task-coverage.md)
- [Double-review annotation agreement and adjudication](ai-reviewer-agreement.md)
- [Data quality and copied-label audit](ai-label-audit.md)
- [Clone-family-disjoint benchmark splits](ai-family-splits.md)
- [Three-way training and epoch selection](ai-training-selection.md)
- [Comparing pointwise and pairwise ranking](ai-objective-selection.md)
- [Benchmark fingerprints and independent replay](ai-benchmark-provenance.md)

## How the analysis works

- [The analysis engine](analysis-engine.md): stages, profiles, statuses, and performance
- [The RepositoryDNA model](repository-dna.md): the artifact, findings, rules, and the fingerprint
- [Git history and code archaeology](git-analysis.md)
- [Architecture analysis](architecture-analysis.md)
- [Dependency analysis](dependency-analysis.md)
- [Complexity and code quality](complexity.md)
- [Tests, build, and documentation](tests-and-build.md)
- [Security signals and safe analysis](security.md)
- [The Codebase Time Machine](time-machine.md)

## Developing RepoDNA

- [Architecture](architecture.md)
- [Development](development.md): setup, checks, and where to make changes
- [Branding](branding.md): original SVG logo, colors, startup splash, and icon generation
- [Contributing](contributing.md)
- [Benchmarks](../benchmarks/README.md) and [fixture repositories](../fixtures/README.md)

## The project

- [Examples](../examples/README.md): RepoDNA's analysis of itself, a CI workflow, and
  configurations
- [Media kit](https://github.com/sanskarIN/RepoDNA/blob/images/media/README.md):
  screenshots, promo images, and Project DNA cards for posts and talks, on the `images`
  branch
- [Screenshots](https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/README.md)
  of the web interface in every release, side by side, on the `images` branch
- [Roadmap](../ROADMAP.md), [changelog](../CHANGELOG.md), and the
  [releases](https://github.com/sanskarIN/RepoDNA/tree/releases-info) on the
  `releases-info` branch, one folder per version with its notes, downloads, install
  commands, and pull requests
- [Security policy](../SECURITY.md), [support](../SUPPORT.md), and the
  [code of conduct](../CODE_OF_CONDUCT.md)
- [Privacy Policy](../PRIVACY.md), [Terms of Use](../TERMS.md), and the
  [third-party notices](../THIRD-PARTY-NOTICES.txt)
- [Governance](../GOVERNANCE.md) and [maintainers](../MAINTAINERS.md)

RepoDNA is made by [Sanskar](https://github.com/sanskarIN) and licensed under the
[Apache License 2.0](../LICENSE). More open-source projects by Sanskar are at
[sanskarin.github.io](https://sanskarin.github.io).
