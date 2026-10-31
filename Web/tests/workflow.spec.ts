import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { unzipSync } from "fflate";

const key = "e".repeat(64); // Isolated mock server only; never a live credential.
test("authenticated browser workflow, retry, review, export and cancellation", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (
      message.type() === "error" &&
      /Content Security Policy/.test(message.text())
    )
      errors.push(message.text());
  });
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Connect to your scanner" }),
  ).toBeVisible();
  const anonymous = await page.request.get("/api/v1/jobs");
  expect(anonymous.status()).toBe(401);
  await page.getByLabel("Access key", { exact: true }).fill(key);
  await page.getByRole("button", { name: "Open workspace" }).click();
  await expect(
    page.getByRole("heading", { name: "Ready for the next batch." }),
  ).toBeVisible();
  await expect(page.getByLabel("Resolution", { exact: true })).toContainText(
    "300 DPI",
  );
  await expect(
    page.getByRole("button", { name: "Start scan", exact: true }),
  ).toBeDisabled();
  await expect(page.getByText("Live updates", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBeTruthy();
  // Playwright injects an inline stylesheet for WebKit screenshots; keep CSP intact.
  if (test.info().project.name !== "tablet-webkit")
    await page.screenshot({
      path: `test-results/dashboard-${test.info().project.name}.png`,
      fullPage: true,
      caret: "initial",
    });

  await page.getByRole("link", { name: "Profiles", exact: true }).click();
  await page.getByRole("button", { name: "New profile" }).click();
  const name = `Browser paper ${Date.now()}`;
  await page.getByLabel("Profile name", { exact: true }).fill(name);
  await page.getByRole("button", { name: "Save profile" }).click();
  await expect(page.getByRole("heading", { name, exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Dashboard", exact: true }).click();
  await page
    .getByLabel("Scan profile", { exact: true })
    .selectOption({ label: name });
  await page.getByLabel("I have checked the feeder setup").check();

  // The server creates the job but the first response is lost. Retry must reuse its UUID.
  let requests = 0;
  const ids: string[] = [];
  await page.route("**/api/v1/jobs", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    requests++;
    ids.push(route.request().postDataJSON().requestID);
    if (requests === 1) {
      await route.fetch();
      await route.abort("failed");
    } else await route.continue();
  });
  await page.getByRole("button", { name: "Start scan", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Retry same request" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Retry same request" }).click();
  await expect(page).toHaveURL(/#\/batches\//);
  expect(ids[0]).toBe(ids[1]);
  await page.unroute("**/api/v1/jobs");
  await expect(page.locator(".page-heading .state-completed")).toBeVisible();
  await expect(page.locator(".image-card")).toHaveCount(2);
  await expect(page.locator(".thumbnail img").first()).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBeTruthy();
  // Playwright injects an inline stylesheet for WebKit screenshots; keep CSP intact.
  if (test.info().project.name !== "tablet-webkit")
    await page.screenshot({
      path: `test-results/batch-${test.info().project.name}.png`,
      fullPage: true,
      caret: "initial",
    });
  await page.getByLabel("Select to compare").nth(0).check();
  await page.getByLabel("Select to compare").nth(1).check();
  await page.getByRole("button", { name: "Compare selected" }).click();
  await expect(
    page.getByRole("dialog", { name: "Compare captures" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close comparison" }).click();
  await page
    .getByRole("button", { name: "Identify card", exact: true })
    .first()
    .click();
  await page
    .getByRole("button", { name: "Identify this image", exact: true })
    .click();
  await expect(page.getByRole("radio")).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: "Save reviewed identity" }),
  ).toBeDisabled();
  await expect(page.getByLabel("Finish · manual")).toHaveValue("unverified");
  await expect(page.getByLabel("Condition · manual")).toHaveValue("unassessed");
  await page.getByRole("radio").check();
  await page.getByLabel("Finish · manual").selectOption("reverseHolo");
  await page.getByLabel("Condition · manual").selectOption("lightlyPlayed");
  await page.getByRole("button", { name: "Save reviewed identity" }).click();
  await expect(page.getByText(/Identity confirmed by review/)).toBeVisible();
  await page.getByRole("button", { name: "Close identification" }).click();
  await page.reload();
  await expect(page.getByText(/Furret.*Darkness Ablaze/)).toBeVisible();
  const zip = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export batch ZIP" }).click();
  const archive = await zip;
  expect(archive.suggestedFilename()).toMatch(/^B-\d+\.zip$/);
  const files = unzipSync(await readFile((await archive.path())!));
  const manifestPath = Object.keys(files).find((path) =>
    path.endsWith("/manifest.json"),
  )!;
  const manifest = JSON.parse(new TextDecoder().decode(files[manifestPath]));
  expect(manifest.images).toHaveLength(2);
  expect(manifest.cardIdentifications[0].selectedCardID).toBe("swsh3-136");
  expect(manifest.cardIdentifications[0].finish).toBe("reverseHolo");
  expect(manifest.cardIdentifications[0].condition).toBe("lightlyPlayed");
  for (const image of manifest.images) {
    const bytes =
      files[manifestPath.replace("manifest.json", image.relativePath)];
    expect(createHash("sha256").update(bytes).digest("hex")).toBe(image.sha256);
  }
  await page.getByRole("button", { name: "Edit copy" }).first().click();
  await page.getByLabel("Clockwise rotation").selectOption("1");
  await page.getByRole("button", { name: "Save edited PNG copy" }).click();
  await expect(
    page.getByRole("heading", { name: "Manually edited copies" }),
  ).toBeVisible();
  await expect(page.locator(".image-card")).toHaveCount(3);
  await page.reload();
  await expect(page.locator(".page-heading .state-completed")).toBeVisible();

  await page.getByRole("link", { name: "Dashboard", exact: true }).click();
  await page.getByLabel("I have checked the feeder setup").check();
  await page.getByRole("button", { name: "Start scan", exact: true }).click();
  await expect(page).toHaveURL(/#\/batches\//);
  await page.getByRole("button", { name: "Cancel job" }).click();
  await expect(page.locator(".page-heading .state-cancelled")).toBeVisible();
  await page.getByRole("link", { name: "Scanner", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Automatic document feeder" }),
  ).toBeVisible();
  await page.getByRole("link", { name: "Profiles", exact: true }).click();
  await page
    .getByRole("button", { name: `Delete ${name}`, exact: true })
    .click();
  await page
    .getByRole("button", { name: "Delete profile", exact: true })
    .click();
  await expect(page.getByRole("heading", { name, exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(
    page.getByRole("heading", { name: "Connect to your scanner" }),
  ).toBeVisible();
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
  await expect(page.getByText("Live updates", { exact: true })).toBeVisible({
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
