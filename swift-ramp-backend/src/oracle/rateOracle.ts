/**
 * rateOracle.ts
 *
 * Fetches FX rates via a multi-provider fallback chain and pushes them to the
 * SwiftRamp swap contract.  After every successful push cycle an on-chain
 * heartbeat is recorded so off-chain monitors and the health endpoint can
 * detect oracle silence.
 *
 * Fallback chain (in order):
 *   PRIMARY   → open.er-api.com
 *   FALLBACK1 → frankfurter.app
 *   FALLBACK2 → cdn.jsdelivr.net/currency-api
 *
 * A currency pair is skipped only when every provider fails for that pair.
 * Per-provider error counts and per-pair status are tracked in `OracleMetrics`
 * and exposed via the health server.
 */

import {
  Contract,
  Keypair,
  Networks,
  SorobanRpc,
  TransactionBuilder,
  nativeToScVal,
} from "@stellar/stellar-sdk";
import {
  defaultProviderChain,
  fetchWithFallback,
  FxProvider,
} from "./fxProviders";

// ── types ─────────────────────────────────────────────────────────────────────

export interface OracleConfig {
  /** Stellar RPC endpoint */
  rpcUrl: string;
  /** Stellar network passphrase */
  networkPassphrase: string;
  /** Deployed SwiftRamp swap contract ID */
  contractId: string;
  /** Admin keypair (signs set_rate + oracle_heartbeat transactions) */
  adminKeypair: Keypair;
  /** How often to push rates, in seconds (default 300) */
  intervalSecs: number;
  /**
   * Maximum rate age the contract will accept, in seconds (default 3 600).
   * The oracle must complete at least one push cycle per this window.
   */
  maxAgeSecs: number;
  /** Currency pairs to maintain, e.g. [["USD","NGN"],["USD","KES"]] */
  currencyPairs: [string, string][];
  /**
   * Ordered provider chain.  Defaults to `defaultProviderChain()` when
   * omitted.  Pass a custom chain in tests to inject mock providers.
   */
  providers?: FxProvider[];
}

export interface RateEntry {
  from: string;
  to: string;
  /**
   * Rate multiplied by RATE_SCALE (10_000_000).
   * e.g. a real rate of 1 580.0 → 15_800_000_000n
   */
  scaledRate: bigint;
}

/** Per-pair status tracked across poll cycles */
export interface PairStatus {
  lastSuccessTs: number | null; // Unix ms
  lastError: string | null;
  errorCount: number;
  provider: string | null; // which provider last succeeded
}

/** Snapshot of oracle health metrics */
export interface OracleMetrics {
  /** Unix ms of the last poll cycle that pushed at least one rate */
  lastSuccessfulRunMs: number | null;
  /** Total push cycles completed */
  totalRuns: number;
  /** Total push cycles in which at least one pair failed */
  failedRuns: number;
  /** Per-provider consecutive error count */
  providerErrors: Record<string, number>;
  /** Per-pair status */
  pairs: Record<string, PairStatus>;
}

// ── constants ─────────────────────────────────────────────────────────────────

/** Must match RATE_SCALE in lib.rs */
export const RATE_SCALE = 10_000_000n;

/** Maximum transaction fee in stroops (0.1 XLM) */
const MAX_FEE = "1000000";

/** i128::MAX = 2^127 − 1 */
const I128_MAX = 170_141_183_460_469_231_731_687_303_715_884_105_727n;
const MAX_RATE = I128_MAX / RATE_SCALE;

// ── core class ────────────────────────────────────────────────────────────────

export class RateOracle {
  private readonly server: SorobanRpc.Server;
  private readonly contract: Contract;
  private readonly config: OracleConfig;
  private readonly providers: FxProvider[];

  /** Live metrics — read by the health server */
  readonly metrics: OracleMetrics;

