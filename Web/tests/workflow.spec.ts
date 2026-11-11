import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { unzipSync } from "fflate";

const key = "e".repeat(64); // Isolated Rust fixture only.

test("original workspace layout, profiles, paired review and verified exports", async ({ page }, info) => {
  const errors: string[] = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Connect to your scanner" })).toBeVisible();
  expect((await page.request.get("/api/v1/jobs")).status()).toBe(401);
  await page.getByLabel("Access key", { exact: true }).fill(key);
  await page.getByRole("button", { name: "Open workspace" }).click();
  await expect(page.getByRole("heading", { name: "Ready for the next batch." })).toBeVisible();
  for (const name of ["Dashboard", "Camera", "Library", "Inventory", "Profiles", "Scanner"])
    await expect(page.getByRole("link", { name, exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Start scan", exact: true })).toBeDisabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  if (info.project.name !== "tablet-webkit")
    await page.screenshot({ path: `test-results/restored-dashboard-${info.project.name}.png`, fullPage: true });

  await page.getByRole("link", { name: "Profiles", exact: true }).click();
  await page.getByRole("button", { name: "New profile" }).click();
  const name = `Browser profile ${Date.now()}`;
  await page.getByLabel("Profile name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Save profile" }).click();
  await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
  await page.getByRole("button", { name: `Delete ${name}`, exact: true }).click();
  await page.getByRole("button", { name: "Delete profile", exact: true }).click();
  await expect(page.getByRole("heading", { name, exact: true })).toHaveCount(0);

  await page.getByRole("link", { name: "Library", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Batch library" })).toBeVisible();
  await page.getByRole("link", { name: /B-001/ }).click();
  await expect(page.locator(".card-pair")).toHaveCount(1);
  await expect(page.locator(".thumbnail img")).toHaveCount(2);
  await expect(page.getByText("Browser test card · 피카츄", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Enlarge front of C-001", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  // The original document grid exposes editing/review; the compact paired gallery
  // preserves its existing layout. Both are backed by actual saved Rust fixtures.
  await page.getByRole("link", { name: "Library", exact: true }).click();
  await page.getByRole("link", { name: /B-002/ }).click();
  await expect(page.locator(".image-card")).toHaveCount(2);
  await page.getByRole("button", { name: "Review identity", exact: true }).first().click();
  await page.getByRole("radio").first().check();
  await page.getByLabel("Finish · manual").selectOption("reverseHolo");
  await page.getByLabel("Condition · manual").selectOption("lightlyPlayed");
  const save = page.getByRole("button", { name: "Save reviewed identity" });
  // Exercise keyboard submission in mobile Chromium as well as pointer actions
  // in the desktop and WebKit projects. Its emulated visual viewport can pan
  // during selectOption and misreport hit-test coordinates after scrolling.
  if (info.project.name === "mobile") await save.press("Enter");
  else await save.click();
  await expect(page.getByText(/Identity confirmed by review/)).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const close = page.getByRole("button", { name: "Close identification" });
  if (info.project.name === "mobile") await close.press("Enter");
  else await close.click();
  const zip = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export batch ZIP" }).click();
  const archive = await zip;
  const files = unzipSync(await readFile((await archive.path())!));
  const manifestPath = Object.keys(files).find(path => path.endsWith("/manifest.json"))!;
  const manifest = JSON.parse(new TextDecoder().decode(files[manifestPath]));
  expect(manifest.images).toHaveLength(2);
  expect(manifest.cardIdentifications[0].selectedCardID).toBe("fixture-1");
  expect(manifest.cardIdentifications[0].finish).toBe("reverseHolo");
  for (const image of manifest.images)
    expect(createHash("sha256").update(files[manifestPath.replace("manifest.json", image.relativePath)]).digest("hex")).toBe(image.sha256);
  await page.reload(); // Direct SPA URL must remain routable through the Rust host.
  await expect(page.locator(".image-card")).toHaveCount(2);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await expect(page.getByLabel("Access key", { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});

test("recovers connection loss and clears a revoked session", async ({
  page,
}) => {
  await page.route("**/api/v1/events?**", (route) => route.abort("failed"));
  await page.goto("/");
  await page.getByLabel("Access key", { exact: true }).fill(key);
  await page.getByRole("button", { name: "Open workspace" }).click();
  await expect(page.getByText("Reconnecting", { exact: true })).toBeVisible();
  await page.unroute("**/api/v1/events?**");
  await expect(page.getByText("Reconnecting", { exact: true })).toBeHidden({
    timeout: 10_000,
  });
  await page.route("**/api/v1/status", (route) => route.abort("failed"));
  await expect(
    page.getByRole("heading", { name: "Let’s reconnect." }),
  ).toBeVisible({ timeout: 12_000 });
  await expect(
    page.getByRole("button", { name: "Start scan", exact: true }),
  ).toBeDisabled();
  await page.unroute("**/api/v1/status");
  await expect(
    page.getByRole("heading", { name: "Ready for the next batch." }),
  ).toBeVisible({ timeout: 12_000 });
  const session = await page.evaluate(
    () => JSON.parse(sessionStorage.getItem("scanner.session.v1")!).token,
  );
  const revoked = await page.request.delete("/api/v1/session", {
    headers: { Authorization: `Bearer ${session}` },
  });
  expect(revoked.status()).toBe(200);
  await expect(
    page.getByRole("heading", { name: "Connect to your scanner" }),
  ).toBeVisible({ timeout: 12_000 });
  expect(
    await page.evaluate(() => sessionStorage.getItem("scanner.session.v1")),
  ).toBeNull();
});
