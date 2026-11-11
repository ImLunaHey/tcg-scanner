import type { Page } from "@playwright/test";
export async function setup(
  page: Page,
  deny = false,
  torch: "supported" | "unsupported" | "reject" | "ignored" = "unsupported",
  moving = false,
) {
  await page.addInitScript(
    ({ deny, torch, moving }) => {
      Object.defineProperty(window, "isSecureContext", { value: true });
      const state = window as unknown as {
        cameraStops: number;
        cameraVariant: number;
        cameraURLs: Set<string>;
        torchOn: boolean;
        torchChanges: boolean[];
      };
      state.cameraVariant = 0;
      state.torchOn = false;
      state.torchChanges = [];
      state.cameraURLs = new Set();
      const create = URL.createObjectURL.bind(URL),
        revoke = URL.revokeObjectURL.bind(URL);
      URL.createObjectURL = (blob) => {
        const url = create(blob);
        state.cameraURLs.add(url);
        return url;
      };
      URL.revokeObjectURL = (url) => {
        state.cameraURLs.delete(url);
        revoke(url);
      };
      state.cameraStops = 0;
      Object.defineProperty(navigator, "mediaDevices", {
        value: {
          getUserMedia: async () => {
            if (deny) throw new DOMException("Denied", "NotAllowedError");
            const track = {
              id: `camera-${state.cameraStops}`,
              readyState: "live",
              stop() {
                state.cameraStops++;
                state.torchOn = false;
                this.readyState = "ended";
              },
              getCapabilities: () => ({ torch: torch !== "unsupported" }),
              getSettings: () => ({ torch: state.torchOn }),
              getConstraints: () => ({ facingMode: { ideal: "environment" } }),
              async applyConstraints(constraints: {
                advanced: { torch: boolean }[];
              }) {
                state.torchChanges.push(constraints.advanced[0].torch);
                if (torch === "reject")
                  throw new DOMException("Unavailable", "OverconstrainedError");
                if (torch !== "ignored")
                  state.torchOn = constraints.advanced[0].torch;
              },
            };
            return { getTracks: () => [track], getVideoTracks: () => [track] };
          },
        },
      });
      Object.defineProperty(HTMLVideoElement.prototype, "srcObject", {
        set() {},
        get() {
          return null;
        },
      });
      Object.defineProperty(HTMLVideoElement.prototype, "videoWidth", {
        get: () => 1000,
      });
      Object.defineProperty(HTMLVideoElement.prototype, "videoHeight", {
        get: () => 1333,
      });
      Object.defineProperty(HTMLVideoElement.prototype, "readyState", {
        get: () => 4,
      });
      HTMLVideoElement.prototype.play = async () => {};
      const original = CanvasRenderingContext2D.prototype.drawImage;
      CanvasRenderingContext2D.prototype.drawImage = function (
        this: CanvasRenderingContext2D,
        ...args: unknown[]
      ) {
        if (args[0] instanceof HTMLVideoElement) {
          if (moving) state.cameraVariant = 1 - state.cameraVariant;
          this.fillStyle = "#111";
          this.fillRect(0, 0, 1000, 1333);
          if (state.cameraVariant === 2) return;
          this.fillStyle = state.cameraVariant ? "#444" : "#eee";
          this.fillRect(120, 130, 760, 1060);
          this.fillStyle = state.cameraVariant ? "#fff" : "#598abc";
          this.fillRect(160, 650, 680, 470);
        } else {
          Reflect.apply(original, this, args);
        }
      } as typeof original;
    },
    { deny, torch, moving },
  );
  await page.route("**/api/v1/camera/detect-page", route => route.fulfill({ json: {
    schemaVersion: 1, method: "fixture", cards: [{ x: .1, y: .1, width: .8, height: .8, confidence: .9 }],
  } }));
  await page.route("**/api/v1/camera/price", route => route.fulfill({ json: { number: 1, cardID: route.request().postDataJSON().card.id, state: "priced", lowPence: 125, highPence: 125, stale: false, reason: "Fixture guide price", quotes: [{ updated: "2026-11-10" }] } }));
  await page.route("**/api/v1/events?**", (route) =>
    route.fulfill({
      contentType: "text/event-stream",
      body: "id: 1\nevent: ready\ndata: {}\n\n",
    }),
  );
}
