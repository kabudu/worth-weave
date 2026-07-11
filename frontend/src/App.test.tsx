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

function mockNativeCommands(onboardingComplete: boolean) {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (["list_accounts", "list_holdings", "list_activity", "income_summary", "list_portfolio_snapshots"].includes(command)) return [];
    if (command === "list_currencies") return currencies;
    if (command === "get_settings") return {
      reporting_currency: onboardingComplete ? "GBP" : null,
      onboarding_complete: onboardingComplete,
    };
    if (command === "portfolio_valuation") return {
      reporting_currency: "GBP", total_value: null, missing_price_count: 0,
      missing_fx_count: 0, holdings: [],
    };
    if (command === "portfolio_allocation") return { reporting_currency: "GBP", by_account: [], by_currency: [] };
    if (command === "update_settings") return {
      reporting_currency: "GBP",
      onboarding_complete: true,
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
  expect(screen.getByRole("button", { name: /available after reconciliation/i })).toBeDisabled();

  fireEvent.click(screen.getByRole("button", { name: /import data/i }));
  expect(screen.getByRole("heading", { name: /add portfolio data/i })).toBeInTheDocument();
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

  expect(await screen.findByRole("heading", { name: /make every number/i })).toBeInTheDocument();
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("portfolio_summary"));
  const currencySelect = screen.getByLabelText("Reporting currency");
  fireEvent.change(currencySelect, { target: { value: "EUR" } });
  expect(currencySelect).toHaveValue("EUR");
  fireEvent.click(screen.getByRole("button", { name: /enter worthweave/i }));

  await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_settings", {
    input: { reporting_currency: "EUR" },
  }));
});
