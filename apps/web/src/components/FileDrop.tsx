import { useEffect, useRef, useState } from "react";
import { carriesFiles, useOpenFile } from "../lib/openFile";
import { navigate } from "../lib/router";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Opens an analysis file dropped anywhere on the page. A drop that a part of the page
 * handles itself, such as the drop zone of the start page, is left to it. Without this,
 * a file dropped beside a drop zone would make the browser leave the page to show it.
 */
export function FileDrop() {
  const openFile = useOpenFile();
  const [over, setOver] = useState(false);
  // Over a drop zone of the page, which shows what a drop there does instead.
  const [overZone, setOverZone] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The latest `openFile`, for the listeners added once: adding them again would lose
  // count of a drag in progress.
  const openRef = useRef(openFile);
  useEffect(() => {
    openRef.current = openFile;
  });

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
      const file = event.dataTransfer?.files[0];
      if (!file) {
        return;
      }
      setError(null);
      setBusy(file.name);
      openRef
        .current(file)
        .then(
          () => navigate("/overview"),
          (reason: unknown) => setError(`Could not open ${file.name}: ${message(reason)}`),
        )
        .finally(() => setBusy(null));
    };
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragleave", leave);
    window.addEventListener("dragover", dragOver);
    window.addEventListener("drop", drop);
    return () => {
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("dragover", dragOver);
      window.removeEventListener("drop", drop);
    };
  }, []);

  return (
    <>
      {over && !overZone ? (
        <div className="drop-overlay" aria-hidden="true">
          <div>
            <strong>Drop the file to open it</strong>
            <span>A .repodna or repodna.json file, read on this machine and never uploaded.</span>
          </div>
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
