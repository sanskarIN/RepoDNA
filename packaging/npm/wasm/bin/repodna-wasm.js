#!/usr/bin/env node
// repodna-wasm: RepoDNA's analysis, reports, and Project DNA cards, run as WebAssembly by
// Node.js, on any system Node.js runs on. Without Git, an analysis has no history; the
// repodna command line (@sanskarin/repodna) includes it.
//
// Usage: see USAGE below.

import { readFileSync, writeFileSync } from "node:fs";
import { analyze, card, cardPng, load, report } from "../dist/node.js";

const USAGE = `Usage:
  repodna-wasm analyze <folder or archive> [--profile NAME] [--anonymize-contributors] [--output FILE]
  repodna-wasm report <analysis file> [--format html|markdown|json] [--theme NAME] [--privacy local|share|public] [--output FILE]
  repodna-wasm card <analysis file> [--dark] [--png] [--output FILE]
  repodna-wasm --version

Writes the result to standard output, or to FILE with --output. Profiles: quick, standard,
deep. Themes: professional, minimal, technical, dark.
`;

const VALUED = new Set(["--profile", "--format", "--theme", "--privacy", "--output"]);
const SWITCHES = new Set(["--anonymize-contributors", "--dark", "--png"]);

function fail(message, code = 1) {
  process.stderr.write(`repodna-wasm: ${message}\n`);
  process.exit(code);
}

function parse(argv) {
  const [command, ...rest] = argv;
  const parsed = { command, operand: null, options: {} };
  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index];
    if (VALUED.has(arg)) {
      const value = rest[index + 1];
      if (value === undefined) {
        fail(`${arg} needs a value.\n\n${USAGE}`, 2);
      }
      parsed.options[arg.slice(2)] = value;
      index += 1;
    } else if (SWITCHES.has(arg)) {
      parsed.options[arg.slice(2)] = true;
    } else if (arg.startsWith("-")) {
      fail(`unknown option ${arg}.\n\n${USAGE}`, 2);
    } else if (parsed.operand === null) {
      parsed.operand = arg;
    } else {
      fail(`unexpected argument ${arg}.\n\n${USAGE}`, 2);
    }
  }
  return parsed;
}

/** Writes progress to the terminal, when standard error is one. */
function progress(event) {
  if (process.stderr.isTTY && event.event === "stage" && event.status !== "started") {
    const time = event.milliseconds === undefined ? "" : ` (${event.milliseconds} ms)`;
    process.stderr.write(`${event.status === "completed" ? "✓" : "–"} ${event.stage}${time}\n`);
  }
}

function output(result, file) {
  if (file) {
    writeFileSync(file, result);
  } else {
    process.stdout.write(result);
  }
}

async function main() {
  const argv = process.argv.slice(2);
  if (argv.length === 0 || argv[0] === "--help" || argv[0] === "-h") {
    process.stdout.write(USAGE);
    return;
  }
  if (argv[0] === "--version" || argv[0] === "-V") {
    process.stdout.write(`repodna-wasm ${await (await load()).version()}\n`);
    return;
  }
  const { command, operand, options } = parse(argv);
  if (!["analyze", "report", "card"].includes(command)) {
    fail(`unknown command ${command}.\n\n${USAGE}`, 2);
  }
  if (operand === null) {
    fail(
      `${command} needs ${command === "analyze" ? "a folder or an archive" : "an analysis file"}.\n\n${USAGE}`,
      2,
    );
  }
  if (command === "analyze") {
    const text = await analyze(operand, {
      profile: options.profile,
      anonymizeContributors: options["anonymize-contributors"] === true,
      onProgress: progress,
    });
    output(text, options.output);
    return;
  }
  const artifact = readFileSync(operand, "utf8");
  if (command === "report") {
    const { format, theme, privacy } = options;
    output(await report(artifact, { format, theme, privacy }), options.output);
  } else if (options.png) {
    if (!options.output && process.stdout.isTTY) {
      fail("a PNG image is not shown in a terminal; give --output FILE.", 2);
    }
    output(await cardPng(artifact, { dark: options.dark === true }), options.output);
  } else {
    output(await card(artifact, { dark: options.dark === true }), options.output);
  }
}

main().catch((error) => fail(error instanceof Error ? error.message : String(error)));
