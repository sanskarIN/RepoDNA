// The project's links, shared by the About page, the legal pages, and the sidebar.

export const REPOSITORY = "https://github.com/sanskarIN/RepoDNA";
export const WEB_VERSION = "https://sanskarin.github.io/RepoDNA/";

export interface ProjectLink {
  label: string;
  url: string;
  description: string;
}

/** Ways to support RepoDNA's development. */
export const SUPPORT: ProjectLink[] = [
  {
    label: "Buy Me a Coffee",
    url: "https://www.buymeacoffee.com/sanskarIN",
    description: "Support RepoDNA's development on Buy Me a Coffee.",
  },
  {
    label: "Razorpay",
    url: "https://www.razorpay.me/@sanskarIN",
    description: "Support RepoDNA's development with Razorpay.",
  },
];

/** The project and its creator. */
export const PROJECT: ProjectLink[] = [
  {
    label: "Source code",
    url: REPOSITORY,
    description: "The RepoDNA repository on GitHub: code, issues, and discussions.",
  },
  {
    label: "Downloads",
    url: `${REPOSITORY}/releases/latest`,
    description: "The command line and the desktop app for Linux, macOS, and Windows.",
  },
  {
    label: "Web version",
    url: WEB_VERSION,
    description: "RepoDNA in the browser: open an analysis or try the demo.",
  },
  {
    label: "Documentation",
    url: `${REPOSITORY}/tree/main/docs`,
    description: "Guides for every command, the web interface, and the desktop app.",
  },
  {
    label: "Report a problem",
    url: `${REPOSITORY}/issues/new/choose`,
    description: "Bug reports, false positives, and feature requests.",
  },
  {
    label: "Sanskar on GitHub",
    url: "https://github.com/sanskarIN",
    description: "The creator of RepoDNA.",
  },
  {
    label: "Programming learning",
    url: "https://sanskarIN.gumroad.com",
    description: "Programming learning by Sanskar on Gumroad.",
  },
];

/** Documents in the repository that have a page in this interface. */
export const IN_APP_DOCUMENTS: Record<string, string> = {
  [`${REPOSITORY}/blob/main/PRIVACY.md`]: "/privacy",
  [`${REPOSITORY}/blob/main/TERMS.md`]: "/terms",
  [`${REPOSITORY}/blob/main/LICENSE`]: "/licenses",
  [`${REPOSITORY}/blob/main/THIRD-PARTY-NOTICES.txt`]: "/licenses",
};

/** A shorter form of a URL for display. */
export function displayUrl(url: string): string {
  return url.replace(/^https:\/\/(www\.)?/, "").replace(/\/$/, "");
}
