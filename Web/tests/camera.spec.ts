import { expect, test } from "@playwright/test";
import { setup } from "./camera-fixture";

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
        finishEstimate: {
          state: "estimated",
          suggestedFinish: "holo",
          method: "fixture",
          evidence: ["Numbered catalogue printing and marker agree."],
          limitations: ["Check the physical card under light."],
        },
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
  await expect(page.getByRole("status")).toContainText("Card detected");
  await expect(tile.locator("img")).toBeVisible();
  if (screen.height <= 740 && screen.width <= 700)
    await expect(tile.getByText("Holo", { exact: true })).toBeHidden();
  else await expect(tile.getByText("Holo", { exact: true })).toBeVisible();
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
  await expect(dialog.getByText("Holo", { exact: true })).toBeVisible();
  await expect(
    dialog.getByText(/Camera estimate from printed markers/),
  ).toBeVisible();
  const full = (await dialog.boundingBox())!;
  expect(full.width).toBe(screen.width);
  expect(full.height).toBe(screen.height);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(tile).toBeFocused();
  await page.waitForTimeout(5000);
  expect(requests).toBe(1);
  expect(mutations.filter(path => path !== "/api/v1/camera/price")).toEqual(["/api/v1/camera/identify"]);
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

test("camera tries low-detail frames and shows identifying and retry feedback", async ({
  page,
}) => {
  await setup(page);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/v1/camera/identify", async (route) => {
    await pending;
    await route.fulfill({ json: { state: "noMatch", candidates: [] } });
  });
  await page.goto("/camera");
  await page.evaluate(() => {
    (window as unknown as { cameraVariant: number }).cameraVariant = 2;
  });
  await page.getByRole("button", { name: "Start camera" }).click();
  const status = page.getByRole("status");
  await expect(status).toContainText("Low detail in view");
  await expect(status).toBeInViewport();
  await page.evaluate(() => {
    (window as unknown as { cameraVariant: number }).cameraVariant = 0;
  });
  await expect(status).toContainText("Identifying card");
  await expect(status).toHaveAttribute("data-phase", "identifying");
  release();
  await expect(status).toContainText("No match yet");
  await expect(status).toHaveAttribute("data-phase", "retry");
  await page.getByRole("button", { name: "Stop camera" }).click();
  await expect(status).toHaveCount(0);
});

test("flash switches the active camera light and resets when stopped", async ({
  page,
}) => {
  await setup(page, false, "supported");
  await page.route("**/api/v1/camera/identify", (route) =>
    route.fulfill({ json: { state: "noMatch", candidates: [] } }),
  );
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera" }).click();
  const flash = page.getByRole("button", { name: "Flash", exact: true });
  await expect(flash).toHaveAttribute("aria-pressed", "false");
  await flash.click();
  await expect(flash).toHaveAttribute("aria-pressed", "true");
  await flash.click();
  await expect(flash).toHaveAttribute("aria-pressed", "false");
  expect(
    await page.evaluate(
      () => (window as unknown as { torchChanges: boolean[] }).torchChanges,
    ),
  ).toEqual([true, false]);
  await flash.click();
  await expect(flash).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Stop camera" }).click();
  await expect(flash).toHaveCount(0);
  expect(
    await page.evaluate(
      () => (window as unknown as { torchOn: boolean }).torchOn,
    ),
  ).toBe(false);
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect(flash).toHaveAttribute("aria-pressed", "false");
});

test("unsupported flash is clearly disabled", async ({ page }) => {
  await setup(page);
  await page.route("**/api/v1/camera/identify", (route) =>
    route.fulfill({ json: { state: "noMatch", candidates: [] } }),
  );
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect(
    page.getByRole("button", { name: "Flash unavailable on this camera" }),
  ).toBeDisabled();
});

for (const behavior of ["reject", "ignored"] as const) {
  test(`flash does not claim to be on when the camera ${behavior}s the request`, async ({
    page,
  }) => {
    await setup(page, false, behavior);
    await page.route("**/api/v1/camera/identify", (route) =>
      route.fulfill({ json: { state: "noMatch", candidates: [] } }),
    );
    await page.goto("/camera");
    await page.getByRole("button", { name: "Start camera" }).click();
    const flash = page.getByRole("button", { name: "Flash", exact: true });
    await flash.click();
    await expect(page.getByRole("alert")).toContainText(
      "Couldn’t change the flash",
    );
    await expect(flash).toHaveAttribute("aria-pressed", "false");
    await expect(flash).toBeEnabled();
  });
}

