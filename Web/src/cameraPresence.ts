import type { CardBox } from "./binderCapture";

// This is presence, not an alignment test: no stillness, orientation, centred
// placement or complete outline is required. Ignore small texture detections
// and low-confidence furniture/background boxes before invoking OCR.
export function hasCameraCard(cards: CardBox[]): boolean {
  return cards.some(card =>
    Number.isFinite(card.confidence) && card.confidence! >= 0.5 &&
    Number.isFinite(card.width) && Number.isFinite(card.height) &&
    card.width > 0 && card.height > 0 &&
    card.width <= 1 && card.height <= 1 && card.width * card.height >= 0.05,
  );
}
