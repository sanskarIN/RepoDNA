// The platforms the repodna command line is published for on npm: npm's names for the
// operating system and processor, the Rust target the binary is built for, and the
// binary's file name. The packaging script and the launcher both read this list.

export const SCOPE = "@sanskarin";

export const PLATFORMS = [
  { os: "linux", cpu: "x64", target: "x86_64-unknown-linux-musl", binary: "repodna" },
  { os: "linux", cpu: "arm64", target: "aarch64-unknown-linux-musl", binary: "repodna" },
  { os: "darwin", cpu: "x64", target: "x86_64-apple-darwin", binary: "repodna" },
  { os: "darwin", cpu: "arm64", target: "aarch64-apple-darwin", binary: "repodna" },
  { os: "win32", cpu: "x64", target: "x86_64-pc-windows-msvc", binary: "repodna.exe" },
];

/** The package holding the binary for a platform, such as `@sanskarin/repodna-linux-x64`. */
export function platformPackage(platform) {
  return `${SCOPE}/repodna-${platform.os}-${platform.cpu}`;
}

/** The entry for `os` and `cpu` as Node names them (`process.platform`, `process.arch`). */
export function findPlatform(os, cpu) {
  return PLATFORMS.find((platform) => platform.os === os && platform.cpu === cpu);
}
