#!/usr/bin/env node
// Serves the RepoDNA web interface on this machine, for opening analysis files and the demo
// without an internet connection.
//
// Usage: repodna-web [--port 8080] [--host 127.0.0.1]

import { readFileSync } from "node:fs";
import { createWebServer, root } from "../lib/index.js";

const { version } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const USAGE = `Serves the RepoDNA web interface.

Usage: repodna-web [--port PORT] [--host HOST]

  --port PORT  Port to listen on (default 8080; 0 picks a free one)
  --host HOST  Address to listen on (default 127.0.0.1, this machine only)
  --version    Print the version
  --help       Print this help
`;

function fail(message) {
  process.stderr.write(`repodna-web: ${message}\n\n${USAGE}`);
  process.exit(2);
}

const options = { port: 8080, host: "127.0.0.1" };
const args = process.argv.slice(2);
for (let index = 0; index < args.length; index += 1) {
  const arg = args[index];
  if (arg === "--help" || arg === "-h") {
    process.stdout.write(USAGE);
    process.exit(0);
  } else if (arg === "--version" || arg === "-V") {
    process.stdout.write(`${version}\n`);
    process.exit(0);
  } else if (arg === "--port" || arg === "--host") {
    const value = args[index + 1];
    if (value === undefined) {
      fail(`${arg} needs a value.`);
    }
    index += 1;
    if (arg === "--port") {
      const port = Number(value);
      if (!Number.isInteger(port) || port < 0 || port > 65535) {
        fail(`${value} is not a port.`);
      }
      options.port = port;
    } else {
      options.host = value;
    }
  } else {
    fail(`unknown option ${arg}.`);
  }
}

const server = createWebServer(root);
server.on("error", (error) => {
  process.stderr.write(`repodna-web: ${error.message}\n`);
  process.exit(1);
});
server.listen(options.port, options.host, () => {
  const { port } = server.address();
  const host = options.host.includes(":") ? `[${options.host}]` : options.host;
  const local = ["127.0.0.1", "localhost", "::1"].includes(options.host);
  process.stdout.write(
    `The RepoDNA web interface ${version} is at http://${host}:${port}/` +
      `${local ? " (this machine only)" : ""}.\n` +
      "Analysis files you open are read in the browser and never sent to this server.\n" +
      "Press Ctrl+C to stop.\n",
  );
});
