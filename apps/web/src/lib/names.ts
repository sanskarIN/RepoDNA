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
