import { expect, it } from "vitest";
import { newestSetsFirst, setReleaseLabel } from "./inventorySetOrder";
it("orders newest releases first and unknown dates last, without changing the input", () => {
  const sets = [{ id: "unknown", name: "A" }, { id: "old", name: "Old", releaseDate: "2024-01-01" }, { id: "new", name: "New", releaseDate: "2026-11-07" }];
  expect(newestSetsFirst(sets).map(s => s.id)).toEqual(["new", "old", "unknown"]);
  expect(sets[0].id).toBe("unknown");
  expect(setReleaseLabel()).toBe("Release date unknown");
  expect(setReleaseLabel("2026-11-07")).toContain("2026");
});
