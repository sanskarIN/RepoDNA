# Media kit

Images of RepoDNA for posts, articles, and talks: promo images, Project DNA cards, and
screenshots of the web interface on a desktop and a phone, in light and dark. Each release
with promo images has a folder of its own, and this page shows the newest, made for 1.3.1;
1.2.2, a small follow-up, has screenshots only. Every screenshot shows RepoDNA's analysis
of its own repository: for 1.3.1, at its last commit before the images were added (791
commits, 670 files). Like the rest of this repository, the images are licensed under the
[Apache License 2.0](https://github.com/sanskarIN/RepoDNA/blob/main/LICENSE).

| Version | Promo images | Project DNA cards | Screenshots |
|---|---|---|---|
| 1.3.1 | [`v1.3.1/promo`](v1.3.1/promo) | [`v1.3.1/cards`](v1.3.1/cards) | [`screenshots/v1.3.1`](../screenshots/README.md#131) |
| [1.3.0](#130) | [`v1.3.0/promo`](v1.3.0/promo) | [`v1.3.0/cards`](v1.3.0/cards) | [`screenshots/v1.3.0`](../screenshots/README.md#130) |
| 1.2.2 | — | — | [`screenshots/v1.2.2`](../screenshots/README.md#122) |
| [1.2.1](#121) | [`v1.2.1/promo`](v1.2.1/promo) | [`v1.2.1/cards`](v1.2.1/cards) | [`screenshots/v1.2.1`](../screenshots/README.md#121) |
| [1.2.0](#120) | [`v1.2.0/promo`](v1.2.0/promo) | [`v1.2.0/cards`](v1.2.0/cards) | [`screenshots/v1.2.0`](../screenshots/README.md#120) |

- [Promo images](#promo-images)
- [Project DNA cards](#project-dna-cards)
- [Desktop screenshots](#desktop-screenshots)
- [Phone screenshots](#phone-screenshots)
- [Images of 1.3.0](#130)
- [Images of 1.2.1](#121)
- [Images of 1.2.0](#120)
- [How they were made](#how-they-were-made)

## Promo images

| Image | Size | Fits |
|---|---|---|
| [`v1.3.1/promo/launch-16x9.png`](v1.3.1/promo/launch-16x9.png) | 3200 × 1800 (16:9) | X, LinkedIn, Facebook, and slides |
| [`v1.3.1/promo/whats-new-1x1.png`](v1.3.1/promo/whats-new-1x1.png) | 2160 × 2160 (1:1) | Instagram and LinkedIn posts |
| [`v1.3.1/promo/phones-4x5.png`](v1.3.1/promo/phones-4x5.png) | 2160 × 2700 (4:5) | Instagram portrait posts |

<img src="v1.3.1/promo/launch-16x9.png" alt="RepoDNA 1.3.1 launch image: the headline Understand your codebase. See its DNA., analysis right in the browser with nothing uploaded, the command npm install --global @sanskarin/repodna, and the web interface on a desktop and a phone" width="800">

<img src="v1.3.1/promo/whats-new-1x1.png" alt="What's new in RepoDNA 1.3.1: analysis in the browser of a folder or an archive, reports and cards there too, a web version that works offline, a GitHub Action, Windows on Arm, and RepoDNA as WebAssembly with npx @sanskarin/repodna-wasm" width="395"> <img src="v1.3.1/promo/phones-4x5.png" alt="RepoDNA 1.3.1 on two phones: analyzing a repository in the browser in the light theme, and the Project DNA card with Download SVG and Download PNG in the dark theme" width="316">

## Project DNA cards

The cards that `repodna card --format png` makes for RepoDNA itself, at the size link
previews use: 2400 × 1260, about 1.91:1.

| Image | Theme |
|---|---|
| [`v1.3.1/cards/dna-card-light.png`](v1.3.1/cards/dna-card-light.png) | Light |
| [`v1.3.1/cards/dna-card-dark.png`](v1.3.1/cards/dna-card-dark.png) | Dark |

<img src="v1.3.1/cards/dna-card-light.png" alt="RepoDNA's Project DNA card in the light theme" width="395"> <img src="v1.3.1/cards/dna-card-dark.png" alt="RepoDNA's Project DNA card in the dark theme" width="395">

## Desktop screenshots

2880 × 1620 (16:9), taken in a 1440 × 810 window at twice the resolution. The module map
of the architecture view is 3360 × 1890, from a 1680 × 945 window, so that every module
fits. They are kept in [`screenshots/v1.3.1`](../screenshots/README.md#131), next to the
screenshots of earlier versions and of the desktop app.

| | |
|---|---|
| ![Overview in the light theme: key numbers, the language map, and the Project DNA dimensions](../screenshots/v1.3.1/desktop/overview-light.png) | ![Overview in the dark theme](../screenshots/v1.3.1/desktop/overview-dark.png) |
| [`overview-light.png`](../screenshots/v1.3.1/desktop/overview-light.png) | [`overview-dark.png`](../screenshots/v1.3.1/desktop/overview-dark.png) |
| ![Architecture in the dark theme: the module map in dependency layers](../screenshots/v1.3.1/desktop/architecture-dark.png) | ![History in the light theme: commits over time and when commits happen](../screenshots/v1.3.1/desktop/history-light.png) |
| [`architecture-dark.png`](../screenshots/v1.3.1/desktop/architecture-dark.png) | [`history-light.png`](../screenshots/v1.3.1/desktop/history-light.png) |
| ![History in the dark theme](../screenshots/v1.3.1/desktop/history-dark.png) | ![Time Machine in the light theme: a snapshot with its languages and largest areas](../screenshots/v1.3.1/desktop/time-machine-light.png) |
| [`history-dark.png`](../screenshots/v1.3.1/desktop/history-dark.png) | [`time-machine-light.png`](../screenshots/v1.3.1/desktop/time-machine-light.png) |
| ![Hotspots in the dark theme: the hotspot map](../screenshots/v1.3.1/desktop/hotspots-dark.png) | ![Findings in the dark theme: the severity filters and the findings](../screenshots/v1.3.1/desktop/findings-dark.png) |
| [`hotspots-dark.png`](../screenshots/v1.3.1/desktop/hotspots-dark.png) | [`findings-dark.png`](../screenshots/v1.3.1/desktop/findings-dark.png) |
| ![Files in the light theme: what the files are and where the lines are](../screenshots/v1.3.1/desktop/files-light.png) | ![Search in the dark theme: a view, files, and findings for one query](../screenshots/v1.3.1/desktop/search-dark.png) |
| [`files-light.png`](../screenshots/v1.3.1/desktop/files-light.png) | [`search-dark.png`](../screenshots/v1.3.1/desktop/search-dark.png) |
| ![The start page in the light theme: analyzing a repository in the browser, and the analysis it opened under Recent analyses](../screenshots/v1.3.1/desktop/start-light.png) | |
| [`start-light.png`](../screenshots/v1.3.1/desktop/start-light.png) | |

## Phone screenshots

1170 × 2532, taken on a 390 × 844 screen at three times the resolution, for stories and
portrait posts.

| | | | |
|---|---|---|---|
| <img src="../screenshots/v1.3.1/phone/overview-light.png" alt="Overview on a phone in the light theme" width="180"> | <img src="../screenshots/v1.3.1/phone/menu-light.png" alt="The navigation menu open on a phone in the light theme" width="180"> | <img src="../screenshots/v1.3.1/phone/history-light.png" alt="History on a phone in the light theme" width="180"> | <img src="../screenshots/v1.3.1/phone/hotspots-light.png" alt="Hotspots on a phone in the light theme" width="180"> |
| [`overview-light.png`](../screenshots/v1.3.1/phone/overview-light.png) | [`menu-light.png`](../screenshots/v1.3.1/phone/menu-light.png) | [`history-light.png`](../screenshots/v1.3.1/phone/history-light.png) | [`hotspots-light.png`](../screenshots/v1.3.1/phone/hotspots-light.png) |
| <img src="../screenshots/v1.3.1/phone/overview-dark.png" alt="Overview on a phone in the dark theme" width="180"> | <img src="../screenshots/v1.3.1/phone/menu-dark.png" alt="The navigation menu open on a phone in the dark theme" width="180"> | <img src="../screenshots/v1.3.1/phone/history-dark.png" alt="History on a phone in the dark theme" width="180"> | <img src="../screenshots/v1.3.1/phone/hotspots-dark.png" alt="Hotspots on a phone in the dark theme" width="180"> |
| [`overview-dark.png`](../screenshots/v1.3.1/phone/overview-dark.png) | [`menu-dark.png`](../screenshots/v1.3.1/phone/menu-dark.png) | [`history-dark.png`](../screenshots/v1.3.1/phone/history-dark.png) | [`hotspots-dark.png`](../screenshots/v1.3.1/phone/hotspots-dark.png) |

## 1.3.0

The images of RepoDNA 1.3.0 show its analysis of this repository at its last commit before
the images were added (731 commits, 616 files). Its screenshots are in
[`screenshots/v1.3.0`](../screenshots/README.md#130).

| Image | Size |
|---|---|
| [`v1.3.0/promo/launch-16x9.png`](v1.3.0/promo/launch-16x9.png) | 3200 × 1800 (16:9) |
| [`v1.3.0/promo/whats-new-1x1.png`](v1.3.0/promo/whats-new-1x1.png) | 2160 × 2160 (1:1) |
| [`v1.3.0/promo/phones-4x5.png`](v1.3.0/promo/phones-4x5.png) | 2160 × 2700 (4:5) |
| [`v1.3.0/cards/dna-card-light.png`](v1.3.0/cards/dna-card-light.png) | 2400 × 1260 |
| [`v1.3.0/cards/dna-card-dark.png`](v1.3.0/cards/dna-card-dark.png) | 2400 × 1260 |

<img src="v1.3.0/promo/launch-16x9.png" alt="RepoDNA 1.3.0 launch image: the headline Understand your codebase. See its DNA., the command npm install --global @sanskarin/repodna, and the web interface on a desktop and a phone" width="395"> <img src="v1.3.0/cards/dna-card-light.png" alt="RepoDNA's Project DNA card of 1.3.0 in the light theme" width="395">

<img src="v1.3.0/promo/whats-new-1x1.png" alt="What's new in RepoDNA 1.3.0: recent analyses, reloads that keep your place, easier ways through an analysis, any table as CSV, an installable web version, and npx @sanskarin/repodna-web to run it offline" width="395"> <img src="v1.3.0/promo/phones-4x5.png" alt="RepoDNA 1.3.0 on two phones: the end of a view with Download CSV and links to the previous and next views in the light theme, and recent analyses on the start page in the dark theme" width="316">

## 1.2.1

The images of RepoDNA 1.2.1 show its analysis of this repository at the `v1.2.1` tag (626
commits, 545 files). Its screenshots are in [`screenshots/v1.2.1`](../screenshots/README.md#121).

| Image | Size |
|---|---|
| [`v1.2.1/promo/launch-16x9.png`](v1.2.1/promo/launch-16x9.png) | 3200 × 1800 (16:9) |
| [`v1.2.1/promo/whats-new-1x1.png`](v1.2.1/promo/whats-new-1x1.png) | 2160 × 2160 (1:1) |
| [`v1.2.1/promo/phones-4x5.png`](v1.2.1/promo/phones-4x5.png) | 2160 × 2700 (4:5) |
| [`v1.2.1/cards/dna-card-light.png`](v1.2.1/cards/dna-card-light.png) | 2400 × 1260 |
| [`v1.2.1/cards/dna-card-dark.png`](v1.2.1/cards/dna-card-dark.png) | 2400 × 1260 |

<img src="v1.2.1/promo/launch-16x9.png" alt="RepoDNA 1.2.1 launch image: the headline Understand your codebase. See its DNA., the command npm install --global @sanskarin/repodna, and the web interface on a desktop and a phone" width="395"> <img src="v1.2.1/cards/dna-card-light.png" alt="RepoDNA's Project DNA card of 1.2.1 in the light theme" width="395">

<img src="v1.2.1/promo/whats-new-1x1.png" alt="What's new in RepoDNA 1.2.1: the npm packages on npmjs.com with no token needed, an interface easier to read and navigate, tables made for phones, the Project DNA card back in repodna serve, clearer clone errors, and accessible HTML reports" width="395"> <img src="v1.2.1/promo/phones-4x5.png" alt="RepoDNA 1.2.1 on two phones: commands to copy in the light theme, and a wide table with a shade on its edge in the dark theme" width="316">

## 1.2.0

The images of RepoDNA 1.2.0 show its analysis of this repository at the `v1.2.0` tag (513
commits, 459 files). Its screenshots are in [`screenshots/v1.2.0`](../screenshots/README.md#120).

| Image | Size |
|---|---|
| [`v1.2.0/promo/launch-16x9.png`](v1.2.0/promo/launch-16x9.png) | 3200 × 1800 (16:9) |
| [`v1.2.0/promo/whats-new-1x1.png`](v1.2.0/promo/whats-new-1x1.png) | 2160 × 2160 (1:1) |
| [`v1.2.0/promo/phones-4x5.png`](v1.2.0/promo/phones-4x5.png) | 2160 × 2700 (4:5) |
| [`v1.2.0/cards/dna-card-light.png`](v1.2.0/cards/dna-card-light.png) | 2400 × 1260 |
| [`v1.2.0/cards/dna-card-dark.png`](v1.2.0/cards/dna-card-dark.png) | 2400 × 1260 |

<img src="v1.2.0/promo/launch-16x9.png" alt="RepoDNA 1.2.0 launch image: the headline Understand your codebase. See its DNA., beside the web interface on a desktop and a phone" width="395"> <img src="v1.2.0/cards/dna-card-light.png" alt="RepoDNA's Project DNA card of 1.2.0 in the light theme" width="395">

<img src="v1.2.0/promo/whats-new-1x1.png" alt="What's new in RepoDNA 1.2.0: a web interface image, npm packages, a layout made for phones, one-click copy, smarter tables and search, and reports that fit any screen" width="395"> <img src="v1.2.0/promo/phones-4x5.png" alt="RepoDNA 1.2.0 on two phones: the overview in the light theme and history in the dark theme" width="316">

## How they were made

The screenshots show the web interface of each version in Chromium, with an analysis of
this repository made by `repodna analyze --format json` of the same version. They were
rendered with the Inter and JetBrains Mono fonts; the interface itself uses the system
font of the computer it runs on. The promo images frame those screenshots, along with
phone screenshots of other views taken the same way, and the cards come from
`repodna card --format png`, with `--dark` for the dark one. From 1.3.1, the web interface
in the screenshots was built with its WebAssembly analysis, as the web version is.

To make images of your own repository, run `repodna card --format png` for its
[Project DNA card](https://github.com/sanskarIN/RepoDNA/blob/main/docs/dna-cards.md), and open its analysis in the
[web version](https://sanskarin.github.io/RepoDNA/) or with `repodna serve` to take
screenshots.
