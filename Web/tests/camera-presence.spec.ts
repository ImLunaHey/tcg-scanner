import { expect, test } from "@playwright/test";
import { setup } from "./camera-fixture";

test("empty room keeps sampling without OCR and a moving card starts recognition", async ({ page }) => {
  await setup(page, false, "unsupported", true);
  let present = false, checks = 0, lookups = 0;
  await page.route("**/api/v1/camera/detect-page", route => {
    checks++;
    return route.fulfill({ json: { cards: present
      ? [{ x: 0, y: .03, width: .92, height: .8, confidence: .504 }]
      : [{ x: .433, y: .004, width: .567, height: .244, confidence: .3988 }],
    } });
  });
  await page.route("**/api/v1/camera/identify", route => {
    lookups++;
    return route.fulfill({ json: { state: "noMatch", candidates: [] } });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect.poll(() => checks).toBeGreaterThanOrEqual(4);
  expect(lookups).toBe(0);
  await expect(page.getByRole("status")).toContainText("Looking for a card");
  await expect(page.getByTestId("camera-activity")).toContainText("0 lookups");
  await expect(page.getByTestId("camera-activity")).toContainText("frames sampled");
  present = true;
  await expect.poll(() => lookups, { timeout: 1500 }).toBeGreaterThan(0);
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
});

test("manual scan bypasses presence once and automatic empty checks resume", async ({ page }) => {
  await setup(page);
  let checks = 0, lookups = 0;
  await page.route("**/api/v1/camera/detect-page", route => {
    checks++;
    return route.fulfill({ json: { cards: [] } });
  });
  await page.route("**/api/v1/camera/identify", route => {
    lookups++;
    return route.fulfill({ json: { state: "noMatch", candidates: [] } });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect.poll(() => checks).toBeGreaterThanOrEqual(2);
  await page.getByRole("button", { name: "Scan this frame", exact: true }).click();
  await expect.poll(() => lookups).toBe(1);
  const before = checks;
  await expect.poll(() => checks).toBeGreaterThanOrEqual(before + 3);
  expect(lookups).toBe(1);
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
});

test("stopping during presence never sends the late frame to recognition", async ({ page }) => {
  await setup(page);
  let entered = false, lookups = 0;
  let release!: () => void;
  const wait = new Promise<void>(resolve => { release = resolve; });
  await page.route("**/api/v1/camera/detect-page", async route => {
    entered = true;
    await wait;
    await route.fulfill({ json: { cards: [{ x: 0, y: 0, width: 1, height: 1, confidence: .9 }] } }).catch(() => {});
  });
  page.on("request", request => { if (request.url().endsWith("/camera/identify")) lookups++; });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect.poll(() => entered).toBe(true);
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
  release();
  await expect(page.getByRole("button", { name: "Start camera", exact: true })).toBeVisible();
  await page.waitForTimeout(700);
  expect(lookups).toBe(0);
});

test("missing detector explains the manual fallback without automatic OCR", async ({ page }) => {
  await setup(page);
  let lookups = 0;
  await page.route("**/api/v1/camera/detect-page", route => route.fulfill({
    status: 503, json: { error: { kind: "configuration", message: "No model" } },
  }));
  await page.route("**/api/v1/camera/identify", route => {
    lookups++;
    return route.fulfill({ json: { state: "noMatch", candidates: [] } });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Automatic card detection is unavailable");
  expect(lookups).toBe(0);
  await page.getByRole("button", { name: "Scan this frame", exact: true }).click();
  await expect.poll(() => lookups).toBe(1);
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
});
