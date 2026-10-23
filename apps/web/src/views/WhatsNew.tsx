import changelog from "../../../../CHANGELOG.md?raw";
import { PageHeader, Panel } from "../components/common";
import { releaseNotes } from "../lib/whats-new";

/** Release notes are bundled offline so desktop onboarding needs no network. */
export function ReleaseHighlights({ version = __REPODNA_VERSION__ }: { version?: string }) {
  const notes = releaseNotes(changelog, version);
  return (
    <div className="release-highlights">
      <p className="muted">
        {notes.date ? `Released ${notes.date}` : "Changes included with this installation."}
      </p>
      {notes.highlights.length > 0 ? (
        <ul className="evidence">
          {notes.highlights.map((text, index) => (
            <li key={index}>{text}</li>
          ))}
        </ul>
      ) : (
        <p>See the project changelog for this version&apos;s changes.</p>
      )}
      <p className="muted">
        Full details:{" "}
        <a href="https://github.com/sanskarIN/RepoDNA/blob/main/CHANGELOG.md">
          RepoDNA changelog
        </a>
      </p>
    </div>
  );
}

export function WhatsNew() {
  return (
    <>
      <PageHeader title="What's New">
        Browse highlights for your installed RepoDNA v{__REPODNA_VERSION__}.
      </PageHeader>
      <Panel title={`RepoDNA v${__REPODNA_VERSION__}`}>
        <ReleaseHighlights />
      </Panel>
    </>
  );
}
