import { expect, test, type Page } from "@playwright/test";
async function setup(page: Page, deny = false) {
  await page.addInitScript(
    ({ deny }) => {
      sessionStorage.setItem(
        "scanner.session.v1",
        JSON.stringify({
          token: "e".repeat(64),
          expiresAt: "2099-01-01T00:00:00Z",
        }),
      );
      Object.defineProperty(window, "isSecureContext", { value: true });
      const state = window as unknown as { cameraStops: number };
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
          this.fillStyle = "#eee";
          this.fillRect(120, 130, 760, 1060);
          this.fillStyle = "#598abc";
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
    page.getByRole("heading", { name: "Camera", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  const view = await page.locator(".camera-view").boundingBox();
  expect(view!.width / view!.height).toBeCloseTo(0.75, 2);
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect(
    page.getByRole("heading", { name: "Furret", exact: true }),
  ).toBeVisible();
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
  await page.getByRole("link", { name: "Inventory", exact: true }).click();
  expect(
    await page.evaluate(
      () => (window as unknown as { cameraStops: number }).cameraStops,
    ),
  ).toBe(2);
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


test("backgrounding stops the stream and discards a late lookup", async ({ page }) => {
  await setup(page);
  let entered = false;
  let finish!: () => void;
  const release = new Promise<void>(resolve => { finish = resolve; });
  await page.route("**/api/v1/camera/identify", async route => {
    entered = true; await release;
    await route.fulfill({ json: { state: "matched", game: "pokemon", candidates: [{ id: "late", name: "Late result", setName: "Test", localID: "1" }] } }).catch(() => {});
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect.poll(() => entered).toBe(true);
  await page.evaluate(() => {
    Object.defineProperty(document, "hidden", { configurable: true, value: true });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect(page.getByRole("button", { name: "Start camera" })).toBeVisible();
  finish();
  expect(await page.evaluate(() => (window as unknown as { cameraStops: number }).cameraStops)).toBe(1);
  await expect(page.getByRole("heading", { name: "Late result" })).toHaveCount(0);
});
