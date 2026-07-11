import "@testing-library/jest-dom/vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";

import { App } from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const currencies = [
  { code: "GBP", name: "British pound", symbol: "£" },
  { code: "EUR", name: "Euro", symbol: "€" },
];

function mockNativeCommands(onboardingComplete: boolean, aiOnboardingComplete = true) {
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (["list_accounts", "list_holdings", "list_activity", "income_summary", "list_portfolio_snapshots", "portfolio_reconciliation"].includes(command)) return [];
    if (command === "list_currencies") return currencies;
    if (command === "create_account") {
      const input = (args as { input: { broker: string; jurisdiction: "GB" | "US"; account_type: string; display_name: string } }).input;
      return { id: crypto.randomUUID(), base_currency: input.jurisdiction === "US" ? "USD" : "GBP", ...input };
    }
    if (command === "get_settings") return {
      reporting_currency: onboardingComplete ? "GBP" : null,
      onboarding_complete: onboardingComplete,
      ai_onboarding_complete: aiOnboardingComplete,
      ai_runtime: null, ai_model: null, ai_endpoint: null,
    };
    if (command === "portfolio_valuation") return {
      reporting_currency: "GBP", total_value: null, missing_price_count: 0,
      missing_fx_count: 0, stale_price_count: 0, stale_fx_count: 0,
      total_gain_loss: null, holdings: [],
    };
    if (command === "portfolio_total_return") return {
      reporting_currency: "GBP", coverage_start: null, coverage_end: null, status: "unavailable",
      realized_gain_loss: null, unrealized_gain_loss: null, dividends: null, interest: null,
      fees: null, taxes: null, fx_impact: null, attributed_subtotal: null, total_return: null,
      notes: ["Import broker history to calculate return attribution."],
    };
    if (command === "portfolio_allocation") return { reporting_currency: "GBP", by_account: [], by_currency: [], by_platform: [], by_asset_class: [], by_sector: [], by_geography: [] };
    if (command === "update_settings") return {
      reporting_currency: "GBP",
      onboarding_complete: true,
      ai_onboarding_complete: false,
      ai_runtime: null, ai_model: null, ai_endpoint: null,
    };
    return {
      reporting_currency: "GBP",
      account_count: 0,
      import_count: 0,
      data_status: "awaiting_imports",
    };
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

test("renders truthful empty portfolio state", async () => {
  mockNativeCommands(true);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

  render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );

  expect(await screen.findByRole("heading", { name: /your wealth, in focus/i })).toBeInTheDocument();
  expect(await screen.findByText("Awaiting data")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /set up local ai in settings/i })).toBeDisabled();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "list_holdings")).toBe(false);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "list_accounts")).toBe(false);

  fireEvent.click(screen.getByRole("button", { name: "Portfolio" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_holdings"));
  expect(await screen.findByRole("heading", { name: /what shaped your return/i })).toBeInTheDocument();
  expect(screen.getByText(/import broker history to calculate return attribution/i)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Overview" }));

  fireEvent.click(screen.getByRole("button", { name: /import data/i }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_accounts"));
  expect(screen.getByRole("heading", { name: /import broker data/i })).toBeInTheDocument();
  expect(screen.getByText(/broker credentials are never required/i)).toBeInTheDocument();
});

test("requires reporting currency during first-run onboarding", async () => {
  mockNativeCommands(false);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

  render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );

  expect(await screen.findByRole("heading", { name: /bring your portfolio/i })).toBeInTheDocument();
  expect(screen.getByRole("checkbox", { name: /trading 212 stocks and shares isa/i })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: /trading 212 invest/i })).not.toBeChecked();
  expect(screen.getByLabelText("Robinhood account region")).toHaveValue("GB");
  expect(screen.getByRole("checkbox", { name: /robinhood gb individual brokerage/i })).not.toBeChecked();
  fireEvent.change(screen.getByLabelText("Robinhood account region"), { target: { value: "US" } });
  expect(screen.getByRole("checkbox", { name: /robinhood us roth ira/i })).not.toBeChecked();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "portfolio_summary")).toBe(false);
  const currencySelect = screen.getByLabelText("Reporting currency");
  fireEvent.change(currencySelect, { target: { value: "EUR" } });
  expect(currencySelect).toHaveValue("EUR");
  fireEvent.click(screen.getByRole("button", { name: /continue/i }));

  await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_settings", {
    input: { reporting_currency: "EUR" },
  }));
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "create_account")).toHaveLength(3);
});

test("offers explicit device-tuned local AI setup or skip", async () => {
  mockNativeCommands(true, false);
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "get_settings") return { reporting_currency: "GBP", onboarding_complete: true, ai_onboarding_complete: false, ai_runtime: null, ai_model: null, ai_endpoint: null };
    if (command === "list_currencies") return currencies;
    if (command === "ai_recommendation") return { runtime: "rapid-mlx", runtime_name: "Rapid-MLX", model: "gpt-oss-20b-mxfp4-q8", endpoint: "http://127.0.0.1:8000/v1", rationale: "Apple Silicon with 24 GB unified memory.", installed: false, supported: true };
    return [];
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  expect(await screen.findByRole("heading", { name: /private insight/i })).toBeInTheDocument();
  expect(await screen.findByText("gpt-oss-20b-mxfp4-q8")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /set up recommended ai/i })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: /continue without ai/i }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("skip_ai_setup"));
});
