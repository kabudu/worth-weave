import { describe, expect, it } from "vitest";

import type { Holding, ValuationSummary } from "./api";
import { portfolioWeight } from "./portfolioWeight";

const equity = { asset_class: "Equity" } as Holding;
const complete = { valuation_complete: true, total_value: "1000" } as ValuationSummary;

describe("portfolioWeight", () => {
  it("uses the full portfolio value for equity positions", () => {
    expect(portfolioWeight(equity, "125", complete)).toBe("12.50%");
    expect(portfolioWeight({ asset_class: "STK", symbol: "OPEN" } as Holding, "250", complete)).toBe("25.00%");
  });

  it("omits non-equities and incomplete valuations", () => {
    expect(portfolioWeight({ asset_class: "Warrant" } as Holding, "125", complete)).toBe("—");
    expect(portfolioWeight({ asset_class: "WAR", symbol: "OPENW" } as Holding, "125", complete)).toBe("\u2014");
    expect(portfolioWeight(equity, "125", { ...complete, valuation_complete: false })).toBe("—");
    expect(portfolioWeight(equity, null, complete)).toBe("—");
    expect(portfolioWeight(equity, "125", { ...complete, total_value: "0" })).toBe("—");
  });
});
