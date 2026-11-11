import { expect, it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  StoreTransfer,
  storeButtonLabel,
  type StoreStatus,
} from "./StoreTransfer";
import type { Api } from "./api";

it("offers resync after completion and counts preserved cards in progress", () => {
  const status: StoreStatus = {
    configured: true,
    state: "sent",
    sentCards: 2,
    totalCards: 3,
    preservedCards: ["C-003"],
  };
  expect(storeButtonLabel(status)).toBe("Resync to Store");
  expect(storeButtonLabel({ ...status, state: "sending" })).toBe(
    "Syncing 3/3…",
  );
  const cache = new QueryClient();
  cache.setQueryData(["store-transfer", "batch"], status);
  const html = renderToStaticMarkup(
    createElement(
      QueryClientProvider,
      { client: cache },
      createElement(StoreTransfer, { api: {} as Api, id: "batch" }),
    ),
  );
  expect(html).toContain("Resync to Store");
  expect(html).not.toContain("disabled=");
  expect(html).toContain("kept unchanged in Store");
});
