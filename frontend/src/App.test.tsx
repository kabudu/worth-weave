import "@testing-library/jest-dom/vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { App } from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => command === "list_accounts" ? [] : {
    base_currency: "GBP",
    account_count: 0,
    import_count: 0,
    data_status: "awaiting_imports",
  }),
}));

afterEach(() => vi.restoreAllMocks());

test("renders truthful empty portfolio state", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

  render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );

  expect(screen.getByRole("heading", { name: /your wealth, in focus/i })).toBeInTheDocument();
  expect(await screen.findByText("Awaiting data")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /available after reconciliation/i })).toBeDisabled();

  fireEvent.click(screen.getByRole("button", { name: /import data/i }));
  expect(screen.getByRole("heading", { name: /add portfolio data/i })).toBeInTheDocument();
  expect(screen.getByText(/broker credentials are never required/i)).toBeInTheDocument();
});
