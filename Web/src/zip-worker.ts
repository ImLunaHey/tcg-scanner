import { zipSync } from "fflate";
self.onmessage = (event: MessageEvent<Record<string, Uint8Array>>) => {
  const archive = zipSync(event.data, { level: 0 });
  self.postMessage(archive, { transfer: [archive.buffer] });
};
