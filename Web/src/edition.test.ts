import { describe, expect, it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { CardSummary } from "./CardSummary";
import {
  editionPresentation,
  finishPresentation,
  type EditionEvidence,
  type Identification,
} from "./Identification";
import {
  matchingCardPrice,
  pricingRevision,
  type CardValuation,
} from "./Pricing";

const edition: EditionEvidence = {
  state: "matched",
  candidateID: "sv02-097",
  name: "Trick or Trade",
  pricingKey: "edition-fixture",
  stamp: {
    state: "present",
    kind: "pikachu-pumpkin",
    method: "fixture",
    evidence: ["A pumpkin-shaped mark was found locally."],
  },
  variants: [
    {
      id: "stamped",
      source: "tcgdex",
      finish: "holo",
      stamps: ["trick-or-trade"],
      years: [],
    },
  ],
  evidence: ["The catalogue lists the marked printing."],
};
const identity: Identification = {
  request: { id: "identity", sequence: 2, language: "en" },
  state: "reviewRequired",
  sourceSHA256: "fixture",
  finish: "unverified",
  condition: "unassessed",
  evidence: {
    candidates: [
      {
        id: "sv02-097",
        name: "Mimikyu",
        localID: "097",
        setID: "sv02",
        setName: "Paldea Evolved",
        evidence: [],
        availableFinishes: ["holo"],
      },
    ],
    lines: [],
    warnings: [],
  },
  finishEstimate: {
    candidateID: "sv02-097",
    state: "estimated",
    suggestedFinish: "holo",
    method: "fixture",
    evidence: [],
    limitations: [],
    edition,
  },
};
const quote: CardValuation = {
  number: 1,
  cardID: "sv02-097",
  editionKey: "edition-fixture",
  state: "priced",
  lowPence: 12,
  highPence: 12,
  stale: false,
  reason: "Stamped quote",
  quotes: [],
};
function withEdition(value: EditionEvidence): Identification {
  return {
    ...identity,
    finishEstimate: { ...identity.finishEstimate!, edition: value },
  };
}

describe("stamped edition presentation", () => {
  it("keeps unresolved marks visible when no base card was recognized", () => {
    const unknown: Identification = {
      ...identity,
      state: "noMatch",
      evidence: { candidates: [], lines: [], warnings: [] },
    };
    const panel = renderToStaticMarkup(
      createElement(CardSummary, {
        label: "C-001",
        identification: unknown,
        pending: false,
        incomplete: false,
        openMarks: () => undefined,
      }),
    );
    expect(panel).toContain("No card match found");
    expect(panel).toContain("Pumpkin stamp · edition unresolved");
    expect(panel).not.toContain("Trick or Trade");
  });
  it("names only a current matched catalogue edition and does not invent a year", () => {
    const result = editionPresentation(identity);
    expect(result).toMatchObject({
      label: "Trick or Trade",
      state: "resolved",
    });
    expect(result?.detail).toContain("found locally");
    expect(result?.detail).toContain("printing year or finish");
    expect(
      editionPresentation(withEdition({ ...edition, candidateID: "other" })),
    ).toMatchObject({
      label: "Pumpkin stamp · edition unresolved",
      state: "uncertain",
    });
    expect(editionPresentation({ ...identity, state: "rejected" })?.state).toBe(
      "uncertain",
    );
    expect(
      editionPresentation({ ...identity, state: "rejected" }, "sv02-097")
        ?.state,
    ).toBe("uncertain");
  });
  it("keeps absent, uncertain and unlisted marks separate", () => {
    expect(
      editionPresentation(
        withEdition({
          ...edition,
          stamp: { ...edition.stamp, state: "absent" },
        }),
      ),
    ).toBeUndefined();
    expect(
      editionPresentation(
        withEdition({
          ...edition,
          stamp: { ...edition.stamp, state: "uncertain" },
        }),
      )?.label,
    ).toBe("Possible pumpkin stamp");
    expect(
      editionPresentation(withEdition({ ...edition, state: "unlisted" }))
        ?.label,
    ).toBe("Pumpkin stamp · edition unresolved");
    expect(
      editionPresentation(withEdition({ ...edition, name: undefined }))?.state,
    ).toBe("uncertain");
  });
  it("explains edition-related finish abstention without overriding physical review", () => {
    for (const [reason, explanation] of [
      ["stampUncertain", "possible pumpkin stamp"],
      ["editionUnavailable", "catalogue edition is unresolved"],
      ["multipleEditionFinishes", "different finishes"],
    ]) {
      const unresolved: Identification = {
        ...identity,
        finishEstimate: {
          ...identity.finishEstimate!,
          state: "uncertain",
          suggestedFinish: undefined,
          uncertaintyReason: reason,
        },
      };
      expect(finishPresentation(unresolved).label).toBe("Uncertain");
      expect(finishPresentation(unresolved).detail).toContain(explanation);
      expect(
        finishPresentation({ ...unresolved, finish: "normal" }).state,
      ).toBe("reviewed");
    }
  });
});

describe("edition price isolation", () => {
  it("shows edition provenance in the card panel while suppressing a stale ordinary price", () => {
    const panel = (price: CardValuation) =>
      renderToStaticMarkup(
        createElement(CardSummary, {
          label: "C-002",
          identification: identity,
          pending: false,
          incomplete: false,
          openMarks: () => undefined,
          price,
        }),
      );
    const stale = panel({ ...quote, editionKey: undefined });
    expect(stale).toContain("Trick or Trade");
    expect(stale).toContain("Paldea Evolved");
    expect(stale).toContain("097");
    expect(stale).not.toContain("£0.12");
    expect(panel(quote)).toContain("£0.12");
  });
  it("rejects an ordinary or different edition quote for the same base card", () => {
    expect(matchingCardPrice(quote, "sv02-097", "edition-fixture")).toBe(quote);
    expect(
      matchingCardPrice(
        { ...quote, editionKey: undefined },
        "sv02-097",
        "edition-fixture",
      ),
    ).toBeUndefined();
    expect(matchingCardPrice(quote, "sv02-097", "new-edition")).toBeUndefined();
    expect(
      matchingCardPrice(quote, "another-card", "edition-fixture"),
    ).toBeUndefined();
    expect(
      matchingCardPrice(
        { ...quote, cardID: undefined },
        undefined,
        "edition-fixture",
      ),
    ).toBeUndefined();
  });
  it("invalidates valuation when only stamp or edition evidence changes", () => {
    const ordinary = {
      ...identity,
      finishEstimate: { ...identity.finishEstimate!, edition: undefined },
    };
    const revision = (value: Identification) =>
      pricingRevision({ images: [], cardIdentifications: [value] });
    expect(revision(identity)).not.toBe(revision(ordinary));
    expect(revision(identity)).not.toBe(
      revision(withEdition({ ...edition, state: "unlisted" })),
    );
    expect(revision(identity)).not.toBe(
      revision(
        withEdition({
          ...edition,
          variants: [{ ...edition.variants[0], id: "another-printing" }],
        }),
      ),
    );
  });
});
