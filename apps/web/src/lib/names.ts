// Display names for identifiers stored in artifacts.

import type { RepositoryDna } from "@repodna/schema";

/** Returns a function that turns a language identifier ("rust") into its name ("Rust"). */
export function languageNamer(dna: RepositoryDna): (id: string | null | undefined) => string {
  const names = new Map(dna.languages.languages.map((language) => [language.id, language.name]));
  return (id) => (id ? (names.get(id) ?? id) : "–");
}

/** Returns a function that turns a contributor identifier into the recorded name. */
export function contributorNamer(dna: RepositoryDna): (id: string) => string {
  const names = new Map(
    dna.git.contributors.map((contributor) => [contributor.id, contributor.name]),
  );
  return (id) => names.get(id) ?? id;
}

/** "1 file", "3 files". */
export function count(value: number, one: string, many: string): string {
  return `${new Intl.NumberFormat("en-US").format(value)} ${value === 1 ? one : many}`;
}

/**
 * A measurement's unit, such as "files" or "effective languages", in the singular when the
 * value reads as 1 with two decimals. Units that are not counts ("ratio", "share of code
 * files") are returned unchanged.
 */
export function unitFor(value: number, unit: string): string {
  if (Math.round(value * 100) !== 100 || unit.startsWith("share of")) {
    return unit;
  }
  // A plural noun, optionally with a qualifier before it ("dependent modules") or a
  // participle after it ("checks met").
  let before = "";
  let noun = unit;
  let after = "";
  if (unit.endsWith(" met")) {
    noun = unit.slice(0, -" met".length);
    after = " met";
  } else if (unit.includes(" ")) {
    const space = unit.lastIndexOf(" ");
    before = unit.slice(0, space + 1);
    noun = unit.slice(space + 1);
  }
  let singular: string;
  if (noun.endsWith("ies")) {
    singular = `${noun.slice(0, -3)}y`;
  } else if (["ches", "shes", "sses", "xes"].some((ending) => noun.endsWith(ending))) {
    singular = noun.slice(0, -2);
  } else if (noun.endsWith("s") && !noun.endsWith("ss")) {
    singular = noun.slice(0, -1);
  } else {
    return unit;
  }
  return `${before}${singular}${after}`;
}
