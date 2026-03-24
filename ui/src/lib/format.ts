import type { TokenUsage } from "../types";

export function totalTokens(usage: TokenUsage): number {
  return usage.input + usage.output + usage.cache_read + usage.cache_write;
}

export function formatTokens(total: number): string {
  if (total === 0) return "-";
  if (total > 1_000_000) return `${(total / 1_000_000).toFixed(1)}M`;
  if (total > 1_000) return `${(total / 1_000).toFixed(1)}k`;
  return `${total}`;
}

export function estimateCost(
  usage: TokenUsage,
  model: string,
): number {
  const perM = 1_000_000;
  let ip = 3, op = 15, crp = 0.3, cwp = 3.75;

  if (model.includes("opus")) { ip = 15; op = 75; crp = 1.5; cwp = 18.75; }
  else if (model.includes("haiku")) { ip = 0.8; op = 4; crp = 0.08; cwp = 1; }
  else if (model.includes("gpt-4o")) { ip = 2.5; op = 10; crp = 1.25; cwp = 2.5; }
  else if (model.includes("gpt-4")) { ip = 10; op = 30; crp = 5; cwp = 10; }
  else if (/o[134]/.test(model)) { ip = 10; op = 40; crp = 2.5; cwp = 10; }

  return (
    (usage.input * ip + usage.output * op + usage.cache_read * crp + usage.cache_write * cwp) / perM
  );
}

export function formatCost(cost: number): string {
  if (cost === 0) return "";
  if (cost < 0.01) return `$${cost.toFixed(4)}`;
  return `$${cost.toFixed(2)}`;
}
