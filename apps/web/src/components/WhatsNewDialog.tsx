import { useEffect, useRef } from "react";
import { href } from "../lib/router";
import { ReleaseHighlights } from "../views/WhatsNew";

/** First-run announcement: the native dialog traps focus and supports Escape. */
export function WhatsNewDialog({ onClose }: { onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) {
      if (typeof dialog.showModal === "function") {
        dialog.showModal();
      } else {
        // The jsdom test browser and older embedded webviews lack showModal().
        dialog.open = true;
      }
    }
  }, []);

  const dismiss = () => {
    if (ref.current?.open) {
      ref.current.close();
    } else {
      onClose();
    }
  };

  return (
    <dialog
      ref={ref}
      className="dialog whats-new-dialog"
      aria-labelledby="whats-new-title"
      onClose={onClose}
    >
      <h2 id="whats-new-title">What's New in RepoDNA v{__REPODNA_VERSION__}</h2>
      <ReleaseHighlights />
      <div className="whats-new-actions">
        <button type="button" className="primary" onClick={dismiss}>
          Continue to RepoDNA
        </button>
        <a href={href("/whats-new")} onClick={dismiss}>
          Open What's New page
        </a>
      </div>
    </dialog>
  );
}
