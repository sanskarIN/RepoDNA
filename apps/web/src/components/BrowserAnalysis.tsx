import { useEffect, useRef, useState } from "react";
import { parseArtifact } from "@repodna/schema";
import { bytes, thousands } from "@repodna/visualization";
import { engineInfo, type EngineInfo } from "../engine/client";
import { folderFromList, isArchive } from "../engine/input";
import {
  analyze,
  cancel,
  dismiss,
  fail,
  profile,
  setProfile,
  useBrowserAnalysis,
} from "../engine/session";
import { REPOSITORY } from "../lib/links";
import { navigate } from "../lib/router";
import { useApp } from "../state";
import { ErrorBox, ExternalLink, Note } from "./common";

const PROFILES = [
  ["standard", "Standard: dependencies, architecture, quality, and security"],
  ["quick", "Quick: structure, languages, and project conventions"],
  ["deep", "Deep: adds duplication and file similarity"],
] as const;

/** Stage names as the analysis reports them, in words. */
const STAGES: Record<string, string> = {
  discovery: "Reading and measuring the files",
  dependencies: "Dependencies",
  architecture: "Architecture",
  quality: "Code quality",
  project: "Tests, builds, and documentation",
  security: "Security",
  git: "Git history",
  evolution: "Evolution",
  insights: "Insights",
  plugins: "Plugins",
};

function stageName(stage: string): string {
  return STAGES[stage] ?? stage;
}

/** The analysis program this page comes with: undefined while asked, null when there is none. */
export function useEngine(): EngineInfo | null | undefined {
  const [info, setInfo] = useState<EngineInfo | null | undefined>(undefined);
  useEffect(() => {
    let current = true;
    void engineInfo().then((value) => current && setInfo(value));
    return () => {
      current = false;
    };
  }, []);
  return info;
}

/** Seconds since `start`, updated every second. */
function useElapsed(start: number | null): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (start === null) {
      return;
    }
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [start]);
  return start === null ? 0 : Math.max(0, Math.round((now - start) / 1000));
}

function Progress() {
  const analysis = useBrowserAnalysis();
  const elapsed = useElapsed(analysis.phase === "running" ? analysis.startedAt : null);
  switch (analysis.phase) {
    case "idle":
    case "finished":
      return null;
    case "listing":
      return (
        <div className="analysis-progress" aria-live="polite">
          <p>
            Listing the files of <strong>{analysis.name}</strong>: {thousands(analysis.files)} so
            far…
          </p>
          <button type="button" onClick={cancel}>
            Stop
          </button>
        </div>
      );
    case "running":
      return (
        <div className="analysis-progress" aria-live="polite">
          <p>
            Analyzing <strong>{analysis.name}</strong>
            {analysis.files !== null ? ` (${thousands(analysis.files)} files, ` : " ("}
            {bytes(analysis.bytes)}) · {elapsed} s
          </p>
          <ul className="evidence">
            {analysis.stages.length === 0 && analysis.stage === null ? (
              <li>Loading RepoDNA's analysis program…</li>
            ) : null}
            {analysis.stages.map(([stage, outcome]) => (
              <li key={stage}>
                {outcome === "completed" ? "✓" : "–"} {stageName(stage)}
                {outcome !== "completed" ? ` (${outcome})` : ""}
              </li>
            ))}
            {analysis.stage ? (
              <li>
                … {stageName(analysis.stage)}
                {analysis.total > 0
                  ? `: ${thousands(analysis.done)} of ${thousands(analysis.total)} files`
                  : ""}
              </li>
            ) : null}
          </ul>
          {analysis.stage && analysis.total > 0 ? (
            <progress
              value={analysis.done}
              max={analysis.total}
              aria-label={`${stageName(analysis.stage)}: files done`}
            />
          ) : null}
          <button type="button" onClick={cancel}>
            Stop
          </button>
        </div>
      );
    case "failed":
      return (
        <ErrorBox>
          {analysis.name} could not be analyzed: {analysis.message}{" "}
          <button type="button" className="ghost" onClick={dismiss}>
            Dismiss
          </button>
        </ErrorBox>
      );
    case "cancelled":
      return (
        <Note>
          The analysis of {analysis.name} was stopped.{" "}
          <button type="button" className="ghost" onClick={dismiss}>
            Dismiss
          </button>
        </Note>
      );
  }
}

