import type { Holding, ValuationSummary } from "./api";

export function portfolioWeight(holding: Holding, reportingValue: string | null | undefined, valuation?: ValuationSummary): string {
  if (holding.asset_class?.trim().toLowerCase() !== "equity" || !valuation?.valuation_complete || !reportingValue || !valuation.total_value) return "—";
  const total = Number(valuation.total_value);
  const value = Number(reportingValue);
  return Number.isFinite(total) && total > 0 && Number.isFinite(value)
    ? `${(value / total * 100).toFixed(2)}%`
    : "—";
}

