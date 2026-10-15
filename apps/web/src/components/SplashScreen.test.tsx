import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SplashScreen } from "./SplashScreen";
import { SUPPORT } from "../lib/links";

describe("RepoDNA startup branding", () => {
  it("shows the shared local logo and an accessible startup status", () => {
    const { container } = render(<SplashScreen />);
    expect(screen.getByRole("status", { name: "Starting RepoDNA" })).toBeTruthy();
    expect(screen.getByText("Understand your codebase. See its DNA.")).toBeTruthy();
    expect(container.querySelector('img[src="./favicon.svg"]')).toBeTruthy();
  });

  it("provides PayPal support without removing the existing options", () => {
    expect(SUPPORT.map(({ label }) => label)).toEqual(["Buy Me a Coffee", "Razorpay", "PayPal"]);
    expect(SUPPORT.find(({ label }) => label === "PayPal")?.url).toBe(
      "https://paypal.me/SanskarCodes",
    );
  });
});
