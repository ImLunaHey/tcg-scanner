import { afterEach, expect, it, vi } from "vitest";
import { Api } from "./api";
afterEach(() => vi.unstubAllGlobals());
it("uses the normal HTTP cache without credentials for image bytes", async () => {
  const fetch = vi.fn().mockResolvedValue(new Response(new Blob(["image"], { type: "image/png" })));
  vi.stubGlobal("fetch", fetch);
  await new Api("private-token").blob("images/batch:1");
  const [, options] = fetch.mock.calls[0];
  expect(options.cache).toBe("default");
  expect(options.credentials).toBe("omit");
  expect(options.headers.Authorization).toBeUndefined();
});
it("still authenticates metadata requests", async () => {
  const fetch = vi.fn().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
  await new Api("private-token").get("inventory");
  expect(fetch.mock.calls[0][1].headers.Authorization).toBe("Bearer private-token");
  expect(fetch.mock.calls[0][1].cache).toBe("no-store");
});
