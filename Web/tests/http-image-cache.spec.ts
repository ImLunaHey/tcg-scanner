import { chromium, webkit, expect, test } from "@playwright/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

test("normal HTTP image cache survives page reload without tokens", async ({ browserName, baseURL }, info) => {
  // WebKit ephemeral contexts intentionally have no persistent HTTP cache.
  // Use disposable browser profiles to test normal browsing, not private mode.
  const directory = await mkdtemp(join(tmpdir(), "scanner-http-cache-"));
  const context = await (browserName === "webkit" ? webkit : chromium).launchPersistentContext(directory, {
    baseURL, viewport: info.project.use.viewport, isMobile: info.project.use.isMobile,
  });
  const page = await context.newPage();
  try {
    const headers = { Authorization: "Bearer " + "e".repeat(64) }; // Isolated Swift fixture only.
    const id = randomUUID();
    expect((await page.request.post("/api/v1/jobs", { headers, data: { requestID: id, duplex: true, format: "png" } })).status()).toBe(202);
    await expect.poll(async () => (await (await page.request.get(`/api/v1/jobs/${id}`, { headers })).json()).state, { timeout: 15_000 }).toBe("completed");
    const sent: (string | undefined)[] = [];
    page.on("request", request => { if (request.url().includes("/api/v1/images/")) sent.push(request.headers().authorization); });
    const read = () => page.evaluate(async id => {
      const result = [];
      for (const suffix of ["", "/thumbnail?format=jpeg"]) {
        const response = await fetch(`/api/v1/images/${id}:1${suffix}`, { credentials: "omit", cache: "default" });
        const blob = await response.blob();
        result.push({ status: response.status, bytes: blob.size, request: response.headers.get("X-Test-Image-Request"), vary: response.headers.get("Vary") });
      }
      return result;
    }, id);
    await page.goto("/");
    const first = await read();
    expect(first.every(image => image.status === 200 && image.bytes > 0 && image.request && image.vary === null)).toBeTruthy();
    await page.reload();
    expect(await read()).toEqual(first); // Same response counter: no image requests reached the server.
    expect(sent.every(authorization => authorization === undefined)).toBeTruthy();
  } finally { await context.close(); await rm(directory, { recursive: true, force: true }); }
});