  constructor(config: OracleConfig) {
    this.config = config;
    this.providers = config.providers ?? defaultProviderChain();
    this.server = new SorobanRpc.Server(config.rpcUrl, {
      allowHttp: config.rpcUrl.startsWith("http://"),
    });
    this.contract = new Contract(config.contractId);

    // Initialise metrics
    this.metrics = {
      lastSuccessfulRunMs: null,
      totalRuns: 0,
      failedRuns: 0,
      providerErrors: Object.fromEntries(
        this.providers.map((p) => [p.name, 0])
      ),
      pairs: Object.fromEntries(
        config.currencyPairs.map(([f, t]) => [
          `${f}/${t}`,
          { lastSuccessTs: null, lastError: null, errorCount: 0, provider: null },
        ])
      ),
    };
  }

  // ── public API ─────────────────────────────────────────────────────────────

  /**
   * Run one complete fetch-and-push cycle for all configured currency pairs,
   * followed by an on-chain heartbeat.
   *
   * Returns the number of pairs successfully pushed.
   */
  async runCycle(): Promise<number> {
    this.metrics.totalRuns++;
    let successCount = 0;
    let anyFailed = false;

    for (const [from, to] of this.config.currencyPairs) {
      const pairKey = `${from}/${to}`;

      const result = await fetchWithFallback(
        from,
        to,
        this.providers,
        (providerName, err) => {
          this.metrics.providerErrors[providerName] =
            (this.metrics.providerErrors[providerName] ?? 0) + 1;
          console.error(
            `[RateOracle] provider "${providerName}" failed for ${pairKey}: ${err.message}`
          );
        }
      );

      if (result === null) {
        const msg = `all providers failed for pair ${pairKey}`;
        console.error(`[RateOracle] ${msg}`);
        anyFailed = true;
        const ps = this.metrics.pairs[pairKey]!;
        ps.errorCount++;
        ps.lastError = msg;
        continue;
      }

      const scaledRate = scaleRate(result.rate);
      try {
        validateRateEntry({ from, to, scaledRate });
        await this.pushRate({ from, to, scaledRate });

        const ps = this.metrics.pairs[pairKey]!;
        ps.lastSuccessTs = Date.now();
        ps.lastError = null;
        ps.provider = result.provider;

        // Reset provider error counter on success.
        this.metrics.providerErrors[result.provider] = 0;

        successCount++;
        console.info(
          `[RateOracle] pushed ${pairKey} = ${result.rate} ` +
            `(via ${result.provider})`
        );
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error(`[RateOracle] failed to push ${pairKey}: ${msg}`);
        anyFailed = true;
        const ps = this.metrics.pairs[pairKey]!;
        ps.errorCount++;
        ps.lastError = msg;
      }
    }

    if (!anyFailed) {
      this.metrics.lastSuccessfulRunMs = Date.now();
    } else {
      this.metrics.failedRuns++;
    }

    // Always push heartbeat even if some pairs failed — the heartbeat signals
    // "oracle is alive" rather than "all rates were updated".
    if (successCount > 0) {
      try {
        await this.pushHeartbeat();
        console.info("[RateOracle] heartbeat pushed");
      } catch (err) {
        console.error("[RateOracle] heartbeat push failed:", err);
      }
    }

    return successCount;
  }

  /**
   * Start a continuous polling loop that calls `runCycle()` every
   * `intervalSecs`.  Returns a handle with `stop()` to cancel the loop.
   */
  startPolling(): { stop: () => void } {
    if (this.config.intervalSecs >= this.config.maxAgeSecs) {
      console.warn(
        `[RateOracle] WARNING: intervalSecs (${this.config.intervalSecs}) ` +
          `>= maxAgeSecs (${this.config.maxAgeSecs}). ` +
          `Rates will be stale before the next push.`
      );
    }

    let running = true;

    const loop = async (): Promise<void> => {
      while (running) {
        try {
          await this.runCycle();
        } catch (err) {
          console.error("[RateOracle] unexpected cycle error:", err);
        }
        if (!running) break;
        await sleep(this.config.intervalSecs * 1_000);
      }
    };

    loop().catch((err) =>
      console.error("[RateOracle] loop exited unexpectedly:", err)
    );

    return { stop: () => { running = false; } };
  }

  // ── contract interactions ──────────────────────────────────────────────────

