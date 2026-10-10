# Privacy Policy

Last updated: October 10, 2026

This policy explains how RepoDNA handles information. It covers the `repodna` command line,
the RepoDNA desktop app, the web interface of `repodna serve`, and the web version at
<https://sanskarin.github.io/RepoDNA/>. RepoDNA is open-source software made by Sanskar and
its contributors, so everything described here can be checked in its
[source code](https://github.com/sanskarIN/RepoDNA).

## In short

- RepoDNA does not collect, sell, or share personal information. It has no accounts, no
  telemetry, no analytics, no advertising, and no tracking.
- Your code is analyzed on your own device and is not uploaded. The one exception is an
  option you turn on yourself: short excerpts of source files sent to an AI provider you set
  up (see "AI explanations" below).
- AI explanations are optional and off by default. RepoDNA contacts an AI provider only if
  you set one up and ask for an explanation.

## What RepoDNA reads

To analyze a repository, RepoDNA reads its files and Git history on your device: file names
and contents, and commit details such as author names, dates, and messages. Author e-mail
addresses are used only to tell contributors apart and are never stored. The results are
shown to you and, unless you choose otherwise, stored on your device. Nothing is sent to the
author of RepoDNA or to anyone else, except what you choose to send to an AI provider, as
described in "AI explanations" below.

## What is stored on your device

- The command line and the desktop app store analyses, a cache of per-file results, the AI
  explanations you ask for, and your settings in a folder on your device. The
  [privacy guide](https://github.com/sanskarIN/RepoDNA/blob/main/docs/privacy.md) says where
  it is and how to delete it; `repodna clean` and `repodna cache reset` remove stored data,
  and `--no-store` analyzes without storing anything.
- Reports, cards, badges, exports, and diagnostics files are created only when you ask for
  them, where you choose.
- The web interface keeps your theme choice in your browser's local storage. When it is
  served by `repodna serve`, your browser also keeps the sign-in token for that local
  server, in a session cookie or the tab's session storage. The web version sets no
  cookies.
- The web interface also keeps the last five analysis files you opened, with their
  contents, in your browser's storage (IndexedDB), so that they can be opened again from
  the start page and after a reload, and remembers in the tab's session storage which
  analysis that tab has open. You can remove them one by one on the start page, forget
  them all or turn this off in Settings, or clear the site's data in your browser.
- Files you open in the web version are read in your browser and are not uploaded. When
  you analyze a folder or an archive there, the web version reads its files in your
  browser, with RepoDNA's analysis built as WebAssembly, and sends them nowhere. The
  analysis it makes is kept with the recent analyses, as an opened file is.
- So that it works without a connection, the web version keeps its own files (the page,
  its code, the demo, and the analysis program) in your browser's cache storage. It never
  keeps your files or analyses there. Clearing the site's data in your browser removes
  them.

## When RepoDNA uses the network

Only when you ask it to:

- To analyze a Git URL, RepoDNA clones the repository from the host you name, using Git on
  your device. That host sees the request as it would see any clone.
- If you set up an online AI provider and ask for an explanation, RepoDNA sends the request
  to that provider, as described in "AI explanations" below.
- Links to websites, such as GitHub or the support pages, open in your browser.

`repodna serve` accepts connections only from your own computer (127.0.0.1) and requires a
session token.

## AI explanations

`repodna explain` can describe an analysis in prose with an AI model. It is off until you set
up a provider in your RepoDNA user configuration: a program or server on your own device, or
an online service such as the Anthropic API or a service compatible with the OpenAI API. A
repository's own configuration cannot turn it on.

When you ask for an explanation, RepoDNA sends the provider a selection of facts from the
analysis, such as file, module, and dependency names, measurements, findings, and commit
subjects, and your question if you ask one. It never sends contributor names or e-mail
addresses. Source code is sent only if you turn on `ai.include_source_excerpts`, and then
only the first lines of a few files, with likely secrets removed. `repodna explain --dry-run`
shows exactly what would be sent, without sending anything.

A provider that is not on your own computer is used only after you allow it, with
`privacy.remote_ai = true` in your user configuration or `--allow-remote-ai` for one run.
That provider receives your request and handles it under its own terms and privacy policy;
RepoDNA's author does not receive it. Your API key is read from an environment variable when
it is needed and is never stored by RepoDNA. Explanations are cached on your device, and
`repodna cache clear` removes them.

## The web version

The web version is a static website hosted by GitHub Pages. RepoDNA adds no cookies,
analytics, or tracking to it, and it loads nothing from other websites. It analyzes
repositories in your browser: the files you choose are read there and are never sent to
GitHub, to RepoDNA's author, or to anyone else. As with any site it
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
