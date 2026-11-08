import { test, expect, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

const modules = process.env.SCANNER_CACHE_TEST_MODULES;
if (!modules) throw new Error("Run Scripts/test-browser-image-cache.sh");
const scope = "a".repeat(64);
async function fixture(page: Page) {
  let downloads = 0;
  await page.route("https://scanner-cache.test/**", async route => {
    const path = new URL(route.request().url()).pathname;
    if (path.startsWith("/api/v1/")) {
      downloads++;
      return route.fulfill({ status: 200, contentType: "image/png", headers: { "Cache-Control": "no-store" }, body: "fixture image bytes" });
    }
    if (path === "/") return route.fulfill({ contentType: "text/html", body: "<!doctype html><title>Image cache test</title>" });
    const name = path.slice(1).replace(/\.js$/, "");
    if (!["api", "ImageRequests", "PersistentImageCache"].includes(name)) return route.abort();
    return route.fulfill({ contentType: "text/javascript", body: await readFile(join(modules!, name + ".js")) });
  });
  await page.goto("https://scanner-cache.test/");
  return () => downloads;
}
async function image(page: Page, token: string, namespace = scope, hash = "source-a", path = "images/batch:1/thumbnail?format=jpeg") {
  return page.evaluate(async ({ token, namespace, hash, path }) => {
    const module = "/api.js";
    const { Api, imageBlob } = await import(module);
    const blob: Blob = await imageBlob(new Api(token, () => {}, namespace), path, undefined, hash);
    return blob.text();
  }, { token, namespace, hash, path });
}
test("reload, deployment and renewed login reuse stored thumbnails and full images", async ({ page }) => {
  const downloads = await fixture(page);
  expect(await image(page, "session-one")).toBe("fixture image bytes");
  await image(page, "session-one", scope, "source-a", "images/batch:1?editID=edit-one");
  expect(downloads()).toBe(2);
  await page.reload(); // New module instances: no in-memory cache or HTTP cache.
  await image(page, "session-two");
  await image(page, "session-two", scope, "source-a", "images/batch:1?editID=edit-one");
  expect(downloads()).toBe(2);
  await image(page, "session-two", scope, "source-b");
  expect(downloads()).toBe(3); // Source changed.
  await image(page, "session-two", scope, "source-a", "images/batch:1?editID=edit-two");
  expect(downloads()).toBe(4); // Different processed edit.
  await image(page, "session-other-server", "b".repeat(64));
  expect(downloads()).toBe(5); // Rotated server credentials isolate cached data.
});
test("disk cache enforces byte limits and expiry, and storage failure falls back to HTTP", async ({ page }) => {
  const downloads = await fixture(page);
  const checks = await page.evaluate(async () => {
    const module = "/PersistentImageCache.js";
    const { PersistentImageCache } = await import(module);
    const cache = new PersistentImageCache("small-cache", 4);
    const blob = new Blob(["xx"], { type: "image/png" });
    await cache.put("one", blob, 60_000);
    await cache.put("two", blob, 60_000);
    await cache.put("three", blob, 60_000);
    const evicted = await cache.get("one");
    const kept = await cache.get("three");
    await cache.put("expired", blob, -1);
    return { evicted: !!evicted, kept: !!kept, expired: !!await cache.get("expired") };
  });
  expect(checks).toEqual({ evicted: false, kept: true, expired: false });
  await page.evaluate(() => Object.defineProperty(window, "indexedDB", { get() { throw new Error("Storage disabled"); } }));
  expect(await image(page, "valid-session")).toBe("fixture image bytes");
  expect(downloads()).toBe(1);
});
