/**
 * rateOracle.test.ts
 *
 * Unit tests for the RateOracle multi-provider fallback chain, metrics
 * tracking, and the config validation helpers.
 *
 * All network calls are intercepted by mock FxProvider implementations —
 * no real HTTP requests are made.
 */

import { RateOracle, OracleConfig, scaleRate, RATE_SCALE } from "../rateOracle";
import { FxProvider } from "../fxProviders";
import { validateAndLoadKeypair } from "../../config";
import { Keypair, Networks } from "@stellar/stellar-sdk";

// ── helpers ───────────────────────────────────────────────────────────────────

/** Build a minimal OracleConfig wired to mock providers, no real RPC. */
function buildConfig(
  providers: FxProvider[],
  pairs: [string, string][] = [["USD", "NGN"]]
): OracleConfig {
  return {
    rpcUrl: "https://fake.rpc",
    networkPassphrase: Networks.TESTNET,
    contractId: "CFAKE0000000000000000000000000000000000000000000000000000",
    // Dummy keypair — pushRate is mocked so this is never used for real signing.
    adminKeypair: Keypair.random(),
    intervalSecs: 60,
    maxAgeSecs: 300,
    currencyPairs: pairs,
    providers,
  };
}

/** A provider that always returns the given rate. */
function successProvider(name: string, rate: number): FxProvider {
  return {
    name,
    fetchRate: async (_from, _to) => rate,
  };
}

/** A provider that always throws. */
function failingProvider(name: string, message = "network error"): FxProvider {
  return {
    name,
    fetchRate: async (_from, _to) => {
      throw new Error(message);
    },
  };
}

/** A provider that returns null (currency not supported). */
function nullProvider(name: string): FxProvider {
  return {
    name,
    fetchRate: async (_from, _to) => null,
  };
}

/**
 * Patch `pushRate` and `pushHeartbeat` on an oracle instance so no real
 * Stellar transaction is submitted.  Records calls for assertion.
 */
function mockPushMethods(oracle: RateOracle): {
  pushedRates: Array<{ from: string; to: string; scaledRate: bigint }>;
  heartbeatCount: number;
} {
  const state = { pushedRates: [] as typeof state.pushedRates, heartbeatCount: 0 };

  (oracle as unknown as Record<string, unknown>).pushRate = async (
    entry: { from: string; to: string; scaledRate: bigint }
  ) => {
    state.pushedRates.push({ ...entry });
    return "mock-tx-hash";
  };

  (oracle as unknown as Record<string, unknown>).pushHeartbeat = async () => {
    state.heartbeatCount++;
    return "mock-heartbeat-hash";
  };

  return state;
}

// ── scaleRate ─────────────────────────────────────────────────────────────────

describe("scaleRate", () => {
  it("scales 1.0 to RATE_SCALE", () => {
    expect(scaleRate(1.0)).toBe(RATE_SCALE);
  });

  it("scales 1580.0 correctly", () => {
    expect(scaleRate(1580.0)).toBe(1580n * RATE_SCALE);
  });

  it("rounds sub-cent precision", () => {
    // 1.5000001 × 10_000_000 = 15_000_001
    expect(scaleRate(1.5000001)).toBe(15_000_001n);
  });
});

// ── fallback chain — single pair ──────────────────────────────────────────────

describe("RateOracle fallback chain", () => {
  it("uses primary provider when it succeeds", async () => {
    const oracle = new RateOracle(
      buildConfig([
        successProvider("primary", 1580),
        failingProvider("fallback1"),
        failingProvider("fallback2"),
      ])
    );
    const state = mockPushMethods(oracle);

    const pushed = await oracle.runCycle();

    expect(pushed).toBe(1);
    expect(state.pushedRates).toHaveLength(1);
    expect(state.pushedRates[0]!.scaledRate).toBe(scaleRate(1580));
    expect(oracle.metrics.pairs["USD/NGN"]!.provider).toBe("primary");
  });

  it("falls back to provider 2 when provider 1 fails", async () => {
    const oracle = new RateOracle(
      buildConfig([
        failingProvider("primary"),
        successProvider("fallback1", 1590),
        failingProvider("fallback2"),
      ])
    );
    const state = mockPushMethods(oracle);

    const pushed = await oracle.runCycle();

    expect(pushed).toBe(1);
    expect(state.pushedRates[0]!.scaledRate).toBe(scaleRate(1590));
    expect(oracle.metrics.pairs["USD/NGN"]!.provider).toBe("fallback1");
    // Primary error counter must be incremented.
    expect(oracle.metrics.providerErrors["primary"]).toBeGreaterThan(0);
  });

  it("falls back to provider 3 when providers 1 and 2 fail", async () => {
    const oracle = new RateOracle(
      buildConfig([
        failingProvider("primary"),
        failingProvider("fallback1"),
        successProvider("fallback2", 1600),
      ])
    );
    const state = mockPushMethods(oracle);

    const pushed = await oracle.runCycle();

    expect(pushed).toBe(1);
    expect(state.pushedRates[0]!.scaledRate).toBe(scaleRate(1600));
    expect(oracle.metrics.pairs["USD/NGN"]!.provider).toBe("fallback2");
  });

  it("skips pair and records error when ALL providers fail", async () => {
    const oracle = new RateOracle(
      buildConfig([
        failingProvider("primary"),
        failingProvider("fallback1"),
        failingProvider("fallback2"),
      ])
    );
    const state = mockPushMethods(oracle);

    const pushed = await oracle.runCycle();

    expect(pushed).toBe(0);
    expect(state.pushedRates).toHaveLength(0);
    const ps = oracle.metrics.pairs["USD/NGN"]!;
    expect(ps.errorCount).toBe(1);
    expect(ps.lastError).toContain("all providers failed");
    expect(oracle.metrics.failedRuns).toBe(1);
  });

  it("skips pair when all providers return null", async () => {
    const oracle = new RateOracle(
      buildConfig([
        nullProvider("primary"),
        nullProvider("fallback1"),
        nullProvider("fallback2"),
      ])
    );
    const state = mockPushMethods(oracle);

    await oracle.runCycle();

    expect(state.pushedRates).toHaveLength(0);
    expect(oracle.metrics.pairs["USD/NGN"]!.errorCount).toBe(1);
  });
});

