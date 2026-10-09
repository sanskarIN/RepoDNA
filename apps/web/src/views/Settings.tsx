import { SCHEMA_MAJOR, SCHEMA_MINOR } from "@repodna/schema";
import { PageHeader, Panel } from "../components/common";
import { ShortcutTable } from "../components/ShortcutHelp";
import { RECENT_LIMIT } from "../lib/recent";
import { href } from "../lib/router";
import type { ThemePreference } from "../lib/theme";
import { useApp } from "../state";

const THEMES: [ThemePreference, string][] = [
  ["system", "Follow the system"],
  ["light", "Light"],
  ["dark", "Dark"],
];

export function Settings() {
  const {
    themePreference,
    setThemePreference,
    backend,
    session,
    remember,
    setRemember,
    recent,
    forgetAll,
  } = useApp();
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
              This interface stores your theme choice, the analysis files you opened recently
              (unless you turn that off below), and, when it is served by <code>repodna serve</code>
              , the sign-in token for that server. All of it stays in this browser.
            </li>
            <li>Files you open here are read on this machine and never uploaded.</li>
            <li>
              AI explanations are off unless you configure a provider; remote providers need your
              explicit consent.
            </li>
          </ul>
          <fieldset className="choices">
            <legend>Recent analyses</legend>
            <label>
              <input
                type="checkbox"
                checked={remember}
                onChange={(event) => setRemember(event.target.checked)}
              />{" "}
              Keep the last {RECENT_LIMIT} analysis files I open in this browser, so that they open
              again from the start page and after a reload
            </label>
          </fieldset>
          <p>
            <button type="button" disabled={recent.length === 0} onClick={forgetAll}>
              Forget recent analyses
            </button>{" "}
            <span className="muted" role="status">
              {recent.length === 0 ? "None kept." : `${recent.length} kept in this browser.`}
            </span>
          </p>
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
