import { expect, it } from "vitest";
import { inventoryURL } from "./inventoryNavigation";
it("links to card and set views with bookmarkable search", () => {
  expect(inventoryURL()).toBe("/inventory");
  expect(inventoryURL({ view: "sets", search: "Prismatic Evolutions" }))
    .toBe("/inventory/sets?search=Prismatic+Evolutions");
  expect(inventoryURL({ setID: "pokemon-tcgdex-sv08.5", search: "Great Tusk" }))
    .toBe("/inventory/sets/pokemon-tcgdex-sv08.5?search=Great+Tusk");
  expect(new URL(inventoryURL({ search: "Farfetch'd & Pikachu + friends" }), "https://scanner.test").searchParams.get("search"))
    .toBe("Farfetch'd & Pikachu + friends");
});
