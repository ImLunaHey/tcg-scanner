// Map the visible guide area back into object-fit:cover video pixels.
export function cameraViewport(
  width: number,
  height: number,
  viewport: { width: number; height: number },
  region: { x: number; y: number; width: number; height: number },
) {
  const scale = Math.max(viewport.width / width, viewport.height / height);
  const offsetX = (width * scale - viewport.width) / 2;
  const offsetY = (height * scale - viewport.height) / 2;
  return {
    x: (region.x + offsetX) / scale,
    y: (region.y + offsetY) / scale,
    width: region.width / scale,
    height: region.height / scale,
  };
}
export async function cameraSnapshot(
  canvas: HTMLCanvasElement,
): Promise<Blob | null> {
  const snapshot = document.createElement("canvas");
  snapshot.width = 840;
  snapshot.height = 1173;
  // A little extra space outside the guide keeps edges in the review preview.
  snapshot
    .getContext("2d")!
    .drawImage(canvas, 80, 80, 840, 1173, 0, 0, 840, 1173);
  return new Promise((resolve) => snapshot.toBlob(resolve, "image/jpeg", 0.85));
}
export function difference(a: Uint8Array, b: Uint8Array): number {
  if (a.length !== b.length || !a.length) return Infinity;
  return (
    a.reduce((sum, value, i) => sum + Math.abs(value - b[i]), 0) / a.length
  );
}
export class SteadyCard {
  previous?: Uint8Array;
  submitted?: Uint8Array;
  stable = 0;
  lastAttempt = -Infinity;
  matched = false;
  ready(frame: Uint8Array, now: number): boolean {
    const mean = frame.reduce((a, b) => a + b, 0) / frame.length;
    const variance =
      frame.reduce((a, b) => a + (b - mean) ** 2, 0) / frame.length;
    this.stable =
      this.previous && difference(frame, this.previous) < 5
        ? this.stable + 1
        : 0;
    this.previous = frame;
    if (variance < 180 || this.stable < 2 || now - this.lastAttempt < 4500)
      return false;
    if (
      this.matched &&
      this.submitted &&
      difference(frame, this.submitted) < 12
    )
      return false;
    return true;
  }
  accept(frame: Uint8Array, now: number) {
    this.submitted = frame;
    this.lastAttempt = now;
    this.matched = false;
  }
}
export function sampleCard(canvas: HTMLCanvasElement): Uint8Array {
  const small = document.createElement("canvas");
  small.width = 48;
  small.height = 64;
  const context = small.getContext("2d", { willReadFrequently: true })!;
  context.drawImage(
    canvas,
    canvas.width * 0.12,
    canvas.height * 0.1,
    canvas.width * 0.76,
    canvas.height * 0.8,
    0,
    0,
    48,
    64,
  );
  const pixels = context.getImageData(0, 0, 48, 64).data;
  return Uint8Array.from({ length: 48 * 64 }, (_, i) =>
    Math.round((pixels[i * 4] + pixels[i * 4 + 1] + pixels[i * 4 + 2]) / 3),
  );
}
export async function cameraJPEG(canvas: HTMLCanvasElement): Promise<string> {
  for (const quality of [0.82, 0.65, 0.45]) {
    const blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob(resolve, "image/jpeg", quality),
    );
    if (blob && blob.size <= 512 * 1024) {
      return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onerror = () =>
          reject(new Error("Could not read the camera frame."));
        reader.onload = () => resolve(String(reader.result).split(",")[1]);
        reader.readAsDataURL(blob);
      });
    }
  }
  throw new Error("This camera frame is too large. Try a simpler background.");
}
