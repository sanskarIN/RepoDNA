import { afterEach, expect, it, vi } from "vitest";

afterEach(() => {
  vi.restoreAllMocks();
  vi.resetModules();
});

it("retries WebAssembly manifest discovery after a transient HTTP failure", async () => {
  vi.resetModules();
  const fetchMock = vi.spyOn(globalThis, "fetch");
  fetchMock
    .mockResolvedValueOnce(new Response("unavailable", { status: 503 }))
    .mockResolvedValueOnce(new Response(JSON.stringify({ version: "1.4.2", bytes: 1024 }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }));
  const { engineInfo } = await import("./client");
  expect(await engineInfo()).toBeNull();
  expect(await engineInfo()).toEqual({ version: "1.4.2", bytes: 1024 });
  expect(fetchMock).toHaveBeenCalledTimes(2);
});
