import { z } from "zod";
import { invoke } from "@tauri-apps/api/core";

const portfolioSummarySchema = z.object({
  reporting_currency: z.string().regex(/^[A-Z]{3}$/),
  account_count: z.number().int().nonnegative(),
  import_count: z.number().int().nonnegative(),
  data_status: z.enum(["awaiting_imports", "partial", "current"]),
});

export type PortfolioSummary = z.infer<typeof portfolioSummarySchema>;

const accountSchema = z.object({
  id: z.string().uuid(),
  broker: z.enum(["trading_212", "ibkr"]),
  account_type: z.enum(["invest", "stocks_and_shares_isa"]),
  display_name: z.string(),
  base_currency: z.string().regex(/^[A-Z]{3}$/),
});

const importResultSchema = z.object({
  batch_id: z.string().uuid(),
  coverage_start: z.string(),
  coverage_end: z.string(),
  events_added: z.number().int().nonnegative(),
  warnings: z.array(z.string()),
});

export type Account = z.infer<typeof accountSchema>;
export type Broker = Account["broker"];
export type AccountType = Account["account_type"];
export type ImportResult = z.infer<typeof importResultSchema>;

const appSettingsSchema = z.object({
  reporting_currency: z.string().regex(/^[A-Z]{3}$/).nullable(),
  onboarding_complete: z.boolean(),
  ai_onboarding_complete: z.boolean(),
  ai_runtime: z.string().nullable(),
  ai_model: z.string().nullable(),
  ai_endpoint: z.string().url().nullable(),
});

const aiRecommendationSchema = z.object({
  runtime: z.enum(["rapid-mlx", "ollama"]),
  runtime_name: z.string().min(1),
  model: z.string().min(1),
  endpoint: z.string().url(),
  rationale: z.string().min(1),
  installed: z.boolean(),
  supported: z.boolean(),
});

const currencyOptionSchema = z.object({
  code: z.string().regex(/^[A-Z]{3}$/),
  name: z.string().min(1),
  symbol: z.string().min(1),
});

export type AppSettings = z.infer<typeof appSettingsSchema>;
export type CurrencyOption = z.infer<typeof currencyOptionSchema>;
export type AiRecommendation = z.infer<typeof aiRecommendationSchema>;

const exactString = z.string().regex(/^-?\d+(?:\.\d+)?$/);
const activityEventSchema = z.object({
  id: z.string().uuid(), account_id: z.string().uuid(), account_name: z.string(),
  broker: z.enum(["trading_212", "ibkr"]), event_type: z.string(), occurred_at: z.string(),
  description: z.string(), amount: exactString.nullable(), currency: z.string().nullable(),
  quantity: exactString.nullable(), instrument_id: z.string().nullable(),
});
const holdingSchema = z.object({
  account_id: z.string().uuid(), account_name: z.string(), broker: z.enum(["trading_212", "ibkr"]),
  instrument_id: z.string(), symbol: z.string().nullable(), name: z.string().nullable(),
  quantity: exactString, cost_basis: exactString.nullable(),
  average_cost: exactString.nullable(), currency: z.string().nullable(), cost_basis_complete: z.boolean(),
});
const incomeSummarySchema = z.object({
  currency: z.string(), dividends: exactString, interest: exactString, total: exactString,
});
const priceQuoteSchema = z.object({
  instrument_id: z.string(), price: exactString, currency: z.string(), as_of: z.string(), source: z.string(), stale: z.boolean(),
});
const valuationSummarySchema = z.object({
  reporting_currency: z.string(), total_value: exactString.nullable(),
  missing_price_count: z.number().int().nonnegative(), missing_fx_count: z.number().int().nonnegative(),
  stale_price_count: z.number().int().nonnegative(), stale_fx_count: z.number().int().nonnegative(),
  total_gain_loss: exactString.nullable(),
  holdings: z.array(z.object({
    holding: holdingSchema, price: priceQuoteSchema.nullable(), market_value: exactString.nullable(),
    reporting_value: exactString.nullable(), reporting_currency: z.string(),
    reporting_cost_basis: exactString.nullable(), gain_loss: exactString.nullable(),
  })),
});
const portfolioSnapshotSchema = z.object({
  id: z.string().uuid(), captured_at: z.string(), reporting_currency: z.string(), total_value: exactString,
});
const allocationReportSchema = z.object({
  reporting_currency: z.string(),
  by_account: z.array(z.object({ label: z.string(), value: exactString, percentage: exactString })),
  by_currency: z.array(z.object({ label: z.string(), value: exactString, percentage: exactString })),
});
const reconciliationItemSchema = z.object({
  account_id: z.string().uuid(), account_name: z.string(), instrument_id: z.string(),
  as_of: z.string().nullable(), ledger_quantity: exactString,
  broker_quantity: exactString.nullable(), difference: exactString.nullable(),
  status: z.enum(["matched", "mismatch", "unavailable"]),
});

