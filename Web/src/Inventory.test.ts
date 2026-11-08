import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Router } from "wouter";
import { expect, it } from "vitest";
import { InventoryRow, type InventoryEntry } from "./Inventory";
const entry: InventoryEntry = {
  id: "pending", game: "pokemon", language: "en", name: "Kyogre", setName: "Delta Reign", number: "026/103",
  status: "awaitingCatalogue", quantity: 2, confirmedQuantity: 0,
  copies: [{ batchID: "batch", batchNumber: 51, cardNumber: 1 }, { batchID: "other", batchNumber: 52, cardNumber: 3 }],
};
it("shows quantities, pending status and both physical locations", () => {
  const html = renderToStaticMarkup(createElement(Router, { ssrPath: "/inventory", children: createElement(InventoryRow, { entry }) }));
  for (const text of ["Kyogre", "026/103", "Awaiting catalogue data", "B-051 / C-001", "B-052 / C-003", "/batches/batch?card=1", "copies"])
    expect(html).toContain(text);
  expect(html).not.toContain("Confirmed");
});
it("does not label suggested catalogue matches as confirmed", () => {
  const html = renderToStaticMarkup(createElement(Router, { ssrPath: "/inventory", children: createElement(InventoryRow, { entry: { ...entry, status: "catalogue" } }) }));
  expect(html).toContain("Includes suggested matches");
  expect(html).not.toContain("Awaiting catalogue data");
});
