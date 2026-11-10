import { describe, expect, it } from "vitest";
import { priceLabel } from "./Pricing";

describe("GBP price display", () => {
  it("shows pence consistently and keeps a real zero separate from missing", () => {
    expect(priceLabel(3, 3)).toBe("£0.03");
    expect(priceLabel(12345, 12345)).toBe("£123.45");
    expect(priceLabel(0, 0)).toBe("£0.00");
    expect(priceLabel(undefined, undefined)).toBe("—");
    expect(priceLabel(3, undefined)).toBe("—");
  });
  it("preserves an unresolved finish range", () => {
    expect(priceLabel(3, 104)).toBe("£0.03–£1.04");
  });
});
