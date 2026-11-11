import { expect, test } from "@playwright/test";
import { setup } from "./camera-fixture";
const card = { id: "swsh3-136", name: "Furret", localID: "136", setID: "swsh3", setName: "Darkness Ablaze", evidence: [], availableFinishes: ["normal", "reverseHolo"] };
const match = { state: "matched", game: "pokemon", candidates: [card] };
test("recent scans persist across reloads, deduplicate video and retry saves safely", async ({ page }) => {
  await setup(page);
  await page.addInitScript(() => sessionStorage.setItem("scanner.session.v1", JSON.stringify({ token: "a".repeat(64), expiresAt: new Date(Date.now() + 3600000).toISOString() })));
  await page.route("**/api/v1/camera/identify", route => route.fulfill({ json: match }));
  const submissions: unknown[] = [];
  await page.route("**/api/v1/camera/import", route => {
    submissions.push(route.request().postDataJSON());
    return submissions.length === 1 ? route.abort("failed") : route.fulfill({ json: { id: route.request().postDataJSON().id, state: "completed" } });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera" }).click();
  await expect(page.getByRole("button", { name: "Recent scans (1)", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Check again" }).click();
  await expect(page.getByRole("button", { name: "View Furret details" })).toBeVisible();
  await page.getByRole("button", { name: "Recent scans (1)", exact: true }).click();
  await expect(page.locator(".camera-recent-row")).toHaveCount(1);
  await expect(page.getByRole("dialog")).toContainText("£1.25");
  await page.getByLabel("Quantity for Furret", { exact: true }).fill("3");
  await expect(page.getByRole("button", { name: "Add 3 cards to inventory" })).toBeEnabled();
  await page.reload();
  await page.getByRole("button", { name: "Recent scans (1)", exact: true }).click();
  await expect(page.getByLabel("Quantity for Furret", { exact: true })).toHaveValue("3");
  await page.getByRole("button", { name: "Add 3 cards to inventory" }).click();
  await expect(page.getByRole("alert")).toContainText("will not add duplicates");
  await expect(page.getByLabel("Quantity for Furret", { exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Add 3 cards to inventory" }).click();
  await expect(page.getByText("Saved to inventory · view batch")).toBeVisible();
  expect(submissions).toHaveLength(2);
  expect(submissions[0]).toEqual(submissions[1]);
});
test("binder pockets keep separate copies, expose uncertainty and allow retry", async ({ page }) => {
  await setup(page);
  await page.route("**/api/v1/camera/detect-page", route => route.fulfill({ json: { cards: [
    { x: .02, y: .05, width: .45, height: .9, confidence: .9 },
    { x: .53, y: .05, width: .45, height: .9, confidence: .9 },
  ] } }));
  let lookups = 0;
  await page.route("**/api/v1/camera/identify", route => route.fulfill({ json: ++lookups === 2 ? { state: "noMatch", game: "pokemon", candidates: [] } : match }));
  await page.goto("/camera");
  await page.getByRole("button", { name: "Binder page", exact: true }).click();
  const data = await page.evaluate(() => { const c = document.createElement("canvas"); c.width = 1000; c.height = 700; const x = c.getContext("2d")!; x.fillStyle = "#ddd"; x.fillRect(20, 30, 430, 640); x.fillStyle = "#369"; x.fillRect(550, 30, 430, 640); return c.toDataURL("image/jpeg").split(",")[1]; });
  await page.getByLabel("Binder page photo", { exact: true }).setInputFiles({ name: "page.jpg", mimeType: "image/jpeg", buffer: Buffer.from(data, "base64") });
  await expect(page.locator(".binder-box")).toHaveCount(2);
  await page.getByRole("button", { name: "Identify selected cards", exact: true }).click();
  await expect(page.getByRole("button", { name: "Keep 1 matches in Recent scans" })).toBeEnabled();
  await expect(page.getByText(/No exact match/)).toBeVisible();
  await page.locator(".camera-recent-row").nth(1).getByRole("button", { name: "Retry" }).click();
  await expect(page.getByRole("button", { name: "Keep 2 matches in Recent scans" })).toBeEnabled();
  await page.getByRole("button", { name: "Keep 2 matches in Recent scans" }).click();
  await expect(page.getByText("Added to Recent scans", { exact: true })).toHaveCount(2);
  await page.getByRole("button", { name: "Review recent scans" }).click();
  await expect(page.locator(".camera-recent-row")).toHaveCount(2);
  await expect(page.getByRole("link", { name: "Connect to scanner to add to inventory" })).toBeVisible();
});
