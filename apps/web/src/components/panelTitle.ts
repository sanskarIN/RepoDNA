import { createContext, useContext } from "react";

/** The title of the panel a component is in, for names that say which panel they belong to. */
export const PanelTitle = createContext<string | undefined>(undefined);

export function usePanelTitle(): string | undefined {
  return useContext(PanelTitle);
}
