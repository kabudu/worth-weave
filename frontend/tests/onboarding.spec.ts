import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    let settings = { reporting_currency: null as string | null, onboarding_complete: false, ai_onboarding_complete: false, ai_runtime: null, ai_model: null, ai_endpoint: null };
    const emptyCommands = new Set(["list_accounts", "list_holdings", "list_activity", "income_summary", "list_portfolio_snapshots", "portfolio_reconciliation"]);
    const invoke = async (command: string, args?: { input?: { reporting_currency?: string } }) => {
      if (command === "get_settings") return settings;
      if (command === "list_currencies") return [{ code: "GBP", name: "British pound", symbol: "£" }, { code: "EUR", name: "Euro", symbol: "€" }];
      if (command === "update_settings") { settings = { ...settings, reporting_currency: args?.input?.reporting_currency ?? "GBP", onboarding_complete: true }; return settings; }
      if (command === "ai_recommendation") return { runtime: "rapid-mlx", runtime_name: "Rapid-MLX", model: "gpt-oss-20b-mxfp4-q8", endpoint: "http://127.0.0.1:8000/v1", rationale: "Apple Silicon with 24 GB unified memory.", installed: false, supported: true };
      if (command === "skip_ai_setup") { settings = { ...settings, ai_onboarding_complete: true }; return settings; }
      if (emptyCommands.has(command)) return [];
      if (command === "portfolio_valuation") return { reporting_currency: "GBP", total_value: null, missing_price_count: 0, missing_fx_count: 0, stale_price_count: 0, stale_fx_count: 0, total_gain_loss: null, holdings: [] };
      if (command === "portfolio_allocation") return { reporting_currency: "GBP", by_account: [], by_currency: [], by_platform: [], by_asset_class: [], by_sector: [], by_geography: [] };
      return { reporting_currency: "GBP", account_count: 0, import_count: 0, data_status: "awaiting_imports" };
    };
    Object.assign(window, { __TAURI_INTERNALS__: { invoke } });
  });
});

test("completes accessible first-run onboarding", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => { if (message.type() === "error") browserErrors.push(message.text()); });
  page.on("pageerror", (error) => browserErrors.push(error.message));
  await page.goto("/");
  await expect(page.getByRole("heading", { name: /make every number/i })).toBeVisible();
  const firstScan = await new AxeBuilder({ page }).analyze();
  expect(firstScan.violations).toEqual([]);
  await page.getByLabel("Reporting currency").selectOption("EUR");
  await page.getByRole("button", { name: /continue/i }).click();
  await expect(page.getByRole("heading", { name: /private insight/i })).toBeVisible();
  await page.getByRole("button", { name: /continue without ai/i }).click();
  await expect(page.getByRole("heading", { name: /your wealth, in focus/i })).toBeVisible();
  const dashboardScan = await new AxeBuilder({ page }).analyze();
  expect(dashboardScan.violations).toEqual([]);
  await page.getByRole("button", { name: "Insights", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Portfolio insights" })).toBeVisible();
  await page.getByRole("button", { name: /import data/i }).click();
  await expect(page.getByRole("heading", { name: /add portfolio data/i })).toBeVisible();
  const dialogScan = await new AxeBuilder({ page }).analyze();
  expect(dialogScan.violations).toEqual([]);
  await page.getByRole("button", { name: "Close import dialog" }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Settings", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Restore backup" })).toBeDisabled();
  const settingsScan = await new AxeBuilder({ page }).analyze();
  expect(settingsScan.violations).toEqual([]);
  await page.getByRole("button", { name: "Close settings" }).click();
  await page.getByRole("button", { name: "Portfolio", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Your holdings" })).toBeVisible();
  const portfolioScan = await new AxeBuilder({ page }).analyze();
  expect(portfolioScan.violations).toEqual([]);
  expect(browserErrors).toEqual([]);
});
