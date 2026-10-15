# RepoDNA v1.4.0 — screenshots

This release was captured with headless Chromium against the **real v1.4.0 web interface** and documentation site compiled from the development source, not a mockup.

The [media capture workflow](https://github.com/sanskarIN/RepoDNA/actions/runs/38688707790) built the CLI, WebAssembly program, web assets and documentation; RepoDNA then analyzed its own source and the browser loaded that newly generated artifact as the demo. Captures are genuine PNG screenshots at the following resolutions:

| View | Resolution | File |
|---|---|---|
| Overview, light | 2880 × 1620 | [desktop/overview-light.png](desktop/overview-light.png) |
| Overview, dark | 2880 × 1620 | [desktop/overview-dark.png](desktop/overview-dark.png) |
| Architecture, dark | 2880 × 1620 | [desktop/architecture-dark.png](desktop/architecture-dark.png) |
| Overview, phone | 1170 × 2532 | [phone/overview-light.png](phone/overview-light.png) |
| Documentation index, light | 2880 × 1620 | [docs-site/index-light.png](docs-site/index-light.png) |
| CLI documentation, dark | 2880 × 1620 | [docs-site/cli-dark.png](docs-site/cli-dark.png) |
| Documentation, phone | 1170 × 2532 | [docs-site/phone-dark.png](docs-site/phone-dark.png) |

The capture job verified PNG file signatures, dimensions, and nonempty content before pushing these files to the `images` branch. The branded logo was built as a hand-authored SVG and the matching PNG icon is available in [the media kit](../../media/v1.4.0/README.md). No version 1.3.3 screenshot was renamed to make this gallery.
