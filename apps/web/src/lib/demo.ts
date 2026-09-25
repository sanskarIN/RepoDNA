// The bundled demo: real RepoDNA artifacts shipped with the web interface, so it can be
// explored offline and without analyzing anything first.

import { parseArtifact, type LoadedArtifact } from "@repodna/schema";

export interface DemoEntry {
  file: string;
  title: string;
  description: string;
}

/** Lists the bundled demo artifacts (empty when this build has none). */
export async function demoIndex(): Promise<DemoEntry[]> {
  try {
    const response = await fetch("./demo/index.json", { cache: "no-store" });
    if (!response.ok) {
      return [];
    }
    const value: unknown = await response.json();
    if (!Array.isArray(value)) {
      return [];
    }
    return value.filter(
      (entry): entry is DemoEntry =>
        typeof entry === "object" &&
        entry !== null &&
        typeof (entry as DemoEntry).file === "string" &&
        /^[\w.-]+\.json$/.test((entry as DemoEntry).file) &&
        typeof (entry as DemoEntry).title === "string",
    );
  } catch {
    return [];
  }
}

/** Loads one bundled demo artifact. */
export async function loadDemo(entry: DemoEntry): Promise<LoadedArtifact> {
  const response = await fetch(`./demo/${entry.file}`);
  if (!response.ok) {
    throw new Error(`The demo file ${entry.file} could not be loaded (${response.status}).`);
  }
  return parseArtifact(await response.text());
}

/** Reads an artifact the user picked or dropped. */
export async function loadFile(file: File): Promise<LoadedArtifact> {
  if (file.size > 512 * 1024 * 1024) {
    throw new Error("This file is larger than 512 MB, which is more than an artifact should be.");
  }
  return parseArtifact(await file.text());
}
