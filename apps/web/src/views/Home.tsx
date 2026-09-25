import { useEffect, useRef, useState, type DragEvent } from "react";
import { thousands } from "@repodna/visualization";
import { ErrorBox, Note } from "../components/common";
import type { JobState, RepositorySummary } from "../lib/backend";
import { demoIndex, loadDemo, loadFile, type DemoEntry } from "../lib/demo";
import { navigate } from "../lib/router";
import { useApp } from "../state";

const PROFILES = [
  ["standard", "Standard: everything except the slowest history reconstruction"],
  ["quick", "Quick: structure, languages, and dependencies only"],
  ["deep", "Deep: all analyzers, including duplication and historical architecture"],
] as const;

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function ScanForm({ onDone }: { onDone: (repositoryId: string, scanId?: string) => void }) {
  const { backend, session } = useApp();
  const [input, setInput] = useState("");
  const [profile, setProfile] = useState("standard");
  const [job, setJob] = useState<JobState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (timer.current !== null) {
        window.clearTimeout(timer.current);
      }
    },
    [],
  );

  if (!backend || !session?.allowScans) {
    return null;
  }

  const poll = (id: string) => {
    timer.current = window.setTimeout(async () => {
      try {
        const next = await backend.job(id);
        setJob(next);
        if (next.status === "running") {
          poll(id);
        } else if (next.status === "completed" && next.repositoryId) {
          onDone(next.repositoryId, next.scanId ?? undefined);
        } else if (next.status === "failed") {
          setError(next.error ?? "The analysis failed.");
        }
      } catch (reason) {
        setError(message(reason));
      }
    }, 400);
  };

  const start = async () => {
    setError(null);
    try {
      const started = await backend.startScan(input.trim(), profile);
      setJob(started);
      poll(started.id);
    } catch (reason) {
      setError(message(reason));
    }
  };

  const running = job?.status === "running";
  return (
    <section className="panel" aria-labelledby="scan-title">
      <div className="panel-header">
        <div>
          <h2 id="scan-title">Analyze a repository</h2>
          <p>A local directory, a .zip or .tar.gz archive, or a Git URL. Nothing is uploaded.</p>
        </div>
      </div>
      <form
        className="filters"
        onSubmit={(event) => {
          event.preventDefault();
          if (input.trim() && !running) {
            void start();
          }
        }}
      >
        <label className="visually-hidden" htmlFor="scan-input">
          Repository to analyze
        </label>
        <input
          id="scan-input"
          type="text"
          value={input}
          onChange={(event) => setInput(event.target.value)}
          placeholder="/path/to/repository or https://github.com/owner/repo"
          style={{ flex: "1 1 320px" }}
        />
        {backend.pickDirectory ? (
          <button
            type="button"
            onClick={async () => {
              const picked = await backend.pickDirectory?.();
              if (picked) {
                setInput(picked);
              }
            }}
          >
            Choose folder…
          </button>
        ) : null}
        <label className="visually-hidden" htmlFor="scan-profile">
          Profile
        </label>
        <select
          id="scan-profile"
          value={profile}
          onChange={(event) => setProfile(event.target.value)}
        >
          {PROFILES.map(([id, text]) => (
            <option key={id} value={id}>
              {text}
            </option>
          ))}
        </select>
        <button type="submit" className="primary" disabled={!input.trim() || running}>
          {running ? "Analyzing…" : "Analyze"}
        </button>
        {running && job ? (
          <button type="button" onClick={() => void backend.cancelJob(job.id)}>
            Stop
          </button>
        ) : null}
      </form>
      {job ? (
        <div aria-live="polite">
          <ul className="evidence">
            {job.stages.map(([stage, outcome]) => (
              <li key={stage}>
                {outcome === "completed" ? "✓" : "!"} {stage}
                {outcome !== "completed" ? ` (${outcome})` : ""}
              </li>
            ))}
            {job.stage ? (
              <li>
                … {job.stage}
                {job.filesTotal > 0
                  ? ` ${thousands(job.filesDone)} / ${thousands(job.filesTotal)} files`
                  : ""}
              </li>
            ) : null}
          </ul>
          {job.status === "cancelled" ? <Note>The analysis was stopped.</Note> : null}
        </div>
      ) : null}
      {error ? <ErrorBox>{error}</ErrorBox> : null}
    </section>
  );
}

