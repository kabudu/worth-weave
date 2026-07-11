import { z } from "zod";

const portfolioSummarySchema = z.object({
  base_currency: z.literal("GBP"),
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
  base_currency: z.literal("GBP"),
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

export async function getPortfolioSummary(signal?: AbortSignal): Promise<PortfolioSummary> {
  const response = await fetch("/api/v1/portfolio/summary", { signal });
  if (!response.ok) {
    throw new Error("The local portfolio service is unavailable.");
  }
  return portfolioSummarySchema.parse(await response.json());
}

export async function getAccounts(signal?: AbortSignal): Promise<Account[]> {
  const response = await fetch("/api/v1/accounts", { signal });
  if (!response.ok) throw new Error("Accounts could not be loaded.");
  return z.array(accountSchema).parse(await response.json());
}

export async function createAccount(input: {
  broker: Broker;
  account_type: AccountType;
  display_name: string;
}): Promise<Account> {
  const response = await fetch("/api/v1/accounts", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      ...input,
      external_id: `${input.broker}:${input.account_type}:${crypto.randomUUID()}`,
    }),
  });
  if (!response.ok) throw new Error("The account could not be created.");
  return accountSchema.parse(await response.json());
}

export async function importBrokerFile(account: Account, file: File): Promise<ImportResult> {
  const form = new FormData();
  form.set("confirmed_account_type", account.account_type);
  form.set("file", file);
  const response = await fetch(`/api/v1/accounts/${account.id}/imports`, {
    method: "POST",
    body: form,
  });
  if (!response.ok) {
    const detail = z.object({ detail: z.string() }).safeParse(await response.json());
    throw new Error(detail.success ? detail.data.detail : "The broker file could not be imported.");
  }
  return importResultSchema.parse(await response.json());
}
