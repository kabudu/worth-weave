import { expect, test } from "@playwright/test";

test("shows IBKR stock weight and excludes warrants", async ({ page }) => {
  await page.addInitScript(() => {
    const accountId = "11111111-1111-4111-8111-111111111111";
    const holding = (symbol: string, asset_class: string, name: string) => ({
      account_id: accountId, account_name: "IBKR ISA", broker: "ibkr", instrument_id: symbol,
      symbol, name, asset_class, sector: null, geography: null, quantity: "10",
      cost_basis: "50", average_cost: "5", currency: "USD", cost_basis_complete: true,
    });
    const holdings = [holding("OPEN", "STK", "Opendoor Technologies"), holding("OPENW", "WAR", "OPEN warrant"), holding("OTHER", "ETF", "Other investment")];
    const valuation = {
      reporting_currency: "USD", total_value: "1000", valuation_complete: true,
      valued_holding_count: 3, missing_price_count: 0, missing_fx_count: 0,
      stale_price_count: 0, stale_fx_count: 0, total_gain_loss: "200",
      holdings: holdings.map((item, index) => ({
        holding: item, price: { instrument_id: item.instrument_id, price: "25", currency: "USD", as_of: "2026-09-23", source: "manual", stale: false },
        market_value: ["250", "50", "700"][index], reporting_value: ["250", "50", "700"][index],
        reporting_currency: "USD", reporting_cost_basis: "50", gain_loss: index ? "0" : "200",
      })),
    };
    const invoke = async (command: string) => {
      if (command === "get_settings") return { reporting_currency: "USD", onboarding_complete: true, ai_onboarding_complete: true, ai_runtime: null, ai_model: null, ai_endpoint: null };
      if (command === "list_currencies") return [{ code: "USD", name: "US dollar", symbol: "$" }];
      if (command === "portfolio_summary") return { reporting_currency: "USD", account_count: 1, import_count: 1, data_status: "current" };
      if (command === "list_accounts") return [{ id: accountId, broker: "ibkr", jurisdiction: "US", account_type: "individual_brokerage", display_name: "IBKR ISA", base_currency: "USD" }];
      if (command === "list_holdings") return holdings;
      if (command === "portfolio_valuation") return valuation;
      if (command === "portfolio_total_return") return { reporting_currency: "USD", coverage_start: null, coverage_end: null, status: "unavailable", realized_gain_loss: null, unrealized_gain_loss: null, dividends: null, interest: null, fees: null, taxes: null, fx_impact: null, attributed_subtotal: null, total_return: null, notes: [] };
      if (command === "portfolio_allocation") return { reporting_currency: "USD", by_account: [], by_currency: [], by_platform: [], by_asset_class: [], by_sector: [], by_geography: [] };
      if (command === "portfolio_performance_history") return { reporting_currency: "USD", scope: "all", coverage: "partial", points: [] };
      if (command === "refresh_portfolio_history") return { requested: 0, updated: 0, unavailable: 0, source: "test" };
      return [];
    };
    Object.assign(window, { __TAURI_INTERNALS__: { invoke } });
  });

  await page.goto("/");
  await page.getByRole("button", { name: "Portfolio", exact: true }).click();
  const table = page.locator(".portfolio-content");
  await expect(table.getByRole("row").filter({ hasText: "OPEN" }).first()).toContainText("25.00%");
  await expect(table.getByRole("row").filter({ hasText: "OPENW" })).toContainText("\u2014");
});
