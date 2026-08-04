import { describe, expect, it } from "vitest";

import { gainLossTone } from "./gainLoss";

describe("gainLossTone", () => {
  it("marks gains as positive and losses as negative", () => {
    expect(gainLossTone("125.42")).toBe("positive");
    expect(gainLossTone("-0.01")).toBe("negative");
  });

  it("keeps zero, unavailable, and invalid values neutral", () => {
    expect(gainLossTone("0")).toBe("neutral");
    expect(gainLossTone(null)).toBe("neutral");
    expect(gainLossTone(undefined)).toBe("neutral");
    expect(gainLossTone("not-a-number")).toBe("neutral");
  });
});
