# Frequently asked questions

## General

**What is RepoDNA?**
An open-source, local-first tool that analyzes a software repository (its structure,
languages, architecture, dependencies, history, tests, build, documentation, and security
signals) and explains every conclusion with the evidence behind it. It runs as a command
line, a local web interface, and a desktop app.

**Is it free?**
Yes. RepoDNA is open source under the [Apache License 2.0](../LICENSE). Every feature works
without an account, a subscription, or a network connection.

**Does it upload my code?**
No. Analysis runs on your machine and results are stored on your machine. RepoDNA uses the
network only to clone a Git URL you give it. It collects no telemetry. See
[privacy](privacy.md).

**Does it use AI?**
No. RepoDNA 1.0 does not use AI: the analysis is deterministic, and every conclusion comes
with the files, commits, and measurements behind it. Optional AI explanations built from
that evidence are planned for a later release; see the [roadmap](../ROADMAP.md).

**Which languages does it support?**
65 built-in languages: 25 with lexical analysis (imports, symbols, complexity) and 40 with
line counting. See the [list](../README.md#supported-languages). Plugins can add more.

## Results

**How accurate is it?**
It is as accurate as static, lexical analysis can be, and it tells you how sure it is. Each
finding has a confidence level and states its method and limitations. Imports computed at
run time, reflection, and code generated at build time are invisible to it. Treat findings
as signals that tell you where to look.

**Is this a code quality score?**
No. RepoDNA reports measurements and signals, not grades. The DNA fingerprint describes a
repository (for example how many languages it uses or how concentrated its changes are);
a high or low value is neither good nor bad on its own.

**Why is a finding reported that does not apply to my project?**
Heuristics have false positives. Suppress it with a reason in `repodna.toml`
(see [configuration](configuration.md#suppress-accept-known-findings)), and if the rule
is wrong in general, please [open an issue](https://github.com/sanskarIN/RepoDNA/issues/new/choose).

**Does it find vulnerabilities in my dependencies?**
No. It reads manifests and lockfiles offline and has no advisory database. It does look
for committed secrets and risky code patterns; see [security](security.md).

**Does it judge contributors?**
No. History describes how a repository was worked on, not the quality of anyone's work.
Contributor data can be anonymized (`--anonymize`) or replaced with pseudonyms in shared
reports (`--privacy public`).

**Why is there no history?**
History needs Git and the root of a working tree; see
[troubleshooting](troubleshooting.md#analysis).

## Usage

**Can I analyze a repository I do not trust?**
Yes, that is a design goal: analysis is read-only, Git runs with hardened settings, archives
are extracted safely, and nothing from the repository is executed unless you enable command
execution in your own configuration. See [security](security.md#how-repodna-protects-you).

**How big a repository can it handle?**
The fixture with 5,000 files and 120 commits takes under a second with the standard
profile on a machine with four logical CPUs; see the [benchmarks](../benchmarks/README.md).
Limits such as `max_commits` and `max_file_bytes` keep very large repositories manageable.

**Can I use it in CI?**
Yes: `repodna ci --fail-on warning` exits with code 5 when findings at that severity exist,
and `--format github` adds annotations. See the [example workflow](../examples/ci/repodna.yml).

**Is there a GitHub Action?**
Not yet. The [example workflow](../examples/ci/repodna.yml) installs the release binary and
runs `repodna ci`.

**Can I share results without sharing my code?**
Yes. Reports, cards, and `.repodna` exports contain the analysis, not the source code, and
privacy presets remove more before you share. Anyone can open a `.repodna` file with
`repodna import` or in the web interface.

**Where are my analyses stored?**
See [privacy](privacy.md#what-is-stored-and-where). `repodna list` shows them and
`repodna clean` deletes them.

## Project

**Who makes RepoDNA?**
RepoDNA was created by [Sanskar](https://github.com/sanskarIN) and is developed in the open
with its contributors. See [MAINTAINERS.md](../MAINTAINERS.md).

**How can I help?**
Report bugs, suggest features, add a language or an ecosystem, improve the documentation,
or share RepoDNA with others. See [CONTRIBUTING.md](../CONTRIBUTING.md). You can also
support the project through [Buy Me A Coffee](https://www.buymeacoffee.com/sanskarIN) or
[Razorpay](https://www.razorpay.me/@sanskarIN).
