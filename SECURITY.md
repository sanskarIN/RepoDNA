# Security policy

RepoDNA is built to analyze code you may not trust, so security reports are taken
seriously. Thank you for helping keep its users safe.

## Supported versions

| Version | Security fixes |
|---|---|
| 1.0.x (the latest release) | Yes |
| Earlier development builds | No |

Fixes are released as new patch versions. Please upgrade to the latest release before
reporting, if you can.

## Reporting a vulnerability

**Please do not report vulnerabilities in public issues, pull requests, or discussions.**

Report them privately through GitHub:

1. Open the [Security tab](https://github.com/sanskarIN/RepoDNA/security) of the repository.
2. Choose **Report a vulnerability**
   (direct link: <https://github.com/sanskarIN/RepoDNA/security/advisories/new>).
3. Describe the problem, the affected version (`repodna version --json`), and how to
   reproduce it. A minimal repository, archive, or plugin that triggers the problem helps
   most. Please do not include real credentials.

If that form is not available to you, contact the maintainer through the details on
[their GitHub profile](https://github.com/sanskarIN) and ask for a private channel, without
describing the vulnerability publicly.

## What happens next

- You will receive an acknowledgment as soon as possible.
- The maintainer will confirm the problem, work on a fix, and keep you informed.
- The fix is released in a new version, together with a GitHub security advisory that
  describes the problem and the fixed versions. You will be credited in the advisory unless
  you prefer otherwise.

Please give the maintainer reasonable time to release a fix before you disclose the
problem publicly.

## Scope

Examples of what is in scope:

- Code execution caused by analyzing a repository, archive, artifact, or configuration, in
  any way other than through explicitly enabled command execution or plugins
- Escaping the analyzed directory or the output directory, for example through paths or
  links in archives or repositories
- Git settings or URLs in an analyzed repository that make RepoDNA run commands, contact
  unexpected hosts, or read files outside the repository
- A repository's `repodna.toml` enabling plugins, AI providers, command execution, or other
  settings that only the user may enable
- Secret values, absolute local paths, or credentials from URLs ending up in artifacts,
  reports, cards, or exports
- Bypassing the session token, the origin checks, or the loopback-only binding of
  `repodna serve`
- Evidence sent to an AI provider without the consent that RepoDNA requires

Not vulnerabilities, but welcome as ordinary [issues](https://github.com/sanskarIN/RepoDNA/issues/new/choose):

- A secret or risky pattern that RepoDNA's scanner misses or reports wrongly (its findings are
  signals, not guarantees)
- Crashes or slow analyses on unusual input that have no security impact
- Vulnerabilities in dependencies that are already public, unless RepoDNA is affected in a
  way that is not yet known

## How RepoDNA protects you

[The security documentation](docs/security.md#how-repodna-protects-you) describes the
safeguards: the hardened Git runner, safe cloning and archive extraction, limits on file
sizes and counts, linear-time regular expressions, the privacy of stored results, and the
permissions of plugins and AI providers.