test("sampling continues and completed captures survive a changed live view", async ({
  page,
}) => {
  await setup(page);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  let requests = 0;
  await page.route("**/api/v1/camera/identify", async (route) => {
    requests++;
    if (requests === 1) {
      await pending;
      await route.fulfill({
        json: {
          state: "matched",
          candidates: [
            { id: "old", name: "Old card", localID: "1", setName: "Fixture" },
          ],
        },
      });
    } else await route.fulfill({ json: { state: "noMatch", candidates: [] } });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect.poll(() => requests).toBe(1);
  const activity = page.getByTestId("camera-activity");
  const count = async () =>
    Number((await activity.innerText()).match(/^(\d+)/)?.[1]);
  await expect(activity).toContainText("waiting for recognition");
  const first = await count();
  await expect.poll(count).toBeGreaterThan(first + 5);
  expect(requests).toBe(1);
  await page.evaluate(() => {
    (window as unknown as { cameraVariant: number }).cameraVariant = 1;
  });
  const changedAt = await count();
  await expect.poll(count).toBeGreaterThan(changedAt + 5);
  release();
  await expect.poll(() => requests).toBeGreaterThanOrEqual(2);
  await expect(
    page.getByRole("button", { name: "View Old card details" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
});


test("last outline rejection stays visible during the next lookup", async ({ page }) => {
  await setup(page);
  let requests = 0;
  await page.route("**/api/v1/camera/identify", async (route) => {
    requests++;
    if (requests === 1)
      await route.fulfill({ json: { state: "noCard", candidates: [] } });
    // Leave the next request pending to verify that progress doesn't hide the reason.
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  const last = page.getByTestId("camera-last-lookup");
  await expect(last).toHaveText("Last lookup: no complete card outline found");
  await expect.poll(() => requests).toBe(2);
  await expect(page.getByTestId("camera-activity")).toContainText("waiting for recognition");
  await expect(last).toBeVisible();
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect(last).toHaveCount(0);
});


test("recognition starts and retries even when every video frame moves", async ({ page }) => {
  await setup(page, false, "unsupported", true);
  let requests = 0;
  let concurrent = 0;
  let maximumConcurrent = 0;
  await page.route("**/api/v1/camera/identify", async (route) => {
    requests++;
    maximumConcurrent = Math.max(maximumConcurrent, ++concurrent);
    await new Promise((resolve) => setTimeout(resolve, 1500));
    concurrent--;
    await route.fulfill({ json: { state: "noCard", candidates: [] } });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect.poll(() => requests, { timeout: 2000 }).toBe(1);
  await expect.poll(() => requests, { timeout: 7000 }).toBeGreaterThanOrEqual(3);
  expect(maximumConcurrent).toBe(1);
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
});

test("camera feedback and prices leave the card guide unobstructed", async ({ page }) => {
  await setup(page);
  await page.route("**/api/v1/camera/identify", route => route.fulfill({ json: { state: "matched", game: "pokemon", candidates: [{ id: "me06-043", name: "Drowzee", setID: "me06", setName: "Delta Reign", localID: "043", evidence: [], availableFinishes: ["normal"] }] } }));
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect(page.getByTestId("camera-last-lookup")).toBeVisible();
  const guide = await page.locator(".camera-guide").boundingBox();
  const status = await page.getByRole("status").boundingBox();
  const diagnostics = await page.locator(".camera-diagnostics").boundingBox();
  expect(status!.y + status!.height).toBeLessThanOrEqual(guide!.y);
  expect(diagnostics!.y).toBeGreaterThanOrEqual(guide!.y + guide!.height);
  const tile = await page.locator(".camera-match-tile").boundingBox();
  const actions = await page.locator(".camera-library-actions").boundingBox();
  expect(tile!.y).toBeGreaterThanOrEqual(diagnostics!.y + diagnostics!.height);
  expect(actions!.x).toBeGreaterThanOrEqual(guide!.x + guide!.width);
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
});
