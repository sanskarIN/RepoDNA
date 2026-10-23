import { expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { WhatsNewDialog } from "./WhatsNewDialog";

it("shows the installed release and allows the user to dismiss What's New", () => {
  const onClose = vi.fn();
  render(<WhatsNewDialog onClose={onClose} />);
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(screen.getByRole("heading", { name: /What's New in RepoDNA/ })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Continue to RepoDNA" }));
  expect(onClose).toHaveBeenCalledOnce();
});

it("offers a permanent What's New page after first-run onboarding", () => {
  render(<WhatsNewDialog onClose={() => undefined} />);
  const link = screen.getByRole("link", { name: "Open What's New page" });
  expect(link.getAttribute("href")).toBe("#/whats-new");
});
