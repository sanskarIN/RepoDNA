// jsdom lacks a few browser APIs the interface uses; these stand-ins keep tests honest
// about behavior without pretending to lay anything out.

import { afterEach, beforeEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { WHATS_NEW_SEEN_KEY } from "../lib/whats-new";

beforeEach(() => {
  // Existing UI tests represent returning users; announcement tests opt in explicitly.
  window.localStorage.setItem(WHATS_NEW_SEEN_KEY, __REPODNA_VERSION__);
});

afterEach(() => {
  cleanup();
  window.location.hash = "";
  // What one test opened must not be reopened, as after a reload, by the next.
  window.sessionStorage.clear();
  window.localStorage.clear();
});

if (!window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    }) as MediaQueryList;
}

// jsdom defines scrollTo but only reports that it is not implemented, and has no
// scrollIntoView.
window.scrollTo = () => undefined;
Element.prototype.scrollIntoView = () => undefined;

// Lets React know updates in tests are wrapped in act().
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
