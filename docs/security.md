# Security

This page covers two things: the **security signals** RepoDNA reports about a repository,
and how RepoDNA **protects you** while it analyzes repositories you may not trust. To report
a vulnerability in RepoDNA itself, see [SECURITY.md](../SECURITY.md).

## Security signals

The security stage is a defensive, static pattern matcher. It looks for credentials
committed to the repository, risky constructs in code, configuration, CI workflows, and
container definitions, and unusual file permissions.

**A clean result does not mean the code is secure, and a match does not prove a
vulnerability.** RepoDNA does not know whether a credential is valid, whether a code path
is reachable, or whether a construct is already mitigated. Dependency vulnerabilities are
not checked: RepoDNA reads manifests and lockfiles offline and has no advisory database.

```sh
repodna findings --rule security.
repodna show --section security
repodna analyze . --profile security-only
```

### Possible secrets (`security.secret`)

| Rule | Looks for |
|---|---|
| `private-key` | Private key block |
| `aws-access-key-id` | AWS access key ID |
| `aws-secret-access-key` | AWS secret access key |
| `github-token` | GitHub token |
| `gitlab-token` | GitLab personal access token |
| `slack-token` | Slack token |
| `slack-webhook` | Slack incoming webhook URL |
| `stripe-secret-key` | Stripe live secret key |
| `google-api-key` | Google API key |
| `anthropic-api-key` | Anthropic API key |
| `openai-api-key` | OpenAI API key |
| `npm-token` | npm access token |
| `pypi-token` | PyPI API token |
| `sendgrid-api-key` | SendGrid API key |
| `azure-storage-key` | Azure storage account key |
| `telegram-bot-token` | Telegram bot token |
| `url-credentials` | Password embedded in a URL |
| `jwt` | JSON Web Token |
| `env-assignment-secret` | Unquoted secret assignment, as in `.env` files |
| `generic-secret` | Hard-coded secret assigned to a secret-like name |

How candidates are handled:

- **Values are never stored.** A candidate records its rule, file, line, and a fingerprint
  computed with HMAC-SHA256 under a key derived from the scanned content. Equal
  fingerprints mean equal values within one scan ("the same value appears in 3 places"),
  but a fingerprint cannot be reversed or compared across unrelated scans.
- Placeholders are ignored: values containing `example`, `changeme`, `placeholder`,
  `dummy`, `your`, `xxxx`, `redacted`, template syntax such as `${...}` or `{{...}}`, or
  references to the environment or a secret store (`process.env`, `os.environ`, `getenv`,
  `secrets.`, `vault:`), and similar. Rules for generic secrets also require enough entropy.
- Matches in tests, fixtures, examples, and documentation are marked as such and reported
  at *attention* severity, as are low-confidence matches. Elsewhere, a high-confidence
  private key is *critical* and other candidates are *warnings*.

### Risky patterns (`security.<rule>`)

| Rule | Category | Looks for |
|---|---|---|
| `tls-verification-disabled` | code | TLS certificate verification is turned off |
| `curl-insecure` | code | curl is run with certificate checks disabled |
| `curl-pipe-shell` | code | A downloaded script is executed directly |
| `chmod-777` | code | Files are made writable by everyone |
| `yaml-unsafe-load` | code | YAML is loaded with a loader that can construct arbitrary objects |
| `pickle-deserialization` | code | Data is deserialized with pickle, which can execute code |
| `shell-command-string` | code | A command string is run through a shell |
| `eval-usage` | code | Code is evaluated from a string |
| `dangerous-html` | code | An HTML string is inserted into the page |
| `debug-enabled` | configuration | Debug mode is enabled in application settings |
| `cors-wildcard` | configuration | Cross-origin requests are allowed from any origin |
| `env-file-committed` | configuration | An environment file with values is committed |
| `workflow-pull-request-target` | CI workflow | The workflow runs on `pull_request_target` with repository secrets and write access |
| `workflow-untrusted-checkout` | CI workflow | A `pull_request_target` workflow checks out the pull request's code |
| `workflow-write-all` | CI workflow | The workflow token has write access to everything |
| `privileged-container` | container | A container runs in privileged mode |
| `docker-socket-mount` | container | The Docker socket is mounted into a container |

Patterns run on lines with comments removed and only in the file types where the construct
means something. Each finding includes a recommendation. High-confidence matches are
*warnings*, medium-confidence ones *attention*, and the rest *informational*.

### File permissions (`security.permission`)

On Unix-like systems, files that are writable by every user, or that have the setuid or
setgid bit, are reported.

### Handling findings

- **A real credential**: revoke and rotate it first. Removing it from the code does not
  remove it from the Git history.
- **An intentional fake** (a test fixture): suppress it with a reason, so it stays visible
  but no longer fails CI:

  ```toml
  [[suppress]]
  rule = "security.secret"
  path = "tests/fixtures/**"
  reason = "Intentional fake credentials used by tests"
  ```

## How RepoDNA protects you

Repositories can be hostile: an archive, a clone, or a shared directory may contain files
crafted to exploit the tools that read them. RepoDNA is built to read them safely.

**Read-only.** Analysis never writes to the repository. Discovery does not follow symbolic
links, skips version-control metadata, and reads files only up to `max_file_bytes`. (If you
enable command execution, the repository's own build and test commands can write, as they
would if you ran them yourself.)

**No commands run by default.** Build and test commands are detected, never run, unless
you enable execution in your user configuration. A repository's `repodna.toml` cannot
enable execution, plugins, or AI.

**Hardened Git.** RepoDNA runs the `git` executable with settings that stop a repository's
own configuration from running programs: `core.fsmonitor` is disabled, text-conversion
filters and external diff drivers are never used, the `ext::` and `file://` transports are
disabled, credential prompts and optional locks are off, and environment variables that
redirect Git to another repository are ignored. Every Git command has a time limit and is
stopped when you cancel.

**Safe cloning.** Only URLs RepoDNA can reason about are cloned. Transport-helper syntax
(`ext::`), local transports, option-like values, and control characters are rejected;
unencrypted `http://` and `git://` and private or loopback hosts need an explicit opt-in.
Host names are checked literally, so a public DNS name that resolves to a private address
is not detected. Credentials in URLs are never stored or displayed.

**Safe archives.** ZIP and TAR extraction rejects absolute paths and `..` traversal, skips
symbolic links, hard links, and device entries, and enforces limits (200,000 entries, 512
MB per file, 4 GB in total, and a compression ratio of 250 for ZIP entries), counting the
bytes actually written.

**Plugins** run without a shell, with a minimal environment, and with time and output
limits; they still run with your permissions, so enable only plugins you trust. See
[plugins](plugins.md#security).

**The local server** (`repodna serve`) listens on 127.0.0.1 only, requires a session
token, refuses unexpected `Host` headers (which defeats DNS rebinding) and state-changing
requests from other origins, and sends a restrictive Content Security Policy. See
[the web interface](web.md#security).

**AI** is off by default. Repository text in prompts is marked as data, never instructions;
answers are checked against the evidence that was sent; and providers off this machine need
your consent. See [AI](ai.md).

**Linear-time patterns.** Built-in and plugin regular expressions use the Rust `regex`
crate, which cannot backtrack catastrophically on crafted input.

Dependencies are checked in CI with [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
for security advisories, licenses, and sources.