/**
 * Analyzing a folder or an archive in this browser, with the WebAssembly build of RepoDNA
 * that the web version comes with. Shown on the start page when there is no server.
 */
export function BrowserScan({ engine }: { engine: EngineInfo }) {
  const analysis = useBrowserAnalysis();
  const [chosen, setChosen] = useState(profile());
  const [error, setError] = useState<string | null>(null);
  const folderInput = useRef<HTMLInputElement>(null);
  const archiveInput = useRef<HTMLInputElement>(null);
  const running = analysis.phase === "listing" || analysis.phase === "running";

  useEffect(() => {
    // Lets the browser offer to choose a folder; React does not know the attribute.
    folderInput.current?.setAttribute("webkitdirectory", "");
  }, []);

  return (
    <section className="panel" aria-labelledby="browser-scan-title">
      <div className="panel-header">
        <div>
          <h2 id="browser-scan-title">Analyze a repository in this browser</h2>
          <p>
            Choose a folder, or a .zip or .tar.gz archive. RepoDNA analyzes it right here, with its
            analysis built for the web; nothing is uploaded.
          </p>
        </div>
      </div>
      <div className="filters">
        <button
          type="button"
          className="primary"
          disabled={running}
          onClick={() => folderInput.current?.click()}
        >
          Choose a folder…
        </button>
        <button type="button" disabled={running} onClick={() => archiveInput.current?.click()}>
          Choose an archive…
        </button>
        <label className="visually-hidden" htmlFor="browser-profile">
          Profile
        </label>
        <select
          id="browser-profile"
          value={chosen}
          disabled={running}
          onChange={(event) => {
            setChosen(event.target.value);
            setProfile(event.target.value);
          }}
        >
          {PROFILES.map(([id, text]) => (
            <option key={id} value={id}>
              {text}
            </option>
          ))}
        </select>
      </div>
      <input
        ref={folderInput}
        type="file"
        multiple
        className="visually-hidden"
        aria-label="Choose a folder to analyze"
        tabIndex={-1}
        onChange={(event) => {
          const input = folderFromList(event.target.files ?? []);
          event.target.value = "";
          setError(input ? null : "The folder has no files to analyze.");
          if (input) {
            analyze(input, chosen);
          }
        }}
      />
      <input
        ref={archiveInput}
        type="file"
        accept=".zip,.tar,.tar.gz,.tgz,application/zip,application/x-tar,application/gzip"
        className="visually-hidden"
        aria-label="Choose an archive to analyze"
        tabIndex={-1}
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = "";
          if (!file) {
            return;
          }
          setError(isArchive(file.name) ? null : `${file.name} is not a .zip or .tar.gz archive.`);
          if (isArchive(file.name)) {
            analyze({ kind: "archive", file }, chosen);
          }
        }}
      />
      {error ? <ErrorBox>{error}</ErrorBox> : null}
      <Progress />
      <p className="muted">
        Browsers cannot run Git, so the analysis has no history: Activity, History, and the Time
        Machine stay empty. For them,{" "}
        <ExternalLink href={`${REPOSITORY}/releases/latest`}>download RepoDNA</ExternalLink>, the
        command line or the desktop app. The first analysis downloads RepoDNA's analysis program (
        {bytes(engine.bytes)}), which the browser then keeps.
      </p>
    </section>
  );
}

/** Opens an analysis made in this browser once it is finished. */
export function BrowserAnalysisOpener() {
  const analysis = useBrowserAnalysis();
  const { open } = useApp();
  useEffect(() => {
    if (analysis.phase !== "finished") {
      return;
    }
    try {
      const loaded = parseArtifact(analysis.text);
      open(
        {
          dna: loaded.artifact,
          origin: { kind: "file", name: analysis.name },
          warnings: loaded.warnings,
        },
        { text: analysis.text, size: analysis.size },
      );
      dismiss();
      navigate("/overview");
    } catch (error) {
      fail(analysis.name, error instanceof Error ? error.message : String(error));
    }
  }, [analysis, open]);
  return null;
}
