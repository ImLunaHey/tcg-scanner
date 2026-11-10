import { expect, it } from "vitest";
import { sameCameraCard } from "./cameraHistory";
import { selectedCameraCard, type CameraSelection } from "./CameraPrice";
const selection: CameraSelection = { card: { id: "sv01-104", name: "Greavard", setName: "SV", setID: "sv01", localID: "104", evidence: [], availableFinishes: ["normal"] }, game: "pokemon", language: "en", finish: "unverified" };
it("repeated frames count once while languages, games and reviewed finishes remain distinct", () => {
  expect(sameCameraCard(selection, { ...selection, card: { ...selection.card, name: "Updated name" } })).toBe(true);
  expect(sameCameraCard(selection, { ...selection, language: "ko" })).toBe(false);
  expect(sameCameraCard(selection, { ...selection, finish: "holo" })).toBe(false);
  expect(sameCameraCard(selection, { ...selection, game: "yugioh" })).toBe(false);
});
it("only a single supported match can enter the recent list automatically", () => {
  expect(selectedCameraCard({ state: "ambiguous", game: "pokemon", candidates: [selection.card] }, "en")).toBeUndefined();
  expect(selectedCameraCard({ state: "matched", game: "unknown", candidates: [selection.card] }, "en")).toBeUndefined();
  expect(selectedCameraCard({ state: "matched", game: "pokemon", candidates: [selection.card] }, "en")?.card.id).toBe(selection.card.id);
});