function Stored() {
  const { backend, open } = useApp();
  const [repositories, setRepositories] = useState<RepositorySummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!backend) {
      return;
    }
    backend.repositories().then(setRepositories, (reason: unknown) => setError(message(reason)));
  }, [backend]);

  if (!backend) {
    return null;
  }
  const openRepository = async (repository: RepositorySummary) => {
    try {
      const dna = await backend.artifact(repository.id);
      open({
        dna,
        origin: { kind: "stored", repositoryId: repository.id, name: repository.name },
        warnings: [],
      });
      navigate("/overview");
    } catch (reason) {
      setError(message(reason));
    }
  };
  return (
    <section className="panel" aria-labelledby="stored-title">
      <div className="panel-header">
        <div>
          <h2 id="stored-title">Stored analyses</h2>
          <p>Analyses kept in local storage on this machine.</p>
        </div>
      </div>
      {error ? <ErrorBox>{error}</ErrorBox> : null}
      {repositories === null && !error ? <p className="muted">Loading…</p> : null}
      {repositories && repositories.length === 0 ? (
        <p className="muted">
          None yet. Analyze a repository above or run <code>repodna analyze &lt;path&gt;</code>.
        </p>
      ) : null}
      <ul className="list">
        {(repositories ?? []).map((repository) => (
          <li key={repository.id}>
            <div>
              <strong>{repository.name}</strong>
              <div className="muted path">{repository.location}</div>
              <div className="muted">
                {repository.scans} {repository.scans === 1 ? "analysis" : "analyses"} · latest{" "}
                {repository.lastScannedAt.slice(0, 10)}
              </div>
            </div>
            <button type="button" onClick={() => void openRepository(repository)}>
              Open
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function Home() {
  const { backend, open, signInNeeded, error: startupError } = useApp();
  const [demos, setDemos] = useState<DemoEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void demoIndex().then(setDemos);
  }, []);

  const openFile = async (file: File | undefined) => {
    if (!file) {
      return;
    }
    setError(null);
    try {
      const loaded = await loadFile(file);
      open({
        dna: loaded.artifact,
        origin: { kind: "file", name: file.name },
        warnings: loaded.warnings,
      });
      navigate("/overview");
    } catch (reason) {
      setError(message(reason));
    }
  };

  const openDemo = async (entry: DemoEntry) => {
    setError(null);
    try {
      const loaded = await loadDemo(entry);
      open({
        dna: loaded.artifact,
        origin: { kind: "demo", title: entry.title },
        warnings: loaded.warnings,
      });
      navigate("/overview");
    } catch (reason) {
      setError(message(reason));
    }
  };

  const onDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setOver(false);
    void openFile(event.dataTransfer.files[0]);
  };

  return (
    <>
      <div className="hero">
        <div>
          <h1>Understand any codebase from the evidence.</h1>
          <p className="lead">
            RepoDNA analyzes a repository's structure, architecture, history, dependencies, and
            health, and explains every conclusion with the files, commits, and measurements behind
            it. It runs on your machine and collects no telemetry.
          </p>
          <div className="actions">
            {demos[0] ? (
              <button
                type="button"
                className="primary"
                onClick={() => void openDemo(demos[0] as DemoEntry)}
              >
                Try the demo
              </button>
            ) : null}
            <button type="button" onClick={() => fileInput.current?.click()}>
              Open an analysis file…
            </button>
          </div>
        </div>
        <div
          className={over ? "dropzone over" : "dropzone"}
          onDragOver={(event) => {
            event.preventDefault();
            setOver(true);
          }}
          onDragLeave={() => setOver(false)}
          onDrop={onDrop}
        >
          <p>
            <strong>Drop a .repodna or repodna.json file here</strong>
          </p>
          <p className="muted">
            Create one with <code>repodna export</code> or{" "}
            <code>repodna analyze --format json</code>. The file is read in this browser and never
            uploaded.
          </p>
          <input
            ref={fileInput}
            type="file"
            accept=".repodna,.json,application/json"
            className="visually-hidden"
            aria-label="Open an analysis file"
            onChange={(event) => void openFile(event.target.files?.[0])}
          />
        </div>
      </div>
      {startupError ? <ErrorBox>{startupError}</ErrorBox> : null}
      {error ? <ErrorBox>{error}</ErrorBox> : null}
      {signInNeeded ? (
        <Note caution>
          A RepoDNA server is running, but this browser has not signed in. Open the link that{" "}
          <code>repodna serve</code> printed in your terminal.
        </Note>
      ) : null}
      <div className="grid two" style={{ marginTop: 20 }}>
        <div>
          {backend ? (
            <ScanForm
              onDone={async (repositoryId, scanId) => {
                try {
                  const dna = await backend.artifact(repositoryId, scanId);
                  open({
                    dna,
                    origin: { kind: "stored", repositoryId, scanId, name: dna.identity.name },
                    warnings: [],
                  });
                  navigate("/overview");
                } catch (reason) {
                  setError(message(reason));
                }
              }}
            />
          ) : (
            <section className="panel" aria-labelledby="cli-title">
              <div className="panel-header">
                <div>
                  <h2 id="cli-title">Analyze a repository</h2>
                  <p>This page is running without a RepoDNA server, so it can only open files.</p>
                </div>
              </div>
              <p>Install RepoDNA, then run one of:</p>
              <pre className="note mono">
                repodna serve{"\n"}repodna analyze path/to/repo --output repodna-report
              </pre>
            </section>
          )}
          <Stored />
        </div>
        <div>
          {demos.length > 0 ? (
            <section className="panel" aria-labelledby="demo-title">
              <div className="panel-header">
                <div>
                  <h2 id="demo-title">Demo analyses</h2>
                  <p>Real RepoDNA artifacts bundled with this page; they work offline.</p>
                </div>
              </div>
              <ul className="list">
                {demos.map((entry) => (
                  <li key={entry.file}>
                    <div>
                      <strong>{entry.title}</strong>
                      <div className="muted">{entry.description}</div>
                    </div>
                    <button type="button" onClick={() => void openDemo(entry)}>
                      Open
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
          <section className="panel" aria-labelledby="minute-title">
            <div className="panel-header">
              <div>
                <h2 id="minute-title">RepoDNA in a minute</h2>
              </div>
            </div>
            <ul className="evidence">
              <li>Every finding lists its evidence, the method used, and its limitations.</li>
              <li>
                Charts always have a table view; severity is shown with words, not only color.
              </li>
              <li>Commands found in a repository are shown as detected, never run.</li>
              <li>AI explanations are optional and off unless you configure a provider.</li>
            </ul>
          </section>
        </div>
      </div>
    </>
  );
}
