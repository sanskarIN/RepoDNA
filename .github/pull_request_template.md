## What this changes

<!-- A short description of the change. Link the issue it addresses, if any (Fixes #123). -->

## Why

<!-- The problem it solves, or the reason for the approach you chose. -->

## How it was tested

<!-- Tests you added or ran, fixtures used, and screenshots for changes to the web interface,
the desktop app, reports, or cards. -->

## Checklist

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` pass
- [ ] For web changes: `npm run format:check`, `npm run typecheck`, and `npm test` pass
- [ ] New or changed findings carry evidence, a method, and limitations, and have tests with realistic samples
- [ ] No network access, telemetry, or execution of repository code was added to the analysis
- [ ] Documentation in `docs/` is updated, and user-visible changes are listed under "Unreleased" in `CHANGELOG.md`
