import { expect, test } from "@playwright/test";

test("recognition updates preserve a continuously playing fullscreen video", async ({ page, browserName }) => {
  test.skip(browserName === "webkit", "Canvas captureStream fixture needs Chromium; camera lifecycle is also tested in WebKit.");
  await page.addInitScript(() => {
    Object.defineProperty(window, "isSecureContext", { value: true });
    Object.defineProperty(navigator, "mediaDevices", { value: {
      getUserMedia: async () => {
        const canvas = document.createElement("canvas");
        canvas.width = 1000; canvas.height = 1333;
        const context = canvas.getContext("2d")!;
        function paint() {
          context.fillStyle = "#253a45";
          context.fillRect(0, 0, 1000, 1333);
          context.fillStyle = "#efdfb0";
          context.fillRect(120, 120, 760, 1093);
          context.fillStyle = "#32828f";
          context.fillRect(175, 450, 650, 650);
          // Keep texture inside even a narrow desktop guide's crop.
          for (let y = 0; y < 1333; y += 80) {
            context.fillStyle = "#c8b775";
            context.fillRect(0, y, 1000, 25);
          }
          requestAnimationFrame(paint);
        }
        paint();
        return canvas.captureStream(30);
      },
    } });
  });
  await page.route("**/api/v1/camera/detect-page", route => route.fulfill({ json: {
    cards: [{ x: .1, y: .1, width: .8, height: .8, confidence: .9 }],
  } }));
  let lookups = 0;
  await page.route("**/api/v1/camera/identify", async route => {
    lookups++;
    await new Promise(resolve => setTimeout(resolve, 700));
    await route.fulfill({ json: { schemaVersion: 1, state: "matched", game: "pokemon", candidates: [
      { id: "video-fixture", name: "Continuous video", localID: "1", setName: "Camera fixture" },
    ] } });
  });
  await page.goto("/camera");
  await page.getByRole("button", { name: "Start camera", exact: true }).click();
  const video = page.locator("video");
  await expect(video).toBeVisible();
  await expect.poll(() => video.evaluate(v => (v as HTMLVideoElement).currentTime)).toBeGreaterThan(0.3);
  const node = await video.elementHandle();
  const stream = await video.evaluateHandle(v => (v as HTMLVideoElement).srcObject);
  await page.getByRole("button", { name: "View Continuous video details" }).waitFor();
  const first = await video.evaluate(v => (v as HTMLVideoElement).currentTime);
  await page.getByRole("button", { name: "Check again", exact: true }).click();
  await expect.poll(() => lookups).toBeGreaterThanOrEqual(2);
  await expect.poll(() => video.evaluate(v => (v as HTMLVideoElement).currentTime)).toBeGreaterThan(first + 0.5);
  expect(await video.evaluate((v, original) => v === original, node)).toBe(true);
  expect(await video.evaluate((v, original) => (v as HTMLVideoElement).srcObject === original, stream)).toBe(true);
  expect(await video.evaluate(v => {
    const element = v as HTMLVideoElement;
    const style = getComputedStyle(v);
    return { paused: element.paused, ended: element.ended, opacity: style.opacity,
      objectFit: style.objectFit, live: (element.srcObject as MediaStream).getVideoTracks()[0].readyState };
  })).toEqual({ paused: false, ended: false, opacity: "1", objectFit: "cover", live: "live" });
  const rect = await video.boundingBox();
  expect(rect).toEqual({ x: 0, y: 0, ...page.viewportSize()! });
  await page.screenshot({ path: `test-results/continuous-camera-${test.info().project.name}.png` });
  await page.getByRole("button", { name: "Stop camera", exact: true }).click();
  expect(await stream.evaluate(s => (s as MediaStream).getTracks().every(t => t.readyState === "ended"))).toBe(true);
});
