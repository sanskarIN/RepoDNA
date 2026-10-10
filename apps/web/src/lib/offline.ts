// Keeping the web version working without a connection, with the service worker that the
// build makes (sw.js). Only the web version, served without RepoDNA's server, uses it.

import { isDesktop } from "./backend";

/** Whether this page can keep itself for use without a connection. */
function supported(): boolean {
  return (
    import.meta.env.PROD && !isDesktop() && window.isSecureContext && "serviceWorker" in navigator
  );
}

/**
 * Registers the service worker when there is no server (`backend` is false), and removes one
 * a page served from the same address registered before when there is.
 */
export function keepOffline(backend: boolean): void {
  if (!supported()) {
    return;
  }
  const workers = navigator.serviceWorker;
  if (backend) {
    void workers.getRegistrations().then((registrations) => {
      for (const registration of registrations) {
        void registration.unregister();
      }
    });
    return;
  }
  // A page that cannot keep itself still works, online.
  workers.register("./sw.js").catch(() => undefined);
}