// ── fallback chain — multiple pairs ──────────────────────────────────────────

describe("RateOracle multiple pairs", () => {
  it("succeeds for pairs where provider works and skips where it fails", async () => {
    const pairs: [string, string][] = [
      ["USD", "NGN"],
      ["USD", "KES"],
    ];

    // Provider returns a rate for NGN but throws for KES.
    const mixed: FxProvider = {
      name: "mixed",
      fetchRate: async (_from, to) => {
        if (to === "NGN") return 1580;
        throw new Error("KES not available");
      },
    };
    const fallback: FxProvider = {
      name: "fallback",
      fetchRate: async (_from, to) => {
        if (to === "KES") return 130;
        return null;
      },
    };

    const oracle = new RateOracle(
      buildConfig([mixed, failingProvider("fb2"), fallback], pairs)
    );
    const state = mockPushMethods(oracle);

    const pushed = await oracle.runCycle();

    expect(pushed).toBe(2);
    expect(state.pushedRates.map((r) => r.to)).toEqual(
      expect.arrayContaining(["NGN", "KES"])
    );
  });
});

// ── metrics tracking ──────────────────────────────────────────────────────────

describe("OracleMetrics", () => {
  it("increments totalRuns on every cycle", async () => {
    const oracle = new RateOracle(
      buildConfig([successProvider("p", 1)])
    );
    mockPushMethods(oracle);

    await oracle.runCycle();
    await oracle.runCycle();

    expect(oracle.metrics.totalRuns).toBe(2);
  });

  it("records lastSuccessfulRunMs after a successful cycle", async () => {
    const oracle = new RateOracle(
      buildConfig([successProvider("p", 1)])
    );
    mockPushMethods(oracle);
    const before = Date.now();
    await oracle.runCycle();
    const after = Date.now();

    expect(oracle.metrics.lastSuccessfulRunMs).toBeGreaterThanOrEqual(before);
    expect(oracle.metrics.lastSuccessfulRunMs).toBeLessThanOrEqual(after);
  });

  it("resets provider error counter after a successful run via that provider", async () => {
    const oracle = new RateOracle(
      buildConfig([
        failingProvider("primary"),
        successProvider("fallback1", 1),
      ])
    );
    mockPushMethods(oracle);

    await oracle.runCycle();

    // primary had one error.
    expect(oracle.metrics.providerErrors["primary"]).toBe(1);
    // fallback1 succeeded — its counter should be 0.
    expect(oracle.metrics.providerErrors["fallback1"]).toBe(0);
  });

  it("pushes heartbeat after a successful cycle", async () => {
    const oracle = new RateOracle(
      buildConfig([successProvider("p", 1)])
    );
    const state = mockPushMethods(oracle);

    await oracle.runCycle();

    expect(state.heartbeatCount).toBe(1);
  });

  it("does not push heartbeat when zero pairs succeeded", async () => {
    const oracle = new RateOracle(
      buildConfig([failingProvider("p")])
    );
    const state = mockPushMethods(oracle);

    await oracle.runCycle();

    expect(state.heartbeatCount).toBe(0);
  });
});

// ── config — validateAndLoadKeypair ──────────────────────────────────────────

describe("validateAndLoadKeypair", () => {
  it("accepts a valid Stellar secret key", () => {
    const kp = Keypair.random();
    const loaded = validateAndLoadKeypair(kp.secret());
    expect(loaded.publicKey()).toBe(kp.publicKey());
  });

  it("throws when key does not start with S", () => {
    expect(() => validateAndLoadKeypair("GABCDE")).toThrow(
      "must start with 'S'"
    );
  });

  it("throws when key has wrong length", () => {
    expect(() => validateAndLoadKeypair("S" + "A".repeat(10))).toThrow(
      "unexpected length"
    );
  });

  it("throws when key is structurally invalid", () => {
    // 56 chars starting with S but not a valid base32 seed.
    expect(() =>
      validateAndLoadKeypair("S" + "!".repeat(55))
    ).toThrow();
  });
});
