import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { JobState } from "../lib/backend";
import { ScanRecovery } from "./ScanRecovery";

const running: JobState = {
  id: "job-1",
  input: "/project",
  status: "running",
  stages: [],
  filesDone: 1,
  filesTotal: 2,
  warnings: [],
  startedAt: "2026-11-28T00:00:00Z",
};

describe("recent native analyses", () => {
  it("loads only when asked and passes the exact job ID to the controller", async () => {
    const finished: JobState = { ...running, id: "job-2", status: "completed" };
    const load = vi.fn(async () => [running, finished]);
    const choose = vi.fn();
    render(<ScanRecovery load={load} onChoose={choose} disabled={false} />);
    expect(load).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Find recent analyses" }));
    });
    expect(screen.getByText("/project · Running")).toBeTruthy();
    expect(screen.getByText("/project · Completed")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Monitor analysis job-1/ }));
    expect(choose).toHaveBeenCalledWith("job-1");
    fireEvent.click(screen.getByRole("button", { name: /View analysis status job-2/ }));
    expect(choose).toHaveBeenCalledWith("job-2");
  });

  it("allows recovery-list retries after an error and distinguishes an empty list", async () => {
    const load = vi.fn(async (): Promise<JobState[]> => []);
    load.mockRejectedValueOnce(new Error("Native process unavailable"));
    render(<ScanRecovery load={load} onChoose={vi.fn()} disabled={false} />);
    const button = screen.getByRole("button", { name: "Find recent analyses" });
    await act(async () => {
      fireEvent.click(button);
    });
    expect(screen.getByRole("alert").textContent).toBe("Native process unavailable");
    await act(async () => {
      fireEvent.click(button);
    });
    expect(screen.getByText("No recent native analysis jobs.")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("does not allow loading or selecting while another scan is active", async () => {
    const load = vi.fn(async () => [running]);
    const choose = vi.fn();
    const { rerender } = render(<ScanRecovery load={load} onChoose={choose} disabled={false} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Find recent analyses" }));
    });
    rerender(<ScanRecovery load={load} onChoose={choose} disabled />);
    const refresh = screen.getByRole("button", {
      name: "Find recent analyses",
    }) as HTMLButtonElement;
    const monitor = screen.getByRole("button", { name: /Monitor analysis/ }) as HTMLButtonElement;
    expect(refresh.disabled).toBe(true);
    expect(monitor.disabled).toBe(true);
    fireEvent.click(monitor);
    expect(choose).not.toHaveBeenCalled();
  });
});
