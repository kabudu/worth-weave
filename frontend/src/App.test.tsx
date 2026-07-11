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
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (["list_accounts", "list_holdings", "list_activity", "income_summary", "list_portfolio_snapshots", "portfolio_reconciliation"].includes(command)) return [];
    if (command === "list_currencies") return currencies;
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
    if (command === "portfolio_allocation") return { reporting_currency: "GBP", by_account: [], by_currency: [] };
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
  fireEvent.click(screen.getByRole("button", { name: /continue/i }));

  await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_settings", {
    input: { reporting_currency: "EUR" },
  }));
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
