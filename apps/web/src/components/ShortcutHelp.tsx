import { useEffect, useRef } from "react";

export const SHORTCUTS: [string, string][] = [
  ["Ctrl/Cmd + K", "Search views, files, modules, packages, and findings; run commands"],
  ["Ctrl/Cmd + P", "Quick open a file or module"],
  ["Ctrl/Cmd + F", "Search the current view (when it has a search box)"],
  ["↑ / ↓ and Enter", "Move through results and choose one"],
  ["Esc", "Close a dialog"],
  ["?", "Show these shortcuts"],
];

export function ShortcutTable() {
  return (
    <table className="shortcuts">
      <tbody>
        {SHORTCUTS.map(([keys, action]) => (
          <tr key={keys}>
            <td>
              <kbd>{keys}</kbd>
            </td>
            <td>{action}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function ShortcutHelp({ onClose }: { onClose: () => void }) {
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    button.current?.focus();
  }, []);
  return (
    <div
      className="overlay"
      onPointerDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="shortcuts-title">
        <h2 id="shortcuts-title">Keyboard shortcuts</h2>
        <ShortcutTable />
        <p>
          <button ref={button} type="button" onClick={onClose}>
            Close
          </button>
        </p>
      </div>
    </div>
  );
}
