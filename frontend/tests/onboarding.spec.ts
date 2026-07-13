import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import process from "node:process";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    let settings = { reporting_currency: null as string | null, onboarding_complete: false, ai_onboarding_complete: false, ai_runtime: null, ai_model: null, ai_endpoint: null };
    const accounts: Array<{ id: string; broker?: string; jurisdiction?: string; account_type?: string; display_name?: string; base_currency: string }> = [];
    const emptyCommands = new Set(["list_holdings", "list_activity", "income_summary", "list_portfolio_snapshots", "portfolio_reconciliation"]);
    const invoke = async (command: string, args?: { input?: { reporting_currency?: string; broker?: string; jurisdiction?: string; account_type?: string; display_name?: string } }) => {
      if (command === "get_settings") return settings;
      if (command === "list_accounts") return accounts;
      if (command === "list_currencies") return [{ code: "GBP", name: "British pound", symbol: "£" }, { code: "EUR", name: "Euro", symbol: "€" }];
      if (command === "update_settings") { settings = { ...settings, reporting_currency: args?.input?.reporting_currency ?? "GBP", onboarding_complete: true }; return settings; }
      if (command === "create_account") { const account = { id: crypto.randomUUID(), broker: args?.input?.broker, jurisdiction: args?.input?.jurisdiction, account_type: args?.input?.account_type, display_name: args?.input?.display_name, base_currency: args?.input?.jurisdiction === "US" ? "USD" : "GBP" }; accounts.push(account); return account; }
      if (command === "ai_recommendation") return { runtime: "rapid-mlx", runtime_name: "Rapid-MLX", model: "gpt-oss-20b-mxfp4-q8", endpoint: "http://127.0.0.1:8000/v1", rationale: "Apple Silicon with 24 GB unified memory.", installed: false, supported: true };
      if (command === "skip_ai_setup") { settings = { ...settings, ai_onboarding_complete: true }; return settings; }
      if (emptyCommands.has(command)) return [];
      if (command === "portfolio_valuation") return { reporting_currency: "GBP", total_value: null, valuation_complete: true, valued_holding_count: 0, missing_price_count: 0, missing_fx_count: 0, stale_price_count: 0, stale_fx_count: 0, total_gain_loss: null, holdings: [] };
      if (command === "portfolio_total_return") return { reporting_currency: "GBP", coverage_start: null, coverage_end: null, status: "unavailable", realized_gain_loss: null, unrealized_gain_loss: null, dividends: null, interest: null, fees: null, taxes: null, fx_impact: null, attributed_subtotal: null, total_return: null, notes: ["Import broker history to calculate return attribution."] };
      if (command === "portfolio_allocation") return { reporting_currency: "GBP", by_account: [], by_currency: [], by_platform: [], by_asset_class: [], by_sector: [], by_geography: [] };
      return { reporting_currency: "GBP", account_count: accounts.length, import_count: 0, data_status: "awaiting_imports" };
    };
    Object.assign(window, { __TAURI_INTERNALS__: { invoke } });
  });
});

