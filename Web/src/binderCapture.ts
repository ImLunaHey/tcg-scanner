import { cameraJPEG } from "./cameraCapture";
export type CardBox = { x: number; y: number; width: number; height: number; confidence?: number };
export function pocketGrid(columns: number, rows: number, inset: number): CardBox[] {
  return Array.from({ length: columns * rows }, (_, i) => ({
    x: inset + (i % columns) * (1 - inset * 2) / columns,
    y: inset + Math.floor(i / columns) * (1 - inset * 2) / rows,
    width: (1 - inset * 2) / columns, height: (1 - inset * 2) / rows,
  }));
}
export function cropBounds(box: CardBox, width: number, height: number) {
  // Keep a small margin for rounded corners, but never sample beyond the photo.
  const x = Math.max(0, Math.floor((box.x - box.width * .025) * width));
  const y = Math.max(0, Math.floor((box.y - box.height * .025) * height));
  return { x, y, width: Math.min(width - x, Math.ceil(box.width * 1.05 * width)), height: Math.min(height - y, Math.ceil(box.height * 1.05 * height)) };
}
export async function readBinderPhoto(file: File): Promise<HTMLCanvasElement> {
  if (!file.type.startsWith("image/") || file.size > 30 * 1024 * 1024) throw new Error("Choose a photo up to 30 MB.");
  const url = URL.createObjectURL(file);
  try {
    const image = new Image(); image.src = url; await image.decode();
    if (image.naturalWidth * image.naturalHeight > 48_000_000) throw new Error("Choose a photo under 48 megapixels.");
    const scale = Math.min(1, 4096 / Math.max(image.naturalWidth, image.naturalHeight));
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(image.naturalWidth * scale); canvas.height = Math.round(image.naturalHeight * scale);
    canvas.getContext("2d")!.drawImage(image, 0, 0, canvas.width, canvas.height);
    return canvas;
  } finally { URL.revokeObjectURL(url); }
}
export async function detectorJPEG(photo: HTMLCanvasElement) {
  const scale = Math.min(1, 1400 / Math.max(photo.width, photo.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(240, Math.round(photo.width * scale));
  canvas.height = Math.max(240, Math.round(photo.height * scale));
  canvas.getContext("2d")!.drawImage(photo, 0, 0, canvas.width, canvas.height);
  return cameraJPEG(canvas);
}
export async function pocketJPEG(photo: HTMLCanvasElement, box: CardBox) {
  const crop = cropBounds(box, photo.width, photo.height);
  const scale = Math.min(1, 1400 / Math.max(crop.width, crop.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(240, Math.round(crop.width * scale));
  canvas.height = Math.max(240, Math.round(crop.height * scale));
  canvas.getContext("2d")!.drawImage(photo, crop.x, crop.y, crop.width, crop.height, 0, 0, canvas.width, canvas.height);
  return cameraJPEG(canvas);
}
