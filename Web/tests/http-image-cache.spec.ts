import { chromium, webkit, expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

test("public image content and validators survive reload without tokens", async ({ browserName, baseURL }, info) => {
  // WebKit ephemeral contexts intentionally have no persistent HTTP cache.
  // Use disposable browser profiles to test normal browsing, not private mode.
  const directory = await mkdtemp(join(tmpdir(), "scanner-http-cache-"));
  const context = await (browserName === "webkit" ? webkit : chromium).launchPersistentContext(directory, {
    baseURL, viewport: info.project.use.viewport, isMobile: info.project.use.isMobile,
  });
  const page = await context.newPage();
  try {
    const headers = { Authorization: "Bearer " + "e".repeat(64) }; // Isolated Rust fixture only.
    const batches = await (await page.request.get("/api/v1/batches", { headers })).json();
    const id = batches.jobs[0].id;
    const sent: (string | undefined)[] = [];
    page.on("request", request => { if (request.url().includes("/api/v1/images/")) sent.push(request.headers().authorization); });
    const read = () => page.evaluate(async id => {
      const result = [];
      for (const suffix of ["", "/thumbnail?format=jpeg"]) {
        const response = await fetch(`/api/v1/images/${id}:1${suffix}`, { credentials: "omit", cache: "default" });
        const blob = await response.blob();
        result.push({ status: response.status, bytes: blob.size, etag: response.headers.get("ETag"), vary: response.headers.get("Vary") });
      }
      return result;
    }, id);
    await page.goto("/");
    const first = await read();
    expect(first.every(image => image.status === 200 && image.bytes > 0 && image.etag && image.vary === null)).toBeTruthy();
    await page.reload();
    expect(await read()).toEqual(first); // Stable content and validators after reload.
    expect(sent.every(authorization => authorization === undefined)).toBeTruthy();
  } finally { await context.close(); await rm(directory, { recursive: true, force: true }); }
});
