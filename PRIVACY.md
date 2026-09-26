# Privacy Policy

Last updated: September 26, 2026

This policy explains how RepoDNA handles information. It covers the `repodna` command line,
the RepoDNA desktop app, the web interface of `repodna serve`, and the web version at
<https://sanskarin.github.io/RepoDNA/>. RepoDNA is open-source software made by Sanskar and
its contributors, so everything described here can be checked in its
[source code](https://github.com/sanskarIN/RepoDNA).

## In short

- RepoDNA does not collect, sell, or share personal information. It has no accounts, no
  telemetry, no analytics, no advertising, and no tracking.
- Your code is analyzed on your own device and is never uploaded.
- RepoDNA does not use AI services.

## What RepoDNA reads

To analyze a repository, RepoDNA reads its files and Git history on your device: file names
and contents, and commit details such as author names, dates, and messages. Author e-mail
addresses are used only to tell contributors apart and are never stored. The results are
shown to you and, unless you choose otherwise, stored on your device. Nothing is sent to the
author of RepoDNA or to anyone else.

## What is stored on your device

- The command line and the desktop app store analyses, a cache of per-file results, and your
  settings in a folder on your device. The
  [privacy guide](https://github.com/sanskarIN/RepoDNA/blob/main/docs/privacy.md) says where
  it is and how to delete it; `repodna clean` and `repodna cache reset` remove stored data,
  and `--no-store` analyzes without storing anything.
- Reports, cards, badges, exports, and diagnostics files are created only when you ask for
  them, where you choose.
- The web interface keeps your theme choice in your browser's local storage. When it is
  served by `repodna serve`, your browser also keeps the sign-in token for that local
  server, in a session cookie or the tab's session storage. The web version sets no
  cookies.
- Files you open in the web version are read in your browser and are not uploaded.

## When RepoDNA uses the network

Only when you ask it to:

- To analyze a Git URL, RepoDNA clones the repository from the host you name, using Git on
  your device. That host sees the request as it would see any clone.
- Links to websites, such as GitHub or the support pages, open in your browser.

`repodna serve` accepts connections only from your own computer (127.0.0.1) and requires a
session token.

## The web version

The web version is a static website hosted by GitHub Pages. RepoDNA adds no cookies,
analytics, or tracking to it, and it loads nothing from other websites. As with any site it
hosts, GitHub receives technical information such as your IP address when you visit; see the
[GitHub General Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Other websites and support payments

Websites you open through RepoDNA's links, such as GitHub, Gumroad, Buy Me a Coffee, and
Razorpay, have their own privacy policies. If you support RepoDNA with a payment, the payment
is handled by that service; RepoDNA never sees or stores your payment details.

## Children

RepoDNA does not collect personal information from anyone, including children.

## Changes

Changes to this policy are published in the RepoDNA repository with a new date at the top,
and [the history of this file](https://github.com/sanskarIN/RepoDNA/commits/main/PRIVACY.md)
shows every change.

## Contact

For questions about this policy, open an
[issue](https://github.com/sanskarIN/RepoDNA/issues) or contact the maintainer through
[their GitHub profile](https://github.com/sanskarIN). Report security problems privately, as
described in the [security policy](https://github.com/sanskarIN/RepoDNA/blob/main/SECURITY.md).

See also the [Terms of Use](https://github.com/sanskarIN/RepoDNA/blob/main/TERMS.md).
