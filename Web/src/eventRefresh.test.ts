import { expect, it } from "vitest";
import { InfiniteQueryObserver, QueryClient } from "@tanstack/react-query";
import { eventQueryKeys, refreshEventQueries } from "./events";

it("refreshes inventory for finished batch changes, not button errors or scan progress", () => {
  for (const kind of ["scanner.button.failed", "diagnostics.buttonMonitor.failed", "device.connected"])
    expect(eventQueryKeys({ event: "scanner", data: JSON.stringify({ kind }) })).not.toContain("inventory");
  for (const state of ["queued", "preparing", "scanning", "processing", "completed", "failed", "cancelled"]) {
    const keys = eventQueryKeys({ event: "scanner", data: JSON.stringify({ kind: "job.updated", payload: { state } }) });
    expect(keys.includes("inventory")).toBe(["completed", "failed", "cancelled"].includes(state));
  }
  expect(eventQueryKeys({ event: "ready", data: "{}" })).toContain("inventory");
  expect(eventQueryKeys({ event: "reset", data: "{}" })).toContain("inventory");
  expect(eventQueryKeys({ event: "message", data: "" })).toEqual([]);
});

it("live updates do not cancel the next inventory page and restart page one", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const requests: number[] = [];
  let aborted = false;
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  const observer = new InfiniteQueryObserver(client, {
    queryKey: ["inventory", "cards", "", ""], initialPageParam: 0,
    queryFn: async ({ pageParam, signal }) => {
      requests.push(pageParam);
      signal.addEventListener("abort", () => { aborted = true; });
      if (pageParam === 100) await held;
      return { offset: pageParam, nextOffset: pageParam === 0 ? 100 : undefined };
    },
    getNextPageParam: page => page.nextOffset,
  });
  const unsubscribe = observer.subscribe(() => {});
  try {
    await observer.refetch();
    const next = observer.fetchNextPage();
    expect(requests).toEqual([0, 100]);
    const updates = Array.from({ length: 10 }, () => refreshEventQueries(client, new Set(["inventory"])));
    expect(aborted).toBe(false);
    release();
    await Promise.all([next, ...updates]);
    expect(requests).toEqual([0, 100]);
    expect(observer.getCurrentResult().data?.pages.map(page => page.offset)).toEqual([0, 100]);
    // A later real change still refreshes completed inventory pages.
    await refreshEventQueries(client, new Set(["inventory"]));
    expect(requests).toEqual([0, 100, 0, 100]);
  } finally { release(); unsubscribe(); client.clear(); }
});
