import { SCHEMA_MAJOR, SCHEMA_MINOR } from "@repodna/schema";
import { ExternalLink, PageHeader, Panel } from "../components/common";
import { ShortcutTable } from "../components/ShortcutHelp";
import type { ThemePreference } from "../lib/theme";
import { useApp } from "../state";

const THEMES: [ThemePreference, string][] = [
  ["system", "Follow the system"],
  ["light", "Light"],
  ["dark", "Dark"],
];

/** The project and its creator's links, as listed on the About screen. */
export const LINKS: [string, string, string][] = [
  ["Source code", "https://github.com/sanskarIN/RepoDNA", "Issues, releases, and discussions"],
  ["GitHub", "https://github.com/sanskarIN", "The creator's profile"],
  ["Programming learning", "https://sanskarIN.gumroad.com", "Programming learning on Gumroad"],
  ["Buy Me A Coffee", "https://www.buymeacoffee.com/sanskarIN", "Support RepoDNA's development"],
  ["Razorpay", "https://www.razorpay.me/@sanskarIN", "Support RepoDNA's development"],
];

export function Settings() {
  const { themePreference, setThemePreference, backend, session } = useApp();
  return (
    <>
      <PageHeader title="Settings & about" />
      <div className="grid two">
        <Panel title="Appearance">
          <fieldset className="choices">
            <legend>Theme</legend>
            {THEMES.map(([value, label]) => (
              <label key={value}>
                <input
                  type="radio"
                  name="theme"
                  value={value}
                  checked={themePreference === value}
                  onChange={() => setThemePreference(value)}
                />{" "}
                {label}
              </label>
            ))}
          </fieldset>
          <p className="muted">Charts follow the theme; every chart also has a table view.</p>
        </Panel>
        <Panel title="Privacy">
          <ul className="evidence">
            <li>RepoDNA analyzes repositories on this machine and sends no telemetry.</li>
            <li>
              This page stores only your theme choice in this browser, and the sign-in token for
              this tab.
            </li>
            <li>Files you open here are read in the browser and never uploaded.</li>
            <li>
              AI explanations are off unless you configure a provider; remote providers need your
              explicit consent.
            </li>
          </ul>
        </Panel>
      </div>
      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel title="Keyboard shortcuts">
          <ShortcutTable />
        </Panel>
        <Panel title="Connection">
          <dl className="facts">
            <dt>Running as</dt>
            <dd>
              {backend?.kind === "desktop"
                ? "Desktop app"
                : backend
                  ? "Web interface of repodna serve"
                  : "Static page (open files and the demo only)"}
            </dd>
            {session ? (
              <>
                <dt>RepoDNA</dt>
                <dd>v{session.version}</dd>
                <dt>New analyses</dt>
                <dd>{session.allowScans ? "Allowed" : "Turned off (--no-scan)"}</dd>
              </>
            ) : null}
            <dt>Web interface</dt>
            <dd>RepoDNA v{__REPODNA_VERSION__}</dd>
            <dt>Reads analyses</dt>
            <dd>
              Analysis schema v{SCHEMA_MAJOR} (up to {SCHEMA_MAJOR}.{SCHEMA_MINOR})
            </dd>
          </dl>
        </Panel>
      </div>
      <Panel title="About" id="about">
        <div className="about">
          <img src="./favicon.svg" alt="" width={48} height={48} />
          <div>
            <h3 style={{ margin: 0 }}>RepoDNA</h3>
            <p style={{ marginTop: 4 }}>
              Open-source repository intelligence and code archaeology.
            </p>
          </div>
        </div>
        <ul className="list">
          {LINKS.map(([label, url, description]) => (
            <li key={url}>
              <div>
                <strong>{label}</strong>
                <div className="muted">{description}</div>
              </div>
              <ExternalLink href={url}>{url.replace(/^https:\/\/(www\.)?/, "")}</ExternalLink>
            </li>
          ))}
        </ul>
        <p className="muted">Licensed under the Apache License 2.0. Made by the Sanskar.</p>
      </Panel>
    </>
  );
}
