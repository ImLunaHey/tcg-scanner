// Keep the browser's ordinary HTTP cache. Never persist camera frames or API responses.
self.addEventListener("fetch", event => {
  if (event.request.mode !== "navigate") return;
  event.respondWith(fetch(event.request).catch(() => new Response(
    '<!doctype html><html lang="en"><meta name="viewport" content="width=device-width"><title>Scanner offline</title><body><h1>Connect to your Mac</h1><p>Camera identification needs your Mac and Tailscale online.</p><a href="/camera">Try again</a></body></html>',
    { status: 503, headers: { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" } }
  )));
});
