import { SCHEMA_MAJOR, SCHEMA_MINOR } from "@repodna/schema";
import { PageHeader, Panel } from "../components/common";
import { ShortcutTable } from "../components/ShortcutHelp";
import { href } from "../lib/router";
import type { ThemePreference } from "../lib/theme";
import { useApp } from "../state";

const THEMES: [ThemePreference, string][] = [
  ["system", "Follow the system"],
  ["light", "Light"],
  ["dark", "Dark"],
];

export function Settings() {
  const { themePreference, setThemePreference, backend, session } = useApp();
  return (
    <>
      <PageHeader title="Settings" />
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
              This interface stores only your theme choice and, when it is served by{" "}
              <code>repodna serve</code>, the sign-in token for that server.
            </li>
            <li>Files you open here are read on this machine and never uploaded.</li>
          </ul>
          <p className="muted">
            Read the <a href={href("/privacy")}>Privacy Policy</a> and the{" "}
            <a href={href("/terms")}>Terms of Use</a>, or learn more{" "}
            <a href={href("/about")}>about RepoDNA and how to support it</a>.
          </p>
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
    </>
  );
}
