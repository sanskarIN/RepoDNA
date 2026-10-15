# RepoDNA visual identity

The v1.4.0 logo is **hand-authored SVG**, constructed from paths, circles, and
text in the repository. It does not contain generated artwork, external
images, external fonts, or embedded remote resources.

- [Symbol](../apps/web/public/favicon.svg), scalable 128 × 128.
- [Wordmark](../brand/repodna-logo.svg), scalable 640 × 160.
- [Splash screen](../apps/web/src/components/SplashScreen.tsx), using the
  identical symbol during application initialization.
- PWA icon PNGs and Tauri desktop icons must be rendered from the SVG
  using the checked-in vector icon; never rescale old icons or redraw it.

## Palette

| Usage | Hex |
|---|---|
| Deep navy | `#102338` |
| Helix blue | `#3987e5` |
| Helix teal | `#22c9b4` |
| Pale graph nodes | `#e5f5ff` |
| Wordmark blue | `#2672ce` |

The icon combines an intertwined DNA helix (a repository's fingerprint)
with graph edges and nodes (the codebase's relationships).

## Accessibility and startup

The startup branding is not a blocking timer: it appears until the
initial local state is ready, after which the regular interface loads.
CSS respects `prefers-reduced-motion`. Branding is informational
and does not trigger external requests.

Test light/dark startup, narrow screens, PWA installation, Tauri desktop
window, and the pre-JavaScript fallback. The fallback must not strand
users when JavaScript is disabled; a `noscript` warning is present.

## Updating icons

The release-preparation workflow renders the same `favicon.svg`
with librsvg and Pillow into icons for the web and native platforms,
then validates required sizes.
