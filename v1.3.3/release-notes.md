# RepoDNA 1.3.3: release notes (prepared)

These notes are prepared for tag `v1.3.3`. The official release page and downloads
will appear at <https://github.com/sanskarIN/RepoDNA/releases/tag/v1.3.3>
once the release workflow finishes. Do not treat this document as confirmation that
the release has been published.

## Improvements

### WebAssembly command-line experience

- Invalid options fail immediately, before the WebAssembly engine starts:
  missing values, unsupported report format/theme/privacy values, options used with
  the wrong command, and duplicate flags.
- A report or DNA card cannot accidentally overwrite the analysis artifact used
  as input, even when the output filename is a hard-link alias of the same file.
- Additional regression tests cover these argument-validation and file-safety cases.

### Documentation-site reliability

- Relative `href` and `src` attributes in Markdown's raw HTML resolve properly
  with either single or double quotes.
- Invalid percent-encoding in links is reported by the documentation builder rather
  than crashing the build.
- Regression tests verify the HTML-link and malformed-link cases.

### Dependency maintenance

- Rust and npm workspace packages and desktop bundle versions are synchronized to 1.3.3.
- Updated both Cargo lockfiles to `rustls 0.23.46`, which addresses
  RUSTSEC-2026-0346, RUSTSEC-2026-0347, RUSTSEC-2026-0348 and RUSTSEC-2026-0349.

## Compatibility

- This is a patch release; it does not change the analysis artifact's schema.
- The default local-first privacy model is unchanged.

## Before publication

The maintainers must confirm all required CI jobs, verify release archives and checksums,
merge the release pull request, and create the annotated `v1.3.3` tag.
