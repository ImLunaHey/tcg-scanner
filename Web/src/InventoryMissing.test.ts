import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it } from "vitest";
import { AuthProvider } from "./auth";
import { MissingInventoryCard } from "./InventoryMissing";
it("marks a missing card visually without stock or batch actions", () => {
  const node = createElement(MissingInventoryCard, { card: { id: "test-2", name: "Pikachu", number: "002", language: "en" }, setName: "Test" });
  const html = renderToStaticMarkup(createElement(QueryClientProvider, { client: new QueryClient(), children: createElement(AuthProvider, { children: node }) }));
  expect(html).toContain("inventory-card-missing");
  expect(html).toContain("Pikachu, missing");
  expect(html).toContain("Artwork unavailable");
  expect(html).not.toContain("Locations");
  expect(html).not.toContain("×");
});
