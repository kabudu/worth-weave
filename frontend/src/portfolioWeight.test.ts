import { describe, expect, it } from "vitest";

import type { Holding, ValuationSummary } from "./api";
import { portfolioWeight } from "./portfolioWeight";

const equity = { asset_class: "Equity" } as Holding;
const complete = { valuation_complete: true, total_value: "1000" } as ValuationSummary;

describe("portfolioWeight", () => {
  it("uses the full portfolio value for equity positions", () => {
    expect(portfolioWeight(equity, "125", complete)).toBe("12.50%");
  });

  it("omits non-equities and incomplete valuations", () => {
    expect(portfolioWeight({ asset_class: "Warrant" } as Holding, "125", complete)).toBe("—");
    expect(portfolioWeight(equity, "125", { ...complete, valuation_complete: false })).toBe("—");
    expect(portfolioWeight(equity, null, complete)).toBe("—");
    expect(portfolioWeight(equity, "125", { ...complete, total_value: "0" })).toBe("—");
  });
});
