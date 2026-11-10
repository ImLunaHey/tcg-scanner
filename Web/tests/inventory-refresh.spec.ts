import { expect, test } from "@playwright/test";

test("inventory refresh finishes every page despite live updates", async ({ page }) => {
  await page.addInitScript(() => sessionStorage.setItem("scanner.session.v1", JSON.stringify({ token: "e".repeat(64), expiresAt: "2099-01-01T00:00:00Z" })));
  const offsets: number[] = [];
  const aborted: string[] = [];
  const eventURLs: string[] = [];
  page.on("requestfailed", request => {
    if (new URL(request.url()).pathname === "/api/v1/inventory") aborted.push(request.url());
  });
  await page.route("**/api/v1/inventory?**", async route => {
    const params = new URL(route.request().url()).searchParams;
    expect(params.has("search")).toBe(false);
    const offset = Number(params.get("offset")); offsets.push(offset);
    if (offset === 100) await new Promise(resolve => setTimeout(resolve, 1200));
    await route.fulfill({ json: {
      entries: Array.from({ length: Math.min(100, 325 - offset) }, (_, index) => ({
        id: String(offset + index), game: "pokemon", language: "en", name: `Card ${offset + index}`,
        setKey: "test", setName: "Test", status: "catalogue", quantity: 1, confirmedQuantity: 1, copies: [],
      })), sets: [], totalCopies: 325, matchingEntries: 325, ...(offset < 300 ? { nextOffset: offset + 100 } : {}),
    } });
  });
  await page.route("**/api/v1/events?**", async route => {
    const url = new URL(route.request().url()); eventURLs.push(url.search);
    const after = url.searchParams.get("after");
    await new Promise(resolve => setTimeout(resolve, after === null ? 200 : 400));
    const id = Number(after || 10000) + 1;
    const data = { kind: id === 10002 ? "job.updated" : "scanner.button.failed", payload: { state: "completed" } };
    await route.fulfill({ contentType: "text/event-stream", body: after === null
      ? "id: 10001\nevent: ready\ndata: {}\n\n"
      : `id: ${id}\nevent: scanner\ndata: ${JSON.stringify(data)}\n\n` });
  });
  for (let run = 0; run < 2; run++) {
    offsets.length = 0; eventURLs.length = 0;
    if (run === 0) await page.goto("/inventory"); else await page.reload();
    await expect(page.locator(".inventory-card")).toHaveCount(325);
    expect(offsets).toEqual([0, 100, 200, 300]);
    expect(aborted).toEqual([]);
    expect(new URLSearchParams(eventURLs[0]).has("after")).toBe(false);
    expect(eventURLs.some(url => new URLSearchParams(url).get("after") === "10001")).toBe(true);
    await expect(page.getByText("Request limit reached; retry later.")).toHaveCount(0);
  }
});
