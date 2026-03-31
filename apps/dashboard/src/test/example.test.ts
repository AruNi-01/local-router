import { afterEach, describe, expect, it, vi } from "vitest";

describe("dashboard api base", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
  });

  it("uses the local daemon API by default", async () => {
    const { api } = await import("@/lib/api");
    expect(api.apiUrl("/health")).toBe("http://127.0.0.1:9731/v1/health");
  });

  it("honors VITE_LOCALROUTER_API overrides", async () => {
    vi.stubEnv("VITE_LOCALROUTER_API", "http://127.0.0.1:9999/v1");
    const { api } = await import("@/lib/api");
    expect(api.apiUrl("/events")).toBe("http://127.0.0.1:9999/v1/events");
  });
});
