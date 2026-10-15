# RepoDNA 1.4.0 — release notes (development draft)

**Status:** In development. This document does not indicate that a
`v1.4.0` GitHub release, tag, or downloadable binaries exist.

Planned changes tracked in the
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

## Before publication

Required: finish development, pass all CI/security checks, capture versioned
screenshots and media on the `images` branch, prepare release downloads
and hashes, merge changes to `main`, create `v1.4.0` only when approved
for publication, and confirm that the GitHub Release assets are attached.
