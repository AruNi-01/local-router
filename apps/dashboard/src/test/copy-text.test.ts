import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/components/ui/sonner", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

import { toast } from "@/components/ui/sonner";
import { copyText } from "@/lib/clipboard";

const mockedToast = vi.mocked(toast, { deep: true });

describe("copyText", () => {
  const writeText = vi.fn();

  beforeEach(() => {
    mockedToast.success.mockReset();
    mockedToast.error.mockReset();
    writeText.mockReset();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
  });

  it("copies text and shows a success toast", async () => {
    writeText.mockResolvedValueOnce(undefined);

    await copyText("http://route.localhost", "Route URL");

    expect(writeText).toHaveBeenCalledWith("http://route.localhost");
    expect(mockedToast.success).toHaveBeenCalledWith("Route URL copied");
    expect(mockedToast.error).not.toHaveBeenCalled();
  });

  it("shows an error toast when clipboard access fails", async () => {
    writeText.mockRejectedValueOnce(new Error("permission denied"));

    await copyText("http://route.localhost", "Route URL");

    expect(mockedToast.error).toHaveBeenCalledWith("Failed to copy route url");
    expect(mockedToast.success).not.toHaveBeenCalled();
  });
});
