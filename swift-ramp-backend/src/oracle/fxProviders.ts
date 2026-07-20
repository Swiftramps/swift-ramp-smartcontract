/**
 * fxProviders.ts
 *
 * Three independent FX-rate providers used by the oracle's fallback chain.
 * The chain tries PRIMARY → FALLBACK_1 → FALLBACK_2; a currency is skipped
 * only when every provider fails for that pair.
 *
 * All providers return rates as plain `number` (human-readable float).
 * The caller is responsible for scaling via `scaleRate()` before pushing
 * to the contract.
 *
 * Provider summary
 * ─────────────────────────────────────────────────────────────────────────────
 * PRIMARY      open.er-api.com  — free, no API key, well-known
 * FALLBACK_1   frankfurter.app  — open-source ECB mirror, no API key
 * FALLBACK_2   cdn.jsdelivr.net — static JSON mirror of exchangerate-api,
 *                                 no API key, serves via CDN
 */

export interface FxRate {
  from: string;
  to: string;
  rate: number;
  /** ISO-8601 timestamp from the provider, if available */
  providerTimestamp?: string;
  /** Which provider actually supplied this rate */
  provider: string;
}

export interface FxProvider {
  name: string;
  /**
   * Fetch the rate for one currency pair.
   * Returns null if the provider cannot supply this pair.
   * Throws on network/parse errors.
   */
  fetchRate(from: string, to: string): Promise<number | null>;
}

// ── helpers ───────────────────────────────────────────────────────────────────

const FETCH_TIMEOUT_MS = 8_000;

async function fetchJson(url: string): Promise<unknown> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(url, { signal: controller.signal });
    if (!res.ok) {
      throw new Error(`HTTP ${res.status} from ${url}`);
    }
    return res.json();
  } finally {
    clearTimeout(timer);
  }
}

// ── Provider 1 — open.er-api.com (primary) ───────────────────────────────────

export class OpenErApiProvider implements FxProvider {
  readonly name = "open.er-api.com";

  async fetchRate(from: string, to: string): Promise<number | null> {
    // Returns all rates relative to `from`.
    const data = (await fetchJson(
      `https://open.er-api.com/v6/latest/${from.toUpperCase()}`
    )) as { result: string; rates?: Record<string, number> };

    if (data.result !== "success" || !data.rates) return null;
    const rate = data.rates[to.toUpperCase()];
    return rate ?? null;
  }
}

// ── Provider 2 — frankfurter.app (fallback 1) ────────────────────────────────

export class FrankfurterProvider implements FxProvider {
  readonly name = "frankfurter.app";

  async fetchRate(from: string, to: string): Promise<number | null> {
    const data = (await fetchJson(
      `https://api.frankfurter.app/latest?from=${from.toUpperCase()}&to=${to.toUpperCase()}`
    )) as { rates?: Record<string, number> };

    if (!data.rates) return null;
    const rate = data.rates[to.toUpperCase()];
    return rate ?? null;
  }
}

// ── Provider 3 — jsdelivr exchangerate-api mirror (fallback 2) ───────────────
//
// jsdelivr serves a static copy of the exchangerate-api.com free tier at:
//   https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/{from}.json
// The file contains a flat object of all destination rates keyed by lowercase
// currency code.

export class JsdelivrCurrencyProvider implements FxProvider {
  readonly name = "cdn.jsdelivr.net/currency-api";

  async fetchRate(from: string, to: string): Promise<number | null> {
    const f = from.toLowerCase();
    const t = to.toLowerCase();
    const data = (await fetchJson(
      `https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/${f}.json`
    )) as Record<string, Record<string, number> | string>;

    const rates = data[f];
    if (!rates || typeof rates !== "object") return null;
    const rate = (rates as Record<string, number>)[t];
    return rate ?? null;
  }
}

// ── Fallback chain ────────────────────────────────────────────────────────────

/** Result of a fallback-chain fetch for a single currency pair */
export interface ChainResult {
  from: string;
  to: string;
  rate: number;
  provider: string;
}

/**
 * Try each provider in order.  Return the first successful rate.
 * Returns null (and records per-provider errors) if every provider fails.
 */
export async function fetchWithFallback(
  from: string,
  to: string,
  providers: FxProvider[],
  onProviderError?: (provider: string, error: Error) => void
): Promise<ChainResult | null> {
  for (const provider of providers) {
    try {
      const rate = await provider.fetchRate(from, to);
      if (rate !== null && rate > 0) {
        return { from, to, rate, provider: provider.name };
      }
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      onProviderError?.(provider.name, error);
    }
  }
  return null;
}

/** Default ordered provider chain: primary → fallback1 → fallback2 */
export function defaultProviderChain(): FxProvider[] {
  return [
    new OpenErApiProvider(),
    new FrankfurterProvider(),
    new JsdelivrCurrencyProvider(),
  ];
}
