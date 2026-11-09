import { expect, test, type Page } from "@playwright/test";
async function setup(page: Page, deny = false) {
  await page.addInitScript(
    ({ deny }) => {
      Object.defineProperty(window, "isSecureContext", { value: true });
      const state = window as unknown as {
        cameraStops: number;
        cameraVariant: number;
        cameraURLs: Set<string>;
      };
      state.cameraVariant = 0;
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
            return { getTracks: () => [{ stop: () => state.cameraStops++ }] };
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
          this.fillStyle = "#111";
          this.fillRect(0, 0, 1000, 1333);
          this.fillStyle = state.cameraVariant ? "#444" : "#eee";
          this.fillRect(120, 130, 760, 1060);
          this.fillStyle = state.cameraVariant ? "#fff" : "#598abc";
          this.fillRect(160, 650, 680, 470);
        } else {
          Reflect.apply(original, this, args);
        }
      } as typeof original;
    },
    { deny },
  );
  await page.route("**/api/v1/events?**", (route) =>
    route.fulfill({
      contentType: "text/event-stream",
      body: "id: 1\nevent: ready\ndata: {}\n\n",
    }),
  );
}
test("camera identifies automatically without saving and stops on exit", async ({
  page,
}) => {
  await setup(page);
  let requests = 0;
  const mutations: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST")
      mutations.push(new URL(request.url()).pathname);
  });
  await page.route("**/api/v1/camera/identify", async (route) => {
    requests++;
    const data = route.request().postDataJSON();
    expect(route.request().headers().authorization).toBeUndefined();
    expect(data.language).toBe("en");
    expect(Buffer.from(data.base64, "base64").length).toBeLessThanOrEqual(
      512 * 1024,
    );
    await route.fulfill({
      json: {
        schemaVersion: 1,
        state: "matched",
        game: "pokemon",
        candidates: [
          {
            id: "swsh3-136",
            name: "Furret",
            localID: "136",
            setName: "Darkness Ablaze",
            officialSetCount: 189,
          },
        ],
      },
    });
  });
  await page.goto("/camera");
  await expect(
    page.getByRole("heading", { name: "Card camera", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  const view = await page.locator(".camera-view").boundingBox();
  const screen = page.viewportSize()!;
  expect(view!.x).toBe(0);
  expect(view!.y).toBe(0);
  expect(view!.width).toBe(screen.width);
  expect(view!.height).toBe(screen.height);
  await expect(page.locator(".sidebar")).toHaveCount(0);
  await expect(page.getByLabel("Access key", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Start camera" }).click();
  const tile = page.getByRole("button", { name: "View Furret details" });
  await expect(tile).toBeVisible();
  await expect(tile.locator("img")).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as unknown as { cameraURLs: Set<string> }).cameraURLs.size,
    ),
  ).toBe(1);
  const bounds = (await tile.boundingBox())!;
  expect(bounds.x).toBeLessThan(25);
  expect(screen.height - bounds.y - bounds.height).toBeLessThan(25);
  expect(bounds.width).toBeLessThanOrEqual(280);
  await tile.click();
  const dialog = page.getByRole("dialog");
  await expect(
    dialog.getByRole("heading", { name: "Furret", exact: true }),
  ).toBeVisible();
  await expect(
    dialog.getByText("Darkness Ablaze", { exact: true }),
  ).toBeVisible();
  await expect(dialog.getByText("136/189", { exact: true })).toBeVisible();
  const full = (await dialog.boundingBox())!;
  expect(full.width).toBe(screen.width);
  expect(full.height).toBe(screen.height);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(tile).toBeFocused();
  await page.waitForTimeout(5000);
  expect(requests).toBe(1);
  expect(mutations).toEqual(["/api/v1/camera/identify"]);
  await page.getByRole("button", { name: "Stop camera" }).click();
  await expect(
    page.getByRole("button", { name: "Start camera" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as unknown as { cameraStops: number }).cameraStops,
    ),
  ).toBe(1);
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect.poll(() => requests).toBe(2);
  await page.getByRole("link", { name: "Close camera", exact: true }).click();
  expect(
    await page.evaluate(
      () => (window as unknown as { cameraStops: number }).cameraStops,
    ),
  ).toBe(2);
  expect(
    await page.evaluate(
      () => (window as unknown as { cameraURLs: Set<string> }).cameraURLs.size,
    ),
  ).toBe(0);
  await expect(page.getByLabel("Access key", { exact: true })).toBeVisible();
});
test("denied camera access gives actionable help", async ({ page }) => {
  await setup(page, true);
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Camera access was denied",
  );
  await expect(
    page.getByRole("button", { name: "Start camera" }),
  ).toBeVisible();
});
test("install manifest starts directly on camera", async ({ request }) => {
  const response = await request.get("/manifest.webmanifest");
  const manifest = await response.json();
  expect(manifest.start_url).toBe("/camera");
  expect(manifest.display).toBe("standalone");
  for (const icon of manifest.icons)
    expect((await request.get(icon.src)).status()).toBe(200);
  const worker = await (await request.get("/sw.js")).text();
  expect(worker).not.toContain("caches.open");
  expect(worker).not.toContain("indexedDB");
});

test("backgrounding stops the stream and discards a late lookup", async ({
  page,
}) => {
  await setup(page);
  let entered = false;
  let finish!: () => void;
  const release = new Promise<void>((resolve) => {
    finish = resolve;
  });
  await page.route("**/api/v1/camera/identify", async (route) => {
    entered = true;
    await release;
    await route
      .fulfill({
        json: {
          state: "matched",
          game: "pokemon",
          candidates: [
            { id: "late", name: "Late result", setName: "Test", localID: "1" },
          ],
        },
      })
      .catch(() => {});
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect.poll(() => entered).toBe(true);
  await page.evaluate(() => {
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: true,
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect(
    page.getByRole("button", { name: "Start camera" }),
  ).toBeVisible();
  finish();
  expect(
    await page.evaluate(
      () => (window as unknown as { cameraStops: number }).cameraStops,
    ),
  ).toBe(1);
  await expect(page.getByRole("heading", { name: "Late result" })).toHaveCount(
    0,
  );
});

test("details keep their card stable while the camera sees another card", async ({
  page,
}) => {
  await setup(page);
  let calls = 0;
  await page.route("**/api/v1/camera/identify", (route) => {
    calls++;
    return route.fulfill({
      json: {
        state: "matched",
        game: "pokemon",
        candidates: [
          {
            id: String(calls),
            name: `Card ${calls}`,
            localID: String(calls),
            setName: "Test",
          },
        ],
      },
    });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera" }).click();
  await page.getByRole("button", { name: "View Card 1 details" }).click();
  await page.evaluate(() => {
    (window as unknown as { cameraVariant: number }).cameraVariant = 1;
  });
  await page.waitForTimeout(5000);
  expect(calls).toBe(1);
  await expect(
    page
      .getByRole("dialog")
      .getByRole("heading", { name: "Card 1", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close card details" }).click();
  await expect(
    page.getByRole("button", { name: "View Card 2 details" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => (window as unknown as { cameraURLs: Set<string> }).cameraURLs.size,
    ),
  ).toBe(1);
});