export type ActivityEvent = z.infer<typeof activityEventSchema>;
export type Holding = z.infer<typeof holdingSchema>;
export type IncomeSummary = z.infer<typeof incomeSummarySchema>;
export type ValuationSummary = z.infer<typeof valuationSummarySchema>;
export type PortfolioSnapshot = z.infer<typeof portfolioSnapshotSchema>;
export type AllocationReport = z.infer<typeof allocationReportSchema>;
export type ReconciliationItem = z.infer<typeof reconciliationItemSchema>;

export async function getPortfolioSummary(signal?: AbortSignal): Promise<PortfolioSummary> {
  signal?.throwIfAborted();
  return portfolioSummarySchema.parse(await invoke("portfolio_summary"));
}

export async function getAccounts(signal?: AbortSignal): Promise<Account[]> {
  signal?.throwIfAborted();
  return z.array(accountSchema).parse(await invoke("list_accounts"));
}

export async function getSettings(signal?: AbortSignal): Promise<AppSettings> {
  signal?.throwIfAborted();
  return appSettingsSchema.parse(await invoke("get_settings"));
}

export async function getCurrencies(signal?: AbortSignal): Promise<CurrencyOption[]> {
  signal?.throwIfAborted();
  return z.array(currencyOptionSchema).parse(await invoke("list_currencies"));
}

export async function updateSettings(reportingCurrency: string): Promise<AppSettings> {
  return appSettingsSchema.parse(await invoke("update_settings", {
    input: { reporting_currency: reportingCurrency },
  }));
}

export async function getAiRecommendation(signal?: AbortSignal): Promise<AiRecommendation> {
  signal?.throwIfAborted();
  return aiRecommendationSchema.parse(await invoke("ai_recommendation"));
}

export async function setupRecommendedAi(): Promise<AppSettings> {
  return appSettingsSchema.parse(await invoke("setup_recommended_ai"));
}

export async function skipAiSetup(): Promise<AppSettings> {
  return appSettingsSchema.parse(await invoke("skip_ai_setup"));
}

export async function getActivity(signal?: AbortSignal): Promise<ActivityEvent[]> {
  signal?.throwIfAborted();
  return z.array(activityEventSchema).parse(await invoke("list_activity", { limit: 250 }));
}

export async function getHoldings(signal?: AbortSignal): Promise<Holding[]> {
  signal?.throwIfAborted();
  return z.array(holdingSchema).parse(await invoke("list_holdings"));
}

export async function getIncomeSummary(signal?: AbortSignal): Promise<IncomeSummary[]> {
  signal?.throwIfAborted();
  return z.array(incomeSummarySchema).parse(await invoke("income_summary"));
}

export async function getPortfolioValuation(signal?: AbortSignal): Promise<ValuationSummary> {
  signal?.throwIfAborted();
  return valuationSummarySchema.parse(await invoke("portfolio_valuation"));
}

export async function setMarketPrice(input: { instrument_id: string; price: string; currency: string }) {
  return priceQuoteSchema.parse(await invoke("set_market_price", { input }));
}

export async function setFxRate(input: { base_currency: string; quote_currency: string; rate: string }) {
  return invoke("set_fx_rate", { input });
}

export async function getPortfolioSnapshots(signal?: AbortSignal): Promise<PortfolioSnapshot[]> {
  signal?.throwIfAborted();
  return z.array(portfolioSnapshotSchema).parse(await invoke("list_portfolio_snapshots"));
}

export async function capturePortfolioSnapshot(): Promise<PortfolioSnapshot> {
  return portfolioSnapshotSchema.parse(await invoke("capture_portfolio_snapshot"));
}

export async function getPortfolioAllocation(signal?: AbortSignal): Promise<AllocationReport> {
  signal?.throwIfAborted();
  return allocationReportSchema.parse(await invoke("portfolio_allocation"));
}

export async function getPortfolioReconciliation(signal?: AbortSignal): Promise<ReconciliationItem[]> {
  signal?.throwIfAborted();
  return z.array(reconciliationItemSchema).parse(await invoke("portfolio_reconciliation"));
}

export async function createEncryptedBackup(path: string, password: string): Promise<void> {
  await invoke("create_encrypted_backup", { input: { path, password } });
}

export async function restoreEncryptedBackup(path: string, password: string): Promise<void> {
  await invoke("restore_encrypted_backup", { input: { path, password } });
}

export async function exportPortfolioJson(path: string): Promise<void> {
  await invoke("export_portfolio_json", { path });
}

export async function createAccount(input: {
  broker: Broker;
  account_type: AccountType;
  display_name: string;
}): Promise<Account> {
  return accountSchema.parse(await invoke("create_account", { input }));
}

export async function importBrokerFile(account: Account, filePath: string): Promise<ImportResult> {
  return importResultSchema.parse(await invoke("import_broker_file", {
    accountId: account.id,
    filePath,
    confirmedAccountType: account.account_type,
  }));
}
