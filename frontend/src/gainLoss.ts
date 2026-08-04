export function gainLossTone(value: string | null | undefined) {
  if (value === null || value === undefined) return "neutral";
  const amount = Number(value);
  if (!Number.isFinite(amount) || amount === 0) return "neutral";
  return amount > 0 ? "positive" : "negative";
}
