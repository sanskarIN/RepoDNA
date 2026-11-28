import { useState } from "react";
import type { RemoteRevision, SourceReference, SourceReferences } from "../lib/backend";

interface Props {
  input: string;
  discover: (input: string) => Promise<SourceReferences>;
  disabled: boolean;
  onSelect: (revision: RemoteRevision) => void;
}

type Status =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "ready"; listing: SourceReferences };

const DISPLAY_LIMIT = 150;

/** Mounted for a specific input. A changed input remounts the picker, so an
 * earlier request cannot populate the new repository's choices. */
export function RemoteRefPicker({ input, discover, disabled, onSelect }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "idle" });
  const [kind, setKind] = useState<SourceReference["kind"]>("branch");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState("");

  const load = async () => {
    setStatus({ kind: "loading" });
    setSelected("");
    try {
      setStatus({ kind: "ready", listing: await discover(input) });
    } catch (error) {
      setStatus({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const listing = status.kind === "ready" ? status.listing : null;
  const matching = (listing?.references ?? []).filter(
    (reference) =>
      reference.kind === kind && reference.name.toLowerCase().includes(query.trim().toLowerCase()),
  );
  const visible = matching.slice(0, DISPLAY_LIMIT);
  const choice = visible.find((reference) => reference.name === selected);

  return (
    <div className="panel" aria-label="Remote branch and tag discovery">
      <div className="actions">
        <button
          type="button"
          disabled={disabled || status.kind === "loading"}
          onClick={() => void load()}
        >
          {status.kind === "loading" ? "Checking remote…" : "Find branches and tags"}
        </button>
      </div>
      <p className="muted">
        Contacts the repository using native Git only when requested. The selected reference is
        verified again during analysis.
      </p>
      {status.kind === "loading" ? <p role="status">Reading remote references…</p> : null}
      {status.kind === "error" ? (
        <p role="alert">Could not list remote references: {status.message}. Enter a name manually.</p>
      ) : null}
      {listing ? (
        <>
          <div className="filters">
            <label htmlFor="remote-reference-kind">Reference type</label>
            <select
              id="remote-reference-kind"
              value={kind}
              disabled={disabled}
              onChange={(event) => {
                setKind(event.target.value === "tag" ? "tag" : "branch");
                setSelected("");
              }}
            >
              <option value="branch">Branches</option>
              <option value="tag">Tags</option>
            </select>
            <label htmlFor="remote-reference-filter">Filter names</label>
            <input
              id="remote-reference-filter"
              value={query}
              disabled={disabled}
              onChange={(event) => {
                setQuery(event.target.value);
                setSelected("");
              }}
            />
            <label htmlFor="remote-reference-choice">Choose a reference</label>
            <select
              id="remote-reference-choice"
              value={selected}
              disabled={disabled || visible.length === 0}
              onChange={(event) => setSelected(event.target.value)}
            >
              <option value="">Select a name…</option>
              {visible.map((reference) => (
                <option key={reference.name} value={reference.name}>
                  {reference.name}
                </option>
              ))}
            </select>
            <button
              type="button"
              disabled={disabled || !choice}
              onClick={() => {
                if (choice) onSelect({ kind: choice.kind, value: choice.name });
              }}
            >
              Use selected reference
            </button>
          </div>
          <p role="status">
            {matching.length} matching {kind === "branch" ? "branches" : "tags"}.
            {matching.length > DISPLAY_LIMIT ? " Filter names to see more matches." : ""}
            {listing.truncated ? " Listing limited; other names can be entered manually." : ""}
          </p>
          {choice ? (
            <p className="muted">
              Advertised object: <code>{choice.object}</code>
            </p>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
