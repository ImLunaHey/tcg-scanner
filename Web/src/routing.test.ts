import { expect, it } from "vitest";
import { legacyRoute } from "./routing";

it("migrates existing app bookmarks without accepting external or API paths", () => {
  for (const path of [
    "/",
    "/library",
    "/profiles",
    "/scanner",
    "/batches/5F2213C2-7E6E-41E1-9A9B-C756613A1A15",
  ]) {
    expect(legacyRoute("#" + path)).toBe(path);
  }
  for (const hash of [
    "",
    "#main-content",
    "#//evil.example",
    "#/api/v1/jobs",
    "#/assets/missing.js",
    "#/batches/invalid",
    "#/library?token=secret",
    "#/../token",
  ]) {
    expect(legacyRoute(hash)).toBeUndefined();
  }
});
