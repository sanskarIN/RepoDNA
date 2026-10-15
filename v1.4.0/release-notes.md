# RepoDNA 1.4.0 — release notes

**Status:** Public stable release, published on GitHub on October 15, 2026. Download all 15 verified release attachments at the [official GitHub Release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.4.0).

Changes developed on the
[development branch](https://github.com/sanskarIN/RepoDNA/tree/release-v1.4.0-development):

## Identity and application startup

- New **hand-authored SVG** logo, combining two DNA strands with code-graph
  nodes, and a complementary wordmark. The design uses only vector primitives:
  no AI-generated image or embedded third-party font.
- Matching favicon, PWA icons, and desktop application icons, generated from
  that same source art.
- A branded splash screen that appears during application initialization,
  with an HTML fallback before the JavaScript bundle loads. Its progress
  treatment honors reduced-motion settings and has no forced delay.

## Support and documentation

- The official [PayPal.Me](https://paypal.me/SanskarCodes) support link
  alongside Buy Me a Coffee and Razorpay, including the app's About page
  and GitHub Sponsor button.
- A dedicated [brand guide](https://github.com/sanskarIN/RepoDNA/blob/release-v1.4.0-development/docs/branding.md)
  for the logo, palette, startup behavior, and raster assets.
- Corrected FAQ guidance for the official GitHub Action.

## Bug fixes and validation

- Preserve additional literal question marks within deep-link query values.
- Restore focus to the main content when startup initialization completes.
- Verify icon dimensions, package manifests, lockfile consistency,
  startup tests, and CI on supported operating systems.

## Release verification

The annotated `v1.4.0` tag and public GitHub Release were verified after publication. The release has 15 attached files including cross-platform binaries, desktop installers, WebAssembly and `SHA256SUMS.txt`.

## Media

- [Launch artwork](https://github.com/sanskarIN/RepoDNA/blob/images/media/v1.4.0/promo/launch-16x9.png) — hand-authored vector design rendered to PNG.
- [Official logo](https://github.com/sanskarIN/RepoDNA/blob/images/media/v1.4.0/branding/repodna-mark.svg) — original hand-written SVG.
- [v1.4.0 screenshots](https://github.com/sanskarIN/RepoDNA/tree/images/screenshots/v1.4.0) — actual web interface and generated docs captures.
