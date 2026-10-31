import { describe, expect, it } from "vitest";
import { ImageRequests } from "./ImageRequests";

describe("image request scheduling", () => {
  it("limits work to two and prioritizes clicked images over queued previews", async () => {
    const queue = new ImageRequests();
    const starts: string[] = [];
    const releases = new Map<string, () => void>();
    const add = (id: string, preview = true) =>
      queue.load(
        id,
        () => {
          starts.push(id);
          return new Promise<Blob>((resolve) =>
            releases.set(id, () => resolve(new Blob([id]))),
          );
        },
        undefined,
        preview,
      );
    const a = add("a"),
      b = add("b"),
      c = add("c"),
      full = add("full", false);
    expect(starts).toEqual(["a", "b"]);
    releases.get("a")!();
    await a;
    await Promise.resolve();
    expect(starts).toEqual(["a", "b", "full"]);
    releases.get("b")!();
    await b;
    await Promise.resolve();
    expect(starts).toEqual(["a", "b", "full", "c"]);
    releases.get("full")!();
    releases.get("c")!();
    await Promise.all([c, full]);
  });
  it("caches only previews, evicts by byte limit, and isolates sessions", async () => {
    const queue = new ImageRequests(4);
    let reads = 0;
    const fetch = async () => {
      reads++;
      return new Blob(["xx"]);
    };
    await queue.load("a", fetch, undefined, true);
    await queue.load("b", fetch, undefined, true);
    await queue.load("a", fetch, undefined, true);
    expect(reads).toBe(2);
    await queue.load("c", fetch, undefined, true);
    await queue.load("b", fetch, undefined, true);
    expect(reads).toBe(4);
    await queue.load("b", fetch);
    expect(reads).toBe(5);
    await new ImageRequests().load("b", fetch, undefined, true);
    expect(reads).toBe(6);
  });
  it("does not fetch an aborted queued preview", async () => {
    const queue = new ImageRequests();
    let release!: () => void;
    const gate = new Promise<Blob>((resolve) => {
      release = () => resolve(new Blob());
    });
    const a = queue.load("a", () => gate),
      b = queue.load("b", () => gate);
    const controller = new AbortController();
    let fetched = false;
    const c = queue.load(
      "c",
      async () => {
        fetched = true;
        return new Blob();
      },
      controller.signal,
      true,
    );
    const rejected = expect(c).rejects.toThrow();
    controller.abort();
    release();
    await Promise.all([a, b]);
    await rejected;
    expect(fetched).toBe(false);
  });
});
