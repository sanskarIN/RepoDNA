// Which files of a folder are given to the analysis.

/** Version-control directories, which the analysis never reads. */
export const SKIPPED_DIRECTORIES: ReadonlySet<string> = new Set([
  ".git",
  ".hg",
  ".svn",
  ".jj",
  ".bzr",
  "_darcs",
  ".fossil",
]);

/** Whether a path inside a folder is worth giving to the analysis. */
export function keep(path: string): boolean {
  return !path.split("/").some((part) => SKIPPED_DIRECTORIES.has(part));
}
