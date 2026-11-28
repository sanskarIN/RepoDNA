import { useEffect, useRef, useState } from "react";
import type { RemoteRevision } from "../lib/backend";
import {
  EMPTY_REFERENCES,
  RemoteReferenceController,
  referencePage,
  referenceQueryError,
  type DiscoverReferences,
} from "../lib/remoteReferenceController";

interface Props {
  input: string;
  discover: DiscoverReferences;
  disabled: boolean;
  onSelect: (revision: RemoteRevision) => void;
}

/** A request belongs only to the current input. A previous advertisement is
 * never mistaken for a complete listing after a narrower search. */
export function RemoteRefPicker({ input, discover, disabled, onSelect }: Props) {
  const [state, setState] = useState(EMPTY_REFERENCES);
  const controller = useRef<RemoteReferenceController | null>(null);
  const discoverRef = useRef(discover);

  useEffect(() => {
    discoverRef.current = discover;
  }, [discover]);

  useEffect(() => {
    setState(EMPTY_REFERENCES);
    const instance = new RemoteReferenceController(
      input,
      (value, query) => (query ? discoverRef.current(value, query) : discoverRef.current(value)),
      setState,
    );
    controller.current = instance;
    return () => {
      instance.dispose();
      if (controller.current === instance) controller.current = null;
    };
  }, [input]);

  const { listing, scope, loading, error, kind, query, selected } = state;
  const page = referencePage(state);
  const queryError = referenceQueryError(query);
  const locked = disabled || loading;
  return (
    <div className="panel" aria-label="Remote branch and tag discovery">
      <div className="actions">
        <button
          type="button"
          disabled={locked}
          onClick={() => void controller.current?.load()}
        >
          {loading ? "Checking remote…" : "Find branches and tags"}
        </button>
      </div>
      <p className="muted">
        Contacts the repository using native Git only when requested. The selected reference is
        verified again during analysis.
      </p>
      {loading ? <p role="status">Reading remote references…</p> : null}
      {error ? (
        <p role="alert">
          Could not list remote references: {error}. Enter a name manually.
          {listing ? " The previous listing is still shown." : ""}
        </p>
      ) : null}
      {listing ? (
        <>
          <div className="filters">
            <label htmlFor="remote-reference-kind">Reference type</label>
            <select
              id="remote-reference-kind"
              value={kind}
              disabled={locked}
              onChange={(event) =>
                controller.current?.setKind(event.target.value === "tag" ? "tag" : "branch")
              }
            >
              <option value="branch">Branches</option>
              <option value="tag">Tags</option>
            </select>
            <label htmlFor="remote-reference-filter">Filter names</label>
            <input
              id="remote-reference-filter"
              value={query}
              disabled={locked}
              maxLength={512}
              aria-invalid={queryError ? true : undefined}
              aria-describedby={queryError ? "remote-reference-query-error" : undefined}
              onChange={(event) => controller.current?.setQuery(event.target.value)}
            />
            <button
              type="button"
              disabled={locked || queryError !== null}
              onClick={() => void controller.current?.search()}
            >
              Search remote references
            </button>
            {queryError ? (
              <p id="remote-reference-query-error" role="alert">
                {queryError}
              </p>
            ) : null}
            <label htmlFor="remote-reference-choice">Choose a reference</label>
            <select
              id="remote-reference-choice"
              value={selected}
              disabled={locked || page.references.length === 0}
              onChange={(event) => controller.current?.select(event.target.value)}
            >
              <option value="">Select a name…</option>
              {page.references.map((reference) => (
                <option key={reference.name} value={reference.name}>
                  {reference.name}
                </option>
              ))}
            </select>
            <button
              type="button"
              disabled={locked || !page.choice}
              onClick={() => {
                if (page.choice) onSelect({ kind: page.choice.kind, value: page.choice.name });
              }}
            >
              Use selected reference
            </button>
          </div>
          {page.pages > 1 ? (
            <div className="actions" aria-label="Reference pages">
              <button
                type="button"
                disabled={locked || page.page === 0}
                onClick={() => controller.current?.setPage(page.page - 1)}
              >
                Previous references
              </button>
              <span>Page {page.page + 1} of {page.pages}</span>
              <button
                type="button"
                disabled={locked || page.page + 1 >= page.pages}
                onClick={() => controller.current?.setPage(page.page + 1)}
              >
                Next references
              </button>
            </div>
          ) : null}
          <p role="status">
            {page.total} matching {kind === "branch" ? "branches" : "tags"}.
            {!page.complete
              ? " Listing limited; search the remote or enter a name manually."
              : ""}
          </p>
          {scope ? (
            <p className="muted">
              Remote search scope: {scope.kind ?? "all"} · {scope.search || "all names"}.
            </p>
          ) : null}
          {page.choice ? (
            <p className="muted">
              Advertised object: <code>{page.choice.object}</code>
            </p>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
