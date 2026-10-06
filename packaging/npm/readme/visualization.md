# RepoDNA visualization

Framework-neutral chart geometry from [RepoDNA](https://github.com/sanskarIN/RepoDNA):
number formats, palettes for light and dark themes, linear scales, treemaps, layered graph
layouts, and weekday-by-hour heatmaps. Every function returns plain data (positions,
paths, colors) that any renderer can draw; the RepoDNA web interface and desktop app draw
their charts with it.

```ts
import { thousands, treemap } from "@sanskarin/repodna-visualization";

// Rectangles for two directories in a 640 by 360 area, largest first.
const cells = treemap(
  [
    { value: 1200, data: "src" },
    { value: 300, data: "tests" },
  ],
  640,
  360,
);
console.log(thousands(12345), cells);
```

## Install

```sh
npm install @sanskarin/repodna-visualization
```

The package is on npmjs.com and on GitHub Packages, which asks for a GitHub token even to
install public packages; see the [installation guide][guide].

Licensed under the Apache License 2.0.

[guide]: https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md#npm-packages