test("completes accessible first-run onboarding", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => { if (message.type() === "error") browserErrors.push(message.text()); });
  page.on("pageerror", (error) => browserErrors.push(error.message));
  await page.goto("/");
  await expect(page.getByRole("heading", { name: /bring your portfolio/i })).toBeVisible();
  await expect(page.getByLabel("Robinhood account region")).toHaveValue("GB");
  expect(await page.locator("body").evaluate((element) => getComputedStyle(element).fontFamily)).toContain("Inter Variable");
  expect(await page.getByRole("heading", { name: /bring your portfolio/i }).evaluate((element) => getComputedStyle(element).fontFamily)).toContain("Manrope Variable");
  expect(await page.evaluate(() => document.fonts.check("16px 'Inter Variable'") && document.fonts.check("16px 'Manrope Variable'"))).toBe(true);
  await page.getByLabel("Robinhood account region").selectOption("US");
  await expect(page.getByRole("checkbox", { name: /robinhood us roth ira/i })).toBeVisible();
  await page.getByLabel("Robinhood account region").selectOption("GB");
  if (process.env.CAPTURE_SCREENSHOTS) { await page.waitForTimeout(300); await page.screenshot({ path: "../.dev/screenshots/onboarding.png", fullPage: true }); }
  const firstScan = await new AxeBuilder({ page }).analyze();
  expect(firstScan.violations).toEqual([]);
  await page.getByRole("button", { name: /continue/i }).click();
  await expect(page.getByRole("heading", { name: /clear answers/i })).toBeVisible();
  await page.getByRole("button", { name: /continue without ai/i }).click();
  await expect(page.getByRole("heading", { name: /your wealth, in focus/i })).toBeVisible();
  if (process.env.CAPTURE_SCREENSHOTS) { await page.waitForTimeout(300); await page.screenshot({ path: "../.dev/screenshots/dashboard.png", fullPage: true }); }
  const dashboardScan = await new AxeBuilder({ page }).analyze();
  expect(dashboardScan.violations).toEqual([]);
  await page.getByRole("button", { name: "Insights", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Ask about your portfolio" })).toBeVisible();
  await expect(page.getByText("Not set up", { exact: true })).toBeVisible();
  await expect(page.getByText("Private AI is currently off", { exact: true })).toBeVisible();
  await expect(page.getByLabel("Your question")).toBeDisabled();
  await expect(page.getByRole("button", { name: "Portfolio balance" })).toBeDisabled();
  await page.getByRole("button", { name: /set up private ai/i }).click();
  await expect(page.getByRole("heading", { name: "Settings", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Close settings" }).click();
  await page.getByRole("button", { name: /import data/i }).click();
  await expect(page.getByRole("heading", { name: /import account history/i })).toBeVisible();
  const importLayout = await page.locator(".import-dialog").evaluate((dialog) => {
    const dialogRect = dialog.getBoundingClientRect();
    const picker = dialog.querySelector<HTMLElement>(".file-drop");
    const pickerRect = picker?.getBoundingClientRect();
    const column = picker?.parentElement;
    const columnStyle = column ? getComputedStyle(column) : null;
    return {
      centreOffsetX: Math.abs((dialogRect.left + dialogRect.right) / 2 - window.innerWidth / 2),
      centreOffsetY: Math.abs((dialogRect.top + dialogRect.bottom) / 2 - window.innerHeight / 2),
      pickerDisplay: picker ? getComputedStyle(picker).display : "",
      pickerWidth: pickerRect?.width ?? 0,
      columnContentWidth: column
        ? column.clientWidth - Number.parseFloat(columnStyle?.paddingLeft ?? "0") - Number.parseFloat(columnStyle?.paddingRight ?? "0")
        : 0,
      controlHeights: Array.from(dialog.querySelectorAll<HTMLElement>(".dialog-columns input, .dialog-columns select"))
        .map((control) => control.getBoundingClientRect().height),
      actionHeights: Array.from(dialog.querySelectorAll<HTMLElement>(".dialog-columns .primary-button, .dialog-columns .secondary-button"))
        .map((control) => control.getBoundingClientRect().height),
    };
  });
  expect(importLayout.centreOffsetX).toBeLessThanOrEqual(1);
  expect(importLayout.centreOffsetY).toBeLessThanOrEqual(1);
  expect(importLayout.pickerDisplay).toBe("grid");
  expect(Math.abs(importLayout.pickerWidth - importLayout.columnContentWidth)).toBeLessThanOrEqual(1);
  expect(new Set(importLayout.controlHeights)).toEqual(new Set([48]));
  expect(new Set(importLayout.actionHeights)).toEqual(new Set([48]));
  if (process.env.CAPTURE_SCREENSHOTS) { await page.waitForTimeout(300); await page.screenshot({ path: "../.dev/screenshots/import-dialog.png", fullPage: true }); }
  const dialogScan = await new AxeBuilder({ page }).analyze();
  expect(dialogScan.violations).toEqual([]);
  await page.getByRole("button", { name: "Close import dialog" }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Settings", exact: true })).toBeVisible();
  const settingsOffset = await page.locator(".settings-dialog").evaluate((dialog) => {
    const rect = dialog.getBoundingClientRect();
    return Math.max(
      Math.abs((rect.left + rect.right) / 2 - window.innerWidth / 2),
      Math.abs((rect.top + rect.bottom) / 2 - window.innerHeight / 2),
    );
  });
  expect(settingsOffset).toBeLessThanOrEqual(1);
  await expect(page.getByRole("switch", { name: /restoring replaces all current portfolio data/i })).not.toBeChecked();
  await expect(page.getByRole("button", { name: "Restore backup" })).toBeDisabled();
  const settingsControls = await page.locator(".settings-dialog").evaluate((dialog) => ({
    currencyHeight: dialog.querySelector<HTMLElement>(".currency-select-wrap")?.getBoundingClientRect().height ?? 0,
    passwordHeight: dialog.querySelector<HTMLInputElement>(".backup-settings input[type=password]")?.getBoundingClientRect().height ?? 0,
    actionHeights: Array.from(dialog.querySelectorAll<HTMLElement>(".backup-actions button")).map((button) => button.getBoundingClientRect().height),
  }));
  expect(settingsControls.currencyHeight).toBe(48);
  expect(settingsControls.passwordHeight).toBe(48);
  expect(new Set(settingsControls.actionHeights)).toEqual(new Set([48]));
  if (process.env.CAPTURE_SCREENSHOTS) { await page.waitForTimeout(300); await page.screenshot({ path: "../.dev/screenshots/settings-dialog.png", fullPage: true }); }
  const settingsScan = await new AxeBuilder({ page }).analyze();
  expect(settingsScan.violations).toEqual([]);
  await page.getByRole("button", { name: "Close settings" }).click();
  await page.getByRole("button", { name: "Portfolio", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Your investments" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "What changed your return" })).toBeVisible();
  await page.getByRole("button", { name: "Update market data" }).click();
  await expect(page.getByRole("heading", { name: /prices, exchange rates/i })).toBeVisible();
  if (process.env.CAPTURE_SCREENSHOTS) { await page.waitForTimeout(300); await page.screenshot({ path: "../.dev/screenshots/market-dialog.png", fullPage: true }); }
  await page.getByRole("button", { name: "Close market data" }).click();
  const portfolioScan = await new AxeBuilder({ page }).analyze();
  expect(portfolioScan.violations).toEqual([]);
  expect(browserErrors).toEqual([]);
});
