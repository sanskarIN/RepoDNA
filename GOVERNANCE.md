# Governance

RepoDNA is an open-source project led by its creator and developed in the open with its
contributors. This document describes how the project is run.

## Roles

- **Users** use RepoDNA and help by reporting problems and sharing what they need.
- **Contributors** improve the project with code, documentation, fixtures, plugins, reviews,
  or issue reports. Anyone can become a contributor by following
  [CONTRIBUTING.md](CONTRIBUTING.md).
- **Maintainers**, listed in [MAINTAINERS.md](MAINTAINERS.md), review and merge pull
  requests, triage issues, cut releases, and handle security reports.
- **The lead maintainer**, the project's creator, sets the direction of the project and
  makes the final decision when the maintainers do not reach agreement.

## How decisions are made

- Most decisions happen in issues and pull requests. A change is accepted when a
  maintainer approves it and no maintainer objects.
- Larger changes (a new analysis area, a change to the RepositoryDNA schema, a new
  front end, or a change to the security model) start as an issue that describes the
  problem, the proposed approach, and the alternatives, so that anyone can comment before
  work begins.
- Disagreements are resolved by discussion. When a decision cannot be reached, the lead
  maintainer decides and explains why.
- Decisions follow the project's principles: evidence-backed analysis, local-first privacy,
  safety with untrusted code, determinism, and accessibility.

## Becoming a maintainer

Contributors who have made sustained, high-quality contributions and who help others
through reviews and issue triage may be invited to become maintainers by the existing
maintainers. A maintainer who is no longer active may step down at any time and is then
listed as an emeritus maintainer.

## Releases

RepoDNA follows [Semantic Versioning](https://semver.org/). The command-line interface, the
configuration file, the RepositoryDNA schema (versioned separately in each artifact), and
the plugin protocol are the public interfaces. Maintainers cut releases as described in
[the development guide](docs/development.md#releasing), and every release is described in
[CHANGELOG.md](CHANGELOG.md).

## Open source, always

The local analysis, the command line, the web interface, the desktop app, and the report
formats are open source under the [Apache License 2.0](LICENSE). No feature of the local
analysis will be put behind a payment. Financial support through the links in the
[README](README.md#creator-and-support) is welcome and helps development, but it does not
buy influence over the project's decisions.

## Code of Conduct

Everyone in the project follows the [Code of Conduct](CODE_OF_CONDUCT.md), and the
maintainers enforce it.

## Changing this document

Changes to this document are proposed in a pull request and require the approval of the
lead maintainer.
