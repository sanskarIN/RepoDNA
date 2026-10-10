import { useEffect, useRef, useState } from "react";
import { droppedFolder, isArchive } from "../engine/input";
import { analyze, analyzeDroppedFolder, profile } from "../engine/session";
import { carriesFiles, useOpenFile } from "../lib/openFile";
import { navigate } from "../lib/router";
import { useApp } from "../state";
import { useEngine } from "./BrowserAnalysis";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** What an installed web version is started with when it opens files (Chromium browsers). */
interface LaunchQueue {
  setConsumer(consumer: (params: { files?: readonly FileSystemHandle[] }) => void): void;
}

/**
 * Opens an analysis file dropped anywhere on the page, and analyzes a dropped folder or
 * archive when the page can analyze in the browser (the web version). A drop that a part of
 * the page handles itself, such as the drop zone of the start page, is left to it. Without
 * this, a file dropped beside a drop zone would make the browser leave the page to show it.
 *
 * Also opens the `.repodna` file that an installed web version was started with, from the
 * computer's file manager.
 */
export function FileDrop() {
  const { ready, backend } = useApp();
  const openFile = useOpenFile();
  const engine = useEngine();
  const [over, setOver] = useState(false);
  // Over a drop zone of the page, which shows what a drop there does instead.
  const [overZone, setOverZone] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Opens a file from outside the page, with a notice while it is read and one if it cannot
  // be opened. Kept up to date for the listeners, which are added once: adding them again
  // would lose count of a drag in progress.
  const openOutside = useRef<(file: File) => void>(() => undefined);
  // Analyzes a dropped folder or archive, or explains why it cannot; false for other files.
  const analyzeOutside = useRef<(transfer: DataTransfer | null) => boolean>(() => false);
  useEffect(() => {
    analyzeOutside.current = (transfer) => {
      const folder = droppedFolder(transfer);
      const file = transfer?.files[0];
      if (folder) {
        setError(null);
        if (engine) {
          void analyzeDroppedFolder(folder, profile());
          navigate("/");
        } else {
          setError(
            `${folder.name} is a folder. Drop a .repodna or repodna.json file to open it, or analyze the folder with ${backend ? "Analyze a repository on the start page" : "the command line"}.`,
          );
        }
        return true;
      }
      if (file && engine && isArchive(file.name)) {
        setError(null);
        analyze({ kind: "archive", file }, profile());
        navigate("/");
        return true;
      }
      return false;
    };
    openOutside.current = (file) => {
      setError(null);
      setBusy(file.name);
      openFile(file)
        .then(
          () => navigate("/overview"),
          (reason: unknown) => setError(`Could not open ${file.name}: ${message(reason)}`),
        )
        .finally(() => setBusy(null));
    };
  });

  useEffect(() => {
    const queue = (window as { launchQueue?: LaunchQueue }).launchQueue;
    // Once an analysis kept from before a reload is open, so that it does not replace this one.
    if (!ready || !queue) {
      return;
    }
    queue.setConsumer(({ files }) => {
      const handle = files?.[0];
      if (handle?.kind === "file") {
        void (handle as FileSystemFileHandle).getFile().then((file) => openOutside.current(file));
      }
    });
  }, [ready]);

  useEffect(() => {
    // Entering a child fires before leaving its parent, so count to know when the drag
    // has left the page.
    let depth = 0;
    const enter = (event: DragEvent) => {
      if (carriesFiles(event)) {
        depth += 1;
        setOver(true);
      }
    };
    const leave = (event: DragEvent) => {
      if (carriesFiles(event)) {
        depth = Math.max(0, depth - 1);
        if (depth === 0) {
          setOver(false);
        }
      }
    };
    const dragOver = (event: DragEvent) => {
      if (carriesFiles(event)) {
        event.preventDefault();
        setOverZone(event.target instanceof Element && event.target.closest(".dropzone") !== null);
        if (event.dataTransfer) {
          event.dataTransfer.dropEffect = "copy";
        }
      }
    };
    const drop = (event: DragEvent) => {
      depth = 0;
      setOver(false);
      if (!carriesFiles(event) || event.defaultPrevented) {
        return;
      }
      event.preventDefault();
      if (analyzeOutside.current(event.dataTransfer)) {
        return;
      }
      const file = event.dataTransfer?.files[0];
      if (!file) {
        return;
      }
      openOutside.current(file);
    };
    // The mouse does not move while something is dragged, so a move means no drag is over the
    // page, even when the browser did not report the drag leaving it.
    const moved = () => {
      if (depth > 0) {
        depth = 0;
        setOver(false);
      }
    };
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragleave", leave);
    window.addEventListener("dragover", dragOver);
    window.addEventListener("drop", drop);
    window.addEventListener("mousemove", moved, { passive: true });
    return () => {
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("dragover", dragOver);
      window.removeEventListener("drop", drop);
      window.removeEventListener("mousemove", moved);
    };
  }, []);

  return (
    <>
      {over && !overZone ? (
        <div className="drop-overlay" aria-hidden="true">
          {engine ? (
            <div>
              <strong>Drop to open or analyze it</strong>
              <span>
                An analysis file opens; a folder or a .zip or .tar.gz archive is analyzed in this
                browser. Nothing is uploaded.
              </span>
            </div>
          ) : (
            <div>
              <strong>Drop the file to open it</strong>
              <span>A .repodna or repodna.json file, read on this machine and never uploaded.</span>
            </div>
          )}
        </div>
      ) : null}
      <div className="notices">
        <div role="status">{busy ? <p className="notice">Opening {busy}…</p> : null}</div>
        {error ? (
          <div className="notice error" role="alert">
            <p>{error}</p>
            <button type="button" className="ghost" onClick={() => setError(null)}>
              Dismiss
            </button>
          </div>
        ) : null}
      </div>
    </>
  );
}
