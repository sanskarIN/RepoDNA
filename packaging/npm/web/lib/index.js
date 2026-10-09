// The RepoDNA web interface, built: a static application that any web server can serve
// from any path. `root` is the directory that holds its index.html.

import { fileURLToPath } from "node:url";

/** The directory of the built web interface, with a trailing separator. */
export const root = fileURLToPath(new URL("../dist/", import.meta.url));

export { createWebServer, HEADERS } from "./server.js";
