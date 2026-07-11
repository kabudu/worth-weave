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
});

const currencyOptionSchema = z.object({
  code: z.string().regex(/^[A-Z]{3}$/),
  name: z.string().min(1),
  symbol: z.string().min(1),
});

export type AppSettings = z.infer<typeof appSettingsSchema>;
export type CurrencyOption = z.infer<typeof currencyOptionSchema>;

const exactString = z.string().regex(/^-?\d+(?:\.\d+)?$/);
const activityEventSchema = z.object({
  id: z.string().uuid(), account_id: z.string().uuid(), account_name: z.string(),
  broker: z.enum(["trading_212", "ibkr"]), event_type: z.string(), occurred_at: z.string(),
  description: z.string(), amount: exactString.nullable(), currency: z.string().nullable(),
  quantity: exactString.nullable(), instrument_id: z.string().nullable(),
});
const holdingSchema = z.object({
  account_id: z.string().uuid(), account_name: z.string(), broker: z.enum(["trading_212", "ibkr"]),
  instrument_id: z.string(), quantity: exactString, cost_basis: exactString.nullable(),
  average_cost: exactString.nullable(), currency: z.string().nullable(), cost_basis_complete: z.boolean(),
});
const incomeSummarySchema = z.object({
  currency: z.string(), dividends: exactString, interest: exactString, total: exactString,
});

export type ActivityEvent = z.infer<typeof activityEventSchema>;
export type Holding = z.infer<typeof holdingSchema>;
export type IncomeSummary = z.infer<typeof incomeSummarySchema>;

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