  /**
   * Submit a `set_rate(from, to, rate)` transaction and wait for confirmation.
   */
  async pushRate(entry: RateEntry): Promise<string> {
    validateRateEntry(entry);

    const op = this.contract.call(
      "set_rate",
      nativeToScVal(entry.from, { type: "symbol" }),
      nativeToScVal(entry.to, { type: "symbol" }),
      nativeToScVal(entry.scaledRate, { type: "i128" })
    );
    return this.submitAndConfirm(op);
  }

  /**
   * Submit an `oracle_heartbeat()` transaction and wait for confirmation.
   */
  async pushHeartbeat(): Promise<string> {
    const op = this.contract.call("oracle_heartbeat");
    return this.submitAndConfirm(op);
  }

  // ── private helpers ────────────────────────────────────────────────────────

  private async submitAndConfirm(operation: ReturnType<Contract["call"]>): Promise<string> {
    const account = await this.server.getAccount(
      this.config.adminKeypair.publicKey()
    );

    const tx = new TransactionBuilder(account, {
      fee: MAX_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(operation)
      .setTimeout(30)
      .build();

    const simResult = await this.server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(simResult)) {
      throw new Error(`Simulation failed: ${simResult.error}`);
    }

    const preparedTx = SorobanRpc.assembleTransaction(tx, simResult).build();
    preparedTx.sign(this.config.adminKeypair);

    const sendResult = await this.server.sendTransaction(preparedTx);
    if (sendResult.status === "ERROR") {
      throw new Error(
        `Transaction submission failed: ${JSON.stringify(sendResult.errorResult)}`
      );
    }

    await this.waitForConfirmation(sendResult.hash);
    return sendResult.hash;
  }

  private async waitForConfirmation(
    txHash: string,
    maxRetries = 20,
    delayMs = 2_000
  ): Promise<void> {
    for (let i = 0; i < maxRetries; i++) {
      const status = await this.server.getTransaction(txHash);
      if (status.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) return;
      if (status.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
        throw new Error(`Transaction ${txHash} failed on-chain`);
      }
      await sleep(delayMs);
    }
    throw new Error(
      `Transaction ${txHash} not confirmed after ${maxRetries} retries`
    );
  }
}

// ── validation ────────────────────────────────────────────────────────────────

function validateRateEntry(entry: RateEntry): void {
  if (!entry.from || !entry.to) {
    throw new Error("RateEntry: from and to currency symbols are required");
  }
  if (entry.from === entry.to) {
    throw new Error(`RateEntry: from and to must differ (got "${entry.from}")`);
  }
  if (entry.scaledRate <= 0n) {
    throw new Error(
      `RateEntry: scaledRate must be positive (got ${entry.scaledRate})`
    );
  }
  if (entry.scaledRate > MAX_RATE) {
    throw new Error(
      `RateEntry: scaledRate ${entry.scaledRate} exceeds MAX_RATE ${MAX_RATE}`
    );
  }
}

// ── utilities ─────────────────────────────────────────────────────────────────

/**
 * Convert a human-readable rate (e.g. 1580.0) to the integer representation
 * stored in the contract.  Uses integer arithmetic to avoid floating-point
 * rounding errors.
 */
export function scaleRate(humanRate: number): bigint {
  return BigInt(Math.round(humanRate * Number(RATE_SCALE)));
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ── factory helpers ───────────────────────────────────────────────────────────

export function createTestnetOracle(
  contractId: string,
  adminKeypair: Keypair,
  currencyPairs: [string, string][],
  intervalSecs = 300,
  maxAgeSecs = 3_600
): RateOracle {
  return new RateOracle({
    rpcUrl: "https://soroban-testnet.stellar.org",
    networkPassphrase: Networks.TESTNET,
    contractId,
    adminKeypair,
    intervalSecs,
    maxAgeSecs,
    currencyPairs,
  });
}

export function createMainnetOracle(
  contractId: string,
  adminKeypair: Keypair,
  currencyPairs: [string, string][],
  intervalSecs = 300,
  maxAgeSecs = 3_600
): RateOracle {
  return new RateOracle({
    rpcUrl: "https://soroban.stellar.org",
    networkPassphrase: Networks.PUBLIC,
    contractId,
    adminKeypair,
    intervalSecs,
    maxAgeSecs,
    currencyPairs,
  });
}
