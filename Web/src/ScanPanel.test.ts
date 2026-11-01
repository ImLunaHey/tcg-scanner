import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScanPanel } from "./ScanPanel";
import type { ScanSubmission } from "./models";

const api = vi.hoisted(() => ({ get: vi.fn(), send: vi.fn() }));
vi.mock("./auth", () => ({ useAuth: () => ({ api }) }));
vi.mock("./data", () => ({
  useWorkspace: () => ({
    devices: { data: { devices: [{ id: "fixture", name: "Fixture" }] } },
    profiles: { data: { profiles: [] } },
    status: { data: { paused: false, stopping: false, queuedJobs: 0 } },
  }),
}));
vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
  useMutation: () => ({ mutate: vi.fn(), isPending: false }),
  useQuery: () => ({
    data: {
      units: [
        {
          type: 3,
          settings: {
            resolutionDPI: { allowed: [300, 600] },
            supportsDuplexScanning: { current: true },
          },
        },
      ],
    },
    refetch: vi.fn(),
  }),
}));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

function renderPending(pending: ScanSubmission | null) {
  vi.stubGlobal("sessionStorage", { getItem: () => JSON.stringify(pending) });
  return renderToStaticMarkup(createElement(ScanPanel, { onStarted: vi.fn() }));
}

describe("explicit scan language", () => {
  it("restores a pending Korean scan without changing or submitting its request", () => {
    const pending: ScanSubmission = {
      requestID: "pending-korean",
      deviceID: "fixture",
      dpi: 600,
      duplex: true,
      format: "png",
      prepareCards: true,
      cardLanguage: "ko",
    };
    const html = renderPending(pending);
    expect(html).toContain('<option value="ko" selected="">Korean</option>');
    expect(html).toContain("Retry same request");
    expect(html).toContain('<fieldset disabled=""');
    expect(api.send).not.toHaveBeenCalled();
    expect(pending.cardLanguage).toBe("ko");
  });
  it("defaults a new card scan to explicit English and leaves document retries alone", () => {
    expect(renderPending(null)).toContain(
      '<option value="en" selected="">English</option>',
    );
    expect(
      renderPending({
        requestID: "document",
        deviceID: "fixture",
        dpi: 300,
        duplex: false,
        format: "png",
      }),
    ).not.toContain("Card language");
    expect(api.send).not.toHaveBeenCalled();
  });
});
