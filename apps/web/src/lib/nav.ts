// The views of the interface, shared by the sidebar, the command palette, and routing.

export interface NavItem {
  path: string;
  label: string;
  /** Needs an open analysis. */
  needsData: boolean;
  group: "Explore" | "Health" | "Share" | "App";
}

export const NAV: NavItem[] = [
  { path: "/overview", label: "Overview", needsData: true, group: "Explore" },
  { path: "/architecture", label: "Architecture", needsData: true, group: "Explore" },
  { path: "/dependencies", label: "Dependencies", needsData: true, group: "Explore" },
  { path: "/history", label: "History", needsData: true, group: "Explore" },
  { path: "/time-machine", label: "Time Machine", needsData: true, group: "Explore" },
  { path: "/files", label: "Files", needsData: true, group: "Explore" },
  { path: "/hotspots", label: "Hotspots", needsData: true, group: "Health" },
  { path: "/quality", label: "Code quality", needsData: true, group: "Health" },
  { path: "/project", label: "Tests, build & docs", needsData: true, group: "Health" },
  { path: "/security", label: "Security signals", needsData: true, group: "Health" },
  { path: "/findings", label: "Findings", needsData: true, group: "Health" },
  { path: "/reports", label: "Reports & export", needsData: true, group: "Share" },
  { path: "/compare", label: "Compare", needsData: true, group: "Share" },
  { path: "/", label: "Open or analyze", needsData: false, group: "App" },
  { path: "/settings", label: "Settings & about", needsData: false, group: "App" },
];
